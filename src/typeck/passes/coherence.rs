use thin_vec::ThinVec;

use crate::diag_params;
use crate::errors::builders;
use crate::hir::{DefId, DefKind, HirId, ItemKind, ModuleId, OwnerNode};
use crate::interner::Symbol;
use crate::resolve::{Res, ResolverOutputs};
use crate::typeck::fold::{
    expand_type_alias, fold_ty, resolve_scheme_with_args, substitute_ty_vars,
};
use crate::typeck::infctx::{InferCtx, TyVarId};
use crate::typeck::types::Ty;
use crate::typeck::unify::unify;
use crate::typeck::{Scheme, Typeck, diag};

use super::check::{emit_ty_from_hir_error, emit_unexpected_generic_args};
use fxhash::{FxHashMap, FxHashSet};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(crate) fn check_coherence(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        // Take ownership of the crate's owners to avoid borrowing issues
        let owners = std::mem::take(&mut self.krate.owners);
        for (i, owner) in owners.iter().enumerate() {
            let def_id = DefId(i as u32);
            let Some(item) = owner
                .as_owner()
                .map(|info| info.nodes.node())
                .and_then(|node| match node {
                    OwnerNode::Item(item) => Some(item),
                    _ => None,
                })
            else {
                continue;
            };
            let ItemKind::Impl {
                self_ty,
                trait_ty,
                items,
            } = &item.kind
            else {
                continue;
            };

            let impl_module = self
                .resolver
                .def_to_module
                .get(&def_id)
                .copied()
                .unwrap_or(ModuleId(0));

            let Some(struct_def_id) = self.resolve_struct(self_ty.res) else {
                builders::emit_at(
                    self.ctx,
                    self_ty.span,
                    impl_module,
                    diag::ImplExpectedPathToStruct,
                    diag_params! { type = self_ty.display(self.ctx) },
                );
                continue;
            };
            let Some(trait_def_id) = self.resolve_trait(trait_ty.res) else {
                builders::emit_at(
                    self.ctx,
                    trait_ty.span,
                    impl_module,
                    diag::ImplExpectedPathToTrait,
                    diag_params! { type = trait_ty.display(self.ctx) },
                );
                continue;
            };

            // 1. Extract and validate trait generic args from the path
            let trait_scheme = self.item_schemes.get(&trait_def_id).cloned();
            let trait_generic_args = match self.ty_hir_generic_args(&mut icx, trait_ty, impl_module)
            {
                Ok(args) => args,
                Err(err) => {
                    emit_ty_from_hir_error(&err, self.ctx);
                    continue;
                }
            };

            if let Some(scheme) = &trait_scheme
                && !scheme.vars.is_empty()
                && let Some(info) = self.coherence.generic_params.get(&trait_def_id)
            {
                for &hir_id in &info.hir_ids {
                    if !icx.hir_id_to_ty_var.contains_key(&hir_id) {
                        let var = icx.next_ty_var();
                        icx.hir_id_to_ty_var.insert(hir_id, var);
                    }
                }
            }

            // If no explicit generic args were provided and the trait has defaults, fill them in
            let trait_generic_args = match (&trait_scheme, &trait_generic_args) {
                (Some(scheme), None) if !scheme.vars.is_empty() => {
                    if let Some(info) = self.coherence.generic_params.get(&trait_def_id).cloned()
                        && info.defaults.iter().all(|d| d.is_some())
                    {
                        let mut subst: FxHashMap<TyVarId, Ty> = FxHashMap::default();
                        let mut args: ThinVec<Ty> = ThinVec::new();
                        for (i, default) in info.defaults.iter().enumerate() {
                            let mut ty = self
                                .ty_from_hir(
                                    &mut icx,
                                    default.as_ref().expect("default exists"),
                                    impl_module,
                                )
                                .unwrap_or_else(|err| {
                                    emit_ty_from_hir_error(&err, self.ctx);
                                    Ty::Error
                                });
                            ty = normalize_type_aliases(ty, self.resolver, &self.item_schemes);
                            if !subst.is_empty() {
                                ty = substitute_ty_vars(&ty, &subst);
                            }
                            ty = substitute_self(&ty, trait_def_id, struct_def_id);
                            if let Some(&var) = icx.hir_id_to_ty_var.get(&info.hir_ids[i]) {
                                subst.insert(var, ty.clone());
                            }
                            args.push(ty);
                        }
                        Some(args)
                    } else {
                        emit_unexpected_generic_args(
                            self.ctx,
                            trait_ty.span,
                            impl_module,
                            scheme.vars.len(),
                            0,
                        );
                        continue;
                    }
                }
                (Some(scheme), Some(args)) if scheme.vars.len() != args.len() => {
                    if args.len() < scheme.vars.len() {
                        match self.coherence.generic_params.get(&trait_def_id).cloned() {
                            Some(info)
                                if info.defaults[args.len()..].iter().all(|d| d.is_some()) =>
                            {
                                let mut subst: FxHashMap<TyVarId, Ty> = FxHashMap::default();
                                let mut full_args: ThinVec<Ty> = args.clone();
                                for (i, arg) in full_args.iter().enumerate() {
                                    if let Some(&var) = icx.hir_id_to_ty_var.get(&info.hir_ids[i]) {
                                        subst.insert(var, arg.clone());
                                    }
                                }
                                for (i, default) in
                                    info.defaults[full_args.len()..].iter().enumerate()
                                {
                                    let idx = full_args.len() + i;
                                    let mut ty = self
                                        .ty_from_hir(
                                            &mut icx,
                                            default.as_ref().expect("default exists"),
                                            impl_module,
                                        )
                                        .unwrap_or_else(|err| {
                                            emit_ty_from_hir_error(&err, self.ctx);
                                            Ty::Error
                                        });
                                    ty = normalize_type_aliases(
                                        ty,
                                        self.resolver,
                                        &self.item_schemes,
                                    );
                                    if !subst.is_empty() {
                                        ty = substitute_ty_vars(&ty, &subst);
                                    }
                                    ty = substitute_self(&ty, trait_def_id, struct_def_id);
                                    if let Some(&var) = icx.hir_id_to_ty_var.get(&info.hir_ids[idx])
                                    {
                                        subst.insert(var, ty.clone());
                                    }
                                    full_args.push(ty);
                                }
                                Some(full_args)
                            }
                            _ => {
                                emit_unexpected_generic_args(
                                    self.ctx,
                                    trait_ty.span,
                                    impl_module,
                                    scheme.vars.len(),
                                    args.len(),
                                );
                                continue;
                            }
                        }
                    } else {
                        emit_unexpected_generic_args(
                            self.ctx,
                            trait_ty.span,
                            impl_module,
                            scheme.vars.len(),
                            args.len(),
                        );
                        continue;
                    }
                }
                _ => trait_generic_args,
            };

            // 2. Check for duplicate impls using resolved generic args
            let self_type_generic_args =
                match self.ty_hir_generic_args(&mut icx, self_ty, impl_module) {
                    Ok(args) => args,
                    Err(err) => {
                        emit_ty_from_hir_error(&err, self.ctx);
                        continue;
                    }
                };
            let self_type = Ty::Adt(struct_def_id, self_type_generic_args);
            self.coherence
                .impl_resolved_generic_args
                .insert(def_id, trait_generic_args.clone());
            self.coherence
                .impl_resolved_self_type
                .insert(def_id, self_type.clone());

            let key = (trait_def_id, struct_def_id);
            // Check for conflicts against other existing impls (exclude self)
            let is_conflicting = self.coherence.impls.get(&key).is_some_and(|existing| {
                let others: Vec<_> = existing
                    .iter()
                    .copied()
                    .filter(|&id| id != def_id)
                    .collect();
                if others.is_empty() {
                    false
                } else {
                    self.coherence
                        .has_conflicting_impl(&others, &trait_generic_args, &self_type)
                }
            });
            if is_conflicting {
                let trait_ = self
                    .ctx
                    .interner
                    .lookup(
                        self.resolver.defs[trait_def_id.0 as usize]
                            .name
                            .expect("trait has name"),
                    )
                    .to_string();
                let strct = self
                    .ctx
                    .interner
                    .lookup(
                        self.resolver.defs[struct_def_id.0 as usize]
                            .name
                            .expect("struct has name"),
                    )
                    .to_string();
                let impl_span = self.resolver.defs[def_id.0 as usize].span;
                builders::emit_at(
                    self.ctx,
                    impl_span,
                    impl_module,
                    diag::ConflictingImplementations,
                    diag_params! { trait = trait_, struct = strct },
                );
                continue;
            }
            if !self
                .coherence
                .impls
                .get(&key)
                .is_some_and(|existing| existing.contains(&def_id))
            {
                self.coherence.impls.entry(key).or_default().push(def_id);
            }

            let mut generic_subst: FxHashMap<TyVarId, Ty> = FxHashMap::default();
            if let (Some(scheme), Some(ref args)) = (trait_scheme, trait_generic_args) {
                for (&var, arg) in scheme.vars.iter().zip(args.iter()) {
                    generic_subst.insert(var, arg.clone());
                }
            }

            // 3. Signatures check
            let Some(trait_methods) = self.coherence.trait_methods.get(&trait_def_id) else {
                continue;
            };

            let mut impl_methods: FxHashMap<Symbol, DefId> = FxHashMap::default();
            for item in items {
                impl_methods.insert(
                    self.resolver.defs[item.0 as usize]
                        .name
                        .expect("item has name"),
                    *item,
                );
            }

            // Build assoc type substitution
            let mut assoc_types: FxHashMap<Symbol, &Ty> = FxHashMap::default();
            for &impl_item_def_id in items {
                let impl_def = &self.resolver.defs[impl_item_def_id.0 as usize];
                if impl_def.kind == DefKind::AssocType {
                    let name = impl_def.name.expect("assoc type has name");
                    let scheme = self
                        .item_schemes
                        .get(&impl_item_def_id)
                        .expect("assoc type has scheme");
                    assoc_types.insert(name, &scheme.body);
                }
            }

            for (name, trait_method) in trait_methods.iter() {
                let Some(impl_method) = impl_methods.get(name) else {
                    let method = self.ctx.interner.lookup(*name).to_string();
                    builders::emit_at(
                        self.ctx,
                        item.span,
                        impl_module,
                        diag::MissingImplementation,
                        diag_params! { method = method },
                    );
                    continue;
                };
                let trait_sig = self.item_schemes.get(trait_method);
                let impl_sig = self.item_schemes.get(impl_method);
                if let (Some(trait_sig), Some(impl_sig)) = (trait_sig, impl_sig) {
                    let trait_sig_sub =
                        substitute_self(&trait_sig.body, trait_def_id, struct_def_id);
                    let trait_sig_sub = substitute_ty_vars(&trait_sig_sub, &generic_subst);
                    // Normalize projections recursively until stable
                    let normalize_projections = |ty: Ty| -> Ty {
                        let mut prev = ty;
                        let mut seen: FxHashSet<DefId> = FxHashSet::default();
                        loop {
                            let mut resolved_this_pass: FxHashSet<DefId> = FxHashSet::default();
                            let next = fold_ty(&prev, &mut |inner| match inner {
                                Ty::Projection { assoc_def_id, .. } => {
                                    if seen.contains(&assoc_def_id) {
                                        return inner;
                                    }
                                    if let Some(name) = self.resolver.def(assoc_def_id).name
                                        && let Some(&concrete) = assoc_types.get(&name)
                                    {
                                        resolved_this_pass.insert(assoc_def_id);
                                        concrete.clone()
                                    } else {
                                        inner
                                    }
                                }
                                t => t,
                            });
                            if next == prev {
                                break next;
                            }
                            seen.extend(resolved_this_pass);
                            prev = next;
                        }
                    };
                    let trait_sig_sub = normalize_projections(trait_sig_sub);
                    let impl_sig_sub = normalize_projections(impl_sig.body.clone());

                    let method_span = self.resolver.defs[impl_method.0 as usize].span;
                    if unify(
                        &mut icx,
                        &trait_sig_sub,
                        &impl_sig_sub,
                        method_span,
                        impl_module,
                    )
                    .is_err()
                    {
                        let method = self.ctx.interner.lookup(*name).to_string();
                        builders::emit_at(
                            self.ctx,
                            method_span,
                            impl_module,
                            diag::SignatureMismatch,
                            diag_params! { method = method },
                        );
                    }
                }
            }
        }
        // Restore the crate's owners
        self.krate.owners = owners;
    }

    pub(super) fn resolve_struct(&self, res: Res<HirId>) -> Option<DefId> {
        self.resolve_def_kind(res, DefKind::Struct)
    }

    pub(super) fn resolve_trait(&self, res: Res<HirId>) -> Option<DefId> {
        self.resolve_def_kind(res, DefKind::Trait)
    }

    fn resolve_def_kind(&self, res: Res<HirId>, kind: DefKind) -> Option<DefId> {
        let Res::Def(def_id) = res else {
            return None;
        };
        (self.resolver.def(def_id).kind == kind).then_some(def_id)
    }
}

