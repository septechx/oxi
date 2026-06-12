use thin_vec::ThinVec;

use super::check::format_unify_error;
use crate::errors::builders;
use crate::hir::{AssocItemKind, DefId, FnDecl, ItemKind, MaybeOwner, Node, OwnerInfo};
use crate::interner::Symbol;
use crate::typeck::Typeck;
use crate::typeck::infctx::InferCtx;
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
                        let ty = self.fn_ty(&mut icx, &fun.decl).reject_vars();
                        self.item_schemes.insert(def_id, Scheme::monomorphic(ty));
                    }
                    ItemKind::Const { ty, .. } => {
                        let ty = Ty::from_hir(&mut icx, ty).reject_vars();
                        self.item_schemes.insert(def_id, Scheme::monomorphic(ty));
                    }
                    ItemKind::Struct { fields, .. } => {
                        let entry = self.coherence.struct_fields.entry(def_id).or_default();
                        for (index, field) in fields.iter().enumerate() {
                            entry.insert(
                                field.name,
                                (Ty::from_hir(&mut icx, &field.ty).reject_vars(), index),
                            );
                        }
                    }
                    ItemKind::Interface { items, .. } => {
                        let mut methods: Vec<(Symbol, DefId)> = Vec::new();
                        for &item in items {
                            if let Some(MaybeOwner::Owner(method_info)) = self.krate.owner(item)
                                && let Node::AssocItem(_) = &method_info.nodes.nodes[0].node
                                && let Some(name) = self.resolver.defs[item.0 as usize].name
                            {
                                let ty = self.fn_ty_for_owner(&mut icx, method_info).reject_vars();
                                self.item_schemes.insert(item, Scheme::monomorphic(ty));
                                methods.push((name, item));
                            }
                        }
                        self.coherence.register_interface(def_id, methods);
                    }
                    ItemKind::Impl { .. } | ItemKind::Module { .. } | ItemKind::Import(_) => {}
                },
                Node::AssocItem(assoc) => {
                    let AssocItemKind::Fn(fun) = &assoc.kind;
                    let ty = self.fn_ty(&mut icx, &fun.decl).reject_vars();
                    self.item_schemes.insert(def_id, Scheme::monomorphic(ty));
                }
                _ => {}
            }
        }

        for err in icx.errors {
            let (msg, span, module_id) =
                format_unify_error(&err, self.resolver, &self.ctx.interner);
            self.ctx.errors.add(
                builders::error_at(msg, module_id, span, self.ctx),
                self.ctx.enable_printing,
            );
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

    fn fn_ty_for_owner(&self, icx: &mut InferCtx, info: &OwnerInfo) -> Ty {
        if let Node::AssocItem(assoc) = &info.nodes.nodes[0].node {
            let AssocItemKind::Fn(fun) = &assoc.kind;
            return self.fn_ty(icx, &fun.decl);
        }
        Ty::Error
    }
}
