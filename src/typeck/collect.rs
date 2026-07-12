use thin_vec::ThinVec;

use super::check::emit_unify_error;
use crate::hir::{AssocItemKind, DefId, FnDecl, HirId, ItemKind, MaybeOwner, Node};
use crate::interner::Symbol;
use crate::typeck::Typeck;
use crate::typeck::infctx::{InferCtx, TyVarId};
use crate::typeck::types::{Scheme, Ty};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(super) fn collect_signatures(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        for (i, owner) in self.krate.owners.iter().enumerate() {
            let def_id = DefId(i as u32);
            let MaybeOwner::Owner(info) = owner else {
                continue;
            };

            match &info.nodes.nodes[0].node {
                Node::Item(item) => match &item.kind {
                    ItemKind::Fn(fun) => {
                        let generic_params = fun.generic_params.as_ref().map(|params| {
                            params
                                .iter()
                                .map(|param| {
                                    let ty_var = icx.next_ty_var();
                                    icx.hir_id_to_ty_var.insert(param.hir_id, ty_var);
                                    ty_var
                                })
                                .collect()
                        });
                        let ty = self.fn_ty(&mut icx, &fun.decl);
                        self.item_schemes.insert(
                            def_id,
                            Scheme {
                                vars: generic_params.unwrap_or_default(),
                                body: ty,
                            },
                        );
                    }
                    ItemKind::Const { ty, .. } => {
                        let ty = Ty::from_hir(&mut icx, ty).reject_vars();
                        self.item_schemes.insert(def_id, Scheme::monomorphic(ty));
                    }
                    ItemKind::Struct {
                        fields,
                        generic_params,
                        ..
                    } => {
                        let param_vars: Vec<TyVarId> = generic_params
                            .as_ref()
                            .map(|params| {
                                params
                                    .iter()
                                    .map(|param| {
                                        let ty_var = icx.next_ty_var();
                                        icx.hir_id_to_ty_var.insert(param.hir_id, ty_var);
                                        ty_var
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let param_hir_ids: Vec<HirId> = generic_params
                            .as_ref()
                            .map(|params| params.iter().map(|param| param.hir_id).collect())
                            .unwrap_or_default();
                        self.coherence
                            .struct_generic_params
                            .insert(def_id, param_hir_ids);
                        let entry = self.coherence.struct_fields.entry(def_id).or_default();
                        for (index, field) in fields.iter().enumerate() {
                            entry.insert(field.name, (field.ty.clone(), index));
                        }
                        let generic_args = if param_vars.is_empty() {
                            None
                        } else {
                            Some(param_vars.iter().map(|&v| Ty::Var(v)).collect())
                        };
                        let body = Ty::Adt(def_id, generic_args);
                        self.item_schemes.insert(
                            def_id,
                            Scheme {
                                vars: param_vars.into(),
                                body,
                            },
                        );
                    }
                    ItemKind::Interface {
                        items,
                        generic_params,
                        ..
                    } => {
                        let param_vars: Vec<TyVarId> = generic_params
                            .as_ref()
                            .map(|params| {
                                params
                                    .iter()
                                    .map(|param| {
                                        let ty_var = icx.next_ty_var();
                                        icx.hir_id_to_ty_var.insert(param.hir_id, ty_var);
                                        ty_var
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let param_hir_ids: Vec<HirId> = generic_params
                            .as_ref()
                            .map(|params| params.iter().map(|param| param.hir_id).collect())
                            .unwrap_or_default();
                        self.coherence
                            .interface_generic_params
                            .insert(def_id, param_hir_ids);
                        let generic_args = if param_vars.is_empty() {
                            None
                        } else {
                            Some(param_vars.iter().map(|&v| Ty::Var(v)).collect())
                        };
                        let body = Ty::Adt(def_id, generic_args);
                        self.item_schemes.insert(
                            def_id,
                            Scheme {
                                vars: param_vars.into(),
                                body,
                            },
                        );
                        let methods: Vec<(Symbol, DefId)> = items
                            .iter()
                            .filter_map(|&item| {
                                self.resolver.defs[item.0 as usize]
                                    .name
                                    .map(|name| (name, item))
                            })
                            .collect();
                        self.coherence.register_interface(def_id, methods);
                    }
                    ItemKind::Impl { .. } | ItemKind::Module { .. } | ItemKind::Import(_) => {}
                },
                Node::AssocItem(assoc) => {
                    let AssocItemKind::Fn(fun) = &assoc.kind;
                    let generic_params = fun.generic_params.as_ref().map(|params| {
                        params
                            .iter()
                            .map(|param| {
                                let ty_var = icx.next_ty_var();
                                icx.hir_id_to_ty_var.insert(param.hir_id, ty_var);
                                ty_var
                            })
                            .collect()
                    });
                    let ty = self.fn_ty(&mut icx, &fun.decl);
                    self.item_schemes.insert(
                        def_id,
                        Scheme {
                            vars: generic_params.unwrap_or_default(),
                            body: ty,
                        },
                    );
                }
                _ => {}
            }
        }

        for err in &icx.errors {
            emit_unify_error(err, self.resolver, self.ctx, &icx);
        }
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
}
