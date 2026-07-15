use thin_vec::ThinVec;

use crate::diag_params;
use crate::errors::builders;
use crate::hashmap::FxHashMap;
use crate::hir::{DefId, DefKind, HirId, ItemKind, MaybeOwner, ModuleId, Node};
use crate::interner::Symbol;
use crate::resolve::Res;
use crate::typeck::fold::{fold_ty, substitute_ty_vars};
use crate::typeck::infctx::{InferCtx, TyVarId};
use crate::typeck::types::Ty;
use crate::typeck::unify::unify;
use crate::typeck::{Typeck, diag};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(crate) fn check_coherence(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        for (i, owner) in self.krate.owners.iter().enumerate() {
            let def_id = DefId(i as u32);
            let MaybeOwner::Owner(info) = owner else {
                continue;
            };
            let Node::Item(item) = &info.nodes.nodes[0].node else {
                continue;
            };
            let ItemKind::Impl {
                self_ty,
                interface_ty,
                items,
            } = &item.kind
            else {
                continue;
            };

            let impl_module = self
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
            let Some(interface_def_id) = self.resolve_interface(interface_ty.res) else {
                builders::emit_at(
                    self.ctx,
                    interface_ty.span,
                    impl_module,
                    diag::ImplExpectedPathToInterface,
                    diag_params! { type = interface_ty.display(self.ctx) },
                );
                continue;
            };

            // 1. Extract and validate interface generic args from the path
            let interface_scheme = self.item_schemes.get(&interface_def_id);
            let interface_generic_args = Ty::hir_generic_params(&mut icx, interface_ty);

            // If no explicit generic args were provided and the interface has defaults, fill them in
            let interface_generic_args = match (interface_scheme, &interface_generic_args) {
                (Some(scheme), None) if !scheme.vars.is_empty() => {
                    if let Some(info) = self.coherence.generic_params.get(&interface_def_id)
                        && info.defaults.iter().all(|d| d.is_some())
                    {
                        let args: ThinVec<Ty> = info
                            .defaults
                            .iter()
                            .map(|d| {
                                let mut ty =
                                    Ty::from_hir(&mut icx, d.as_ref().expect("default exists"));
                                ty = substitute_self(&ty, interface_def_id, struct_def_id);
                                ty
                            })
                            .collect();
                        Some(args)
                    } else {
                        let provided_args_len = 0;
                        builders::emit_at(
                            self.ctx,
                            interface_ty.span,
                            impl_module,
                            diag::UnexpectedGenericParams,
                            diag_params! {
                                expected = scheme.vars.len(),
                                s = if scheme.vars.len() == 1 { "" } else { "s" },
                                found = provided_args_len
                            },
                        );
                        continue;
                    }
                }
                (Some(scheme), Some(args)) if scheme.vars.len() != args.len() => {
                    let provided_args_len = args.len();
                    builders::emit_at(
                        self.ctx,
                        interface_ty.span,
                        impl_module,
                        diag::UnexpectedGenericParams,
                        diag_params! {
                            expected = scheme.vars.len(),
                            s = if scheme.vars.len() == 1 { "" } else { "s" },
                            found = provided_args_len
                        },
                    );
                    continue;
                }
                _ => interface_generic_args,
            };

            // 2. Check for duplicate impls using resolved generic args
            self.coherence
                .impl_resolved_generic_args
                .insert(def_id, interface_generic_args.clone());

            let key = (interface_def_id, struct_def_id);
            let is_conflicting = self.coherence.impls.get(&key).is_some_and(|existing| {
                self.coherence
                    .has_conflicting_impl(existing, &interface_generic_args)
            });
            if is_conflicting {
                let iface = self
                    .ctx
                    .interner
                    .lookup(
                        self.resolver.defs[interface_def_id.0 as usize]
                            .name
                            .expect("interface has name"),
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
                    diag_params! { iface = iface, struct = strct },
                );
                continue;
            }
            self.coherence.impls.entry(key).or_default().push(def_id);

            let mut generic_subst: FxHashMap<TyVarId, Ty> = FxHashMap::default();
            if let (Some(scheme), Some(ref args)) = (interface_scheme, interface_generic_args) {
                for (&var, arg) in scheme.vars.iter().zip(args.iter()) {
                    generic_subst.insert(var, arg.clone());
                }
            }

            // 3. Signatures check
            let Some(interface_methods) = self.coherence.interface_methods.get(&interface_def_id)
            else {
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

            for (name, interface_method) in interface_methods.iter() {
                let Some(impl_method) = impl_methods.get(name) else {
                    let iface_span = self.resolver.defs[interface_method.0 as usize].span;
                    let iface_module = self
                        .def_to_module
                        .get(interface_method)
                        .copied()
                        .unwrap_or(impl_module);
                    let method = self.ctx.interner.lookup(*name).to_string();
                    builders::emit_at(
                        self.ctx,
                        iface_span,
                        iface_module,
                        diag::MissingImplementation,
                        diag_params! { method = method },
                    );
                    continue;
                };
                let iface_sig = self.item_schemes.get(interface_method);
                let impl_sig = self.item_schemes.get(impl_method);
                if let (Some(iface_sig), Some(impl_sig)) = (iface_sig, impl_sig) {
                    let iface_sig_sub =
                        substitute_self(&iface_sig.body, interface_def_id, struct_def_id);
                    let iface_sig_sub = substitute_ty_vars(&iface_sig_sub, &generic_subst);

                    let method_span = self.resolver.defs[impl_method.0 as usize].span;
                    if unify(
                        &mut icx,
                        &iface_sig_sub,
                        &impl_sig.body,
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
    }

    fn resolve_struct(&self, res: Res<HirId>) -> Option<DefId> {
        let Res::Def(def_id) = res else {
            return None;
        };
        match self.resolver.defs[def_id.0 as usize].kind {
            DefKind::Struct => Some(def_id),
            _ => None,
        }
    }

    fn resolve_interface(&self, res: Res<HirId>) -> Option<DefId> {
        let Res::Def(def_id) = res else {
            return None;
        };
        match self.resolver.defs[def_id.0 as usize].kind {
            DefKind::Interface => Some(def_id),
            _ => None,
        }
    }
}

fn substitute_self(ty: &Ty, from: DefId, to: DefId) -> Ty {
    fold_ty(ty, &mut |ty| match ty {
        Ty::Adt(d, generics) if d == from => Ty::Adt(to, generics),
        ty => ty,
    })
}
