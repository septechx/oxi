use fxhash::{FxHashMap, FxHashSet};
use thin_vec::ThinVec;

use super::check::emit_unify_error;
use crate::ast::visit::VisitAction;
use crate::diag_params;
use crate::errors::builders;
use crate::hir::{
    self, AssocItemKind, Def, DefId, DefKind, FnDecl, GenericParam, HirId, ItemKind, OwnerNode,
};
use crate::interner::Symbol;
use crate::typeck::fold::fold_ty;
use crate::typeck::infctx::{InferCtx, TyVarId};
use crate::typeck::types::{Scheme, Ty};
use crate::typeck::{GenericParamInfo, TyVisitable, TyVisitor, Typeck, diag};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(crate) fn collect_signatures(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        for (i, owner) in self.krate.owners.iter().enumerate() {
            let def_id = DefId(i as u32);
            let Some(info) = owner.as_owner() else {
                continue;
            };

            match &info.nodes.node() {
                OwnerNode::Item(item) => match &item.kind {
                    ItemKind::Fn(fun) => {
                        let (vars, hir_ids, defaults) =
                            Self::collect_generic_params(&mut icx, &fun.generic_params);
                        if !hir_ids.is_empty() {
                            self.coherence
                                .generic_params
                                .insert(def_id, GenericParamInfo { hir_ids, defaults });
                        }
                        let body = self.fn_ty(&mut icx, &fun.decl);
                        self.item_schemes.insert(def_id, Scheme { vars, body });
                    }
                    ItemKind::Const { ty, .. } => {
                        let ty = Ty::from_hir(&mut icx, ty).reject_vars();
                        self.item_schemes.insert(def_id, Scheme::monomorphic(ty));
                    }
                    ItemKind::Struct {
                        fields,
                        items,
                        generic_params,
                        ..
                    } => {
                        let (vars, hir_ids, defaults) =
                            Self::collect_generic_params(&mut icx, generic_params);
                        self.coherence
                            .generic_params
                            .insert(def_id, GenericParamInfo { hir_ids, defaults });

                        let scheme = Self::adt_scheme(def_id, vars);
                        self.item_schemes.insert(def_id, scheme);

                        let entry = self.coherence.struct_fields.entry(def_id).or_default();
                        for (index, field) in fields.iter().enumerate() {
                            entry.insert(field.name, (field.ty.clone(), index));
                        }

                        for &item_def_id in items {
                            self.coherence.assoc_to_parent.insert(item_def_id, def_id);
                        }
                    }
                    ItemKind::TypeAlias {
                        type_,
                        generic_params,
                        ..
                    } => {
                        let (vars, hir_ids, defaults) =
                            Self::collect_generic_params(&mut icx, generic_params);
                        if !hir_ids.is_empty() {
                            self.coherence
                                .generic_params
                                .insert(def_id, GenericParamInfo { hir_ids, defaults });
                        }
                        let body = Ty::from_hir(&mut icx, type_);
                        self.item_schemes.insert(def_id, Scheme { vars, body });
                    }
                    ItemKind::Trait {
                        items,
                        generic_params,
                        ..
                    } => {
                        let (vars, hir_ids, defaults) =
                            Self::collect_generic_params(&mut icx, generic_params);
                        self.coherence
                            .generic_params
                            .insert(def_id, GenericParamInfo { hir_ids, defaults });

                        let scheme = Self::adt_scheme(def_id, vars);
                        self.item_schemes.insert(def_id, scheme);

                        let methods: Vec<(Symbol, DefId)> = items
                            .iter()
                            .filter_map(|&item| {
                                self.resolver.defs[item.0 as usize]
                                    .name
                                    .map(|name| (name, item))
                            })
                            .collect();
                        self.coherence.register_trait(def_id, methods);

                        for &item_def_id in items {
                            self.coherence.assoc_to_parent.insert(item_def_id, def_id);
                        }
                    }
                    ItemKind::Impl { items, .. } => {
                        self.coherence.generic_params.insert(
                            def_id,
                            GenericParamInfo {
                                hir_ids: Vec::new(),
                                defaults: ThinVec::new(),
                            },
                        );

                        for &item_def_id in items {
                            self.coherence.assoc_to_parent.insert(item_def_id, def_id);
                        }
                    }
                },
                OwnerNode::AssocItem(assoc) => match &assoc.kind {
                    AssocItemKind::Fn(fun) => {
                        let (mut scheme_vars, hir_ids, defaults) =
                            Self::collect_generic_params(&mut icx, &fun.generic_params);

                        let parent_def_id = self
                            .coherence
                            .assoc_to_parent
                            .get(&def_id)
                            .expect("assoc item has parent");
                        let parent_info = self
                            .coherence
                            .generic_params
                            .get(parent_def_id)
                            .expect("assoc item parent has generic params");
                        let parent_vars: ThinVec<TyVarId> = parent_info
                            .hir_ids
                            .iter()
                            .map(|hir_id| {
                                *icx.hir_id_to_ty_var
                                    .get(hir_id)
                                    .expect("parent generic param registered")
                            })
                            .collect();
                        let parent_args: ThinVec<Ty> =
                            parent_vars.iter().map(|&v| Ty::Var(v)).collect();
                        scheme_vars = parent_vars.into_iter().chain(scheme_vars).collect();
                        let body = self.fn_ty(&mut icx, &fun.decl);
                        let body = fold_ty(&body, &mut |ty| match ty {
                            Ty::Adt(id, None) if id == *parent_def_id => Ty::Adt(
                                id,
                                (!parent_args.is_empty()).then_some(parent_args.clone()),
                            ),
                            t => t,
                        });
                        self.item_schemes.insert(
                            def_id,
                            Scheme {
                                vars: scheme_vars,
                                body,
                            },
                        );
                        if !hir_ids.is_empty() {
                            self.coherence
                                .generic_params
                                .insert(def_id, GenericParamInfo { hir_ids, defaults });
                        }
                    }
                    AssocItemKind::Type { name, type_ } => {
                        todo!("type assoc item: {name:?} = {type_:?}")
                    }
                },
                OwnerNode::Crate => {}
            }
        }

        for err in &icx.errors {
            emit_unify_error(err, self.resolver, self.ctx, &icx);
        }
    }

    pub(crate) fn check_type_aliases(&mut self) {
        let type_aliases: Vec<_> = self
            .krate
            .owners
            .iter()
            .enumerate()
            .filter_map(|(i, owner)| {
                let info = owner.as_owner()?;
                let OwnerNode::Item(item) = info.nodes.node() else {
                    return None;
                };

                matches!(item.kind, ItemKind::TypeAlias { .. })
                    .then_some((DefId(i as u32), item.span))
            })
            .collect();

        let mut visited = FxHashSet::default();

        for (def_id, span) in type_aliases {
            if !visited.insert(def_id) {
                continue;
            }

            let mut in_progress = FxHashSet::default();

            if Self::visit_alias(
                def_id,
                def_id,
                &self.item_schemes,
                &self.resolver.defs,
                &mut visited,
                &mut in_progress,
            ) {
                let module_id = self
                    .resolver
                    .def_to_module
                    .get(&def_id)
                    .copied()
                    .unwrap_or_default();

                builders::emit_at(
                    self.ctx,
                    span,
                    module_id,
                    diag::RecursiveType,
                    diag_params! {},
                );
            }
        }
    }

    fn visit_alias(
        current: DefId,
        start: DefId,
        item_schemes: &FxHashMap<DefId, Scheme>,
        defs: &ThinVec<Def>,
        visited: &mut FxHashSet<DefId>,
        in_progress: &mut FxHashSet<DefId>,
    ) -> bool {
        struct AliasVisitor<'a> {
            start: DefId,
            item_schemes: &'a FxHashMap<DefId, Scheme>,
            defs: &'a ThinVec<Def>,
            visited: &'a mut FxHashSet<DefId>,
            in_progress: &'a mut FxHashSet<DefId>,
            found_cycle: bool,
        }

        impl TyVisitor for AliasVisitor<'_> {
            fn visit_ty(&mut self, ty: &Ty) -> VisitAction {
                if self.found_cycle {
                    return VisitAction::SkipChildren;
                }

                let Ty::Adt(def_id, _) = ty else {
                    return VisitAction::Continue;
                };
                if self.defs[def_id.0 as usize].kind != DefKind::TypeAlias {
                    return VisitAction::Continue;
                }
                if *def_id == self.start || self.in_progress.contains(def_id) {
                    self.found_cycle = true;
                    return VisitAction::SkipChildren;
                }
                if !self.visited.insert(*def_id) {
                    return VisitAction::SkipChildren;
                }

                if Typeck::visit_alias(
                    *def_id,
                    self.start,
                    self.item_schemes,
                    self.defs,
                    self.visited,
                    self.in_progress,
                ) {
                    self.found_cycle = true;
                }

                VisitAction::SkipChildren
            }
        }

        let Some(scheme) = item_schemes.get(&current) else {
            return false;
        };

        in_progress.insert(current);

        let mut visitor = AliasVisitor {
            start,
            item_schemes,
            defs,
            visited,
            in_progress,
            found_cycle: false,
        };

        scheme.body.visit(&mut visitor);

        let found_cycle = visitor.found_cycle;

        in_progress.remove(&current);

        found_cycle
    }

    fn collect_generic_params(
        icx: &mut InferCtx,
        generic_params: &Option<ThinVec<GenericParam>>,
    ) -> (ThinVec<TyVarId>, Vec<HirId>, ThinVec<Option<hir::Ty>>) {
        generic_params
            .as_ref()
            .map(|params| {
                params
                    .iter()
                    .map(|param| {
                        let ty_var = icx.next_ty_var();
                        icx.hir_id_to_ty_var.insert(param.hir_id, ty_var);
                        (ty_var, param.hir_id, param.default.clone())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn fn_ty(&self, icx: &mut InferCtx, decl: &FnDecl) -> Ty {
        let params: ThinVec<Ty> = decl
            .params
            .iter()
            .map(|param| Ty::from_hir(icx, &param.ty))
            .collect();
        let ret = Ty::from_hir(icx, &decl.ret).into_box();
        Ty::Fn { params, ret }
    }

    fn adt_scheme(def_id: DefId, vars: ThinVec<TyVarId>) -> Scheme {
        let generic_args = if vars.is_empty() {
            None
        } else {
            Some(vars.iter().map(|&v| Ty::Var(v)).collect())
        };
        let body = Ty::Adt(def_id, generic_args);
        Scheme { vars, body }
    }
}