fn normalize_type_aliases(
    ty: Ty,
    resolver: &ResolverOutputs,
    item_schemes: &FxHashMap<DefId, Scheme>,
) -> Ty {
    normalize_type_aliases_inner(ty, resolver, item_schemes, &mut FxHashSet::default())
}

fn normalize_type_aliases_inner(
    ty: Ty,
    resolver: &ResolverOutputs,
    item_schemes: &FxHashMap<DefId, Scheme>,
    in_progress: &mut FxHashSet<DefId>,
) -> Ty {
    fold_ty(&ty, &mut |ty| match ty {
        Ty::Adt(def_id, generic_args) if resolver.def(def_id).kind == DefKind::TypeAlias => {
            expand_type_alias(
                def_id,
                &generic_args,
                in_progress,
                |in_progress, def_id, generic_args| match item_schemes.get(&def_id) {
                    Some(scheme) => match resolve_scheme_with_args(scheme, generic_args) {
                        Some(resolved) => normalize_type_aliases_inner(
                            resolved,
                            resolver,
                            item_schemes,
                            in_progress,
                        ),
                        None => Ty::Adt(def_id, generic_args.clone()),
                    },
                    None => Ty::Adt(def_id, generic_args.clone()),
                },
            )
        }
        t => t,
    })
}

fn substitute_self(ty: &Ty, from: DefId, to: DefId) -> Ty {
    fold_ty(ty, &mut |ty| match ty {
        Ty::Adt(d, generics) if d == from => Ty::Adt(to, generics),
        ty => ty,
    })
}
