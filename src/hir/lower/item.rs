use thin_vec::ThinVec;

use crate::ast::{self, Ident, Visibility};
use crate::hashmap::FxHashMap;
use crate::hir::owner::{HirId, MaybeOwner, OwnerInfo, OwnerNodes, ParentedNode};
use crate::hir::types::*;
use crate::hir::{AstLoweringContext, BodyId, Crate, DefId, ItemLocalId, OwnerId};
use crate::span::Span;

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_item(&mut self, def_id: DefId, item: &'a ast::Item, hir_crate: &mut Crate) {
        self.with_owner(def_id, hir_crate, |this, def_id| {
            Some(match &item.kind {
                ast::ItemKind::Const { name, ty, value } => {
                    this.lower_const(def_id, name, ty, value, item.span, item.visibility)
                }
                ast::ItemKind::Fn(f) => this.lower_fn(def_id, f, item.span, item.visibility),
                ast::ItemKind::Struct { .. } => todo!("Implement lowering of struct items"),
                ast::ItemKind::Interface { .. } => todo!("Implement lowering of interface items"),
                ast::ItemKind::Impl { .. } => todo!("Implement lowering of impl items"),
                ast::ItemKind::Import(_) | ast::ItemKind::Module { .. } => return None,
            })
        })
    }

    fn with_owner(
        &mut self,
        def_id: DefId,
        hir_crate: &mut Crate,
        f: impl FnOnce(&mut Self, DefId) -> Option<OwnerInfo>,
    ) {
        self.current_owner = Some(OwnerId(def_id.0));
        self.next_local_id = 1;
        if let Some(info) = f(self, def_id) {
            self.store_owner(def_id, hir_crate, info);
        } else {
            self.current_owner = None;
        }
    }

    pub(super) fn lower_assoc_item(
        &mut self,
        def_id: DefId,
        item: &'a ast::AssocItem,
        hir_crate: &mut Crate,
    ) {
        match &item.kind {
            ast::AssocItemKind::Fn(_f) => {
                _ = def_id;
                _ = hir_crate;
                todo!()
            }
        }
    }

    fn store_owner(&mut self, def_id: DefId, hir_crate: &mut Crate, owner_info: OwnerInfo) {
        self.current_owner = None;
        hir_crate.ensure_owner(def_id);
        if let Some(owner) = hir_crate.owner_mut(def_id) {
            *owner = MaybeOwner::Owner(Box::new(owner_info));
        }
    }

    fn lower_const(
        &mut self,
        def_id: DefId,
        name: &'a Ident,
        ty: &'a ast::Type,
        value: &'a ast::Expr,
        span: Span,
        visibility: Visibility,
    ) -> OwnerInfo {
        let owner_id = OwnerId(def_id.0);
        let hir_id = HirId::make_owner(def_id);

        let ty = self.lower_type(ty);
        let init = self.lower_expr(value);

        let local_id = self.next_local_id();
        let body_id = BodyId(local_id);
        let bodies = {
            let mut bodies = FxHashMap::default();
            bodies.insert(body_id, Body { value: init });
            bodies
        };

        OwnerInfo {
            nodes: OwnerNodes {
                nodes: vec![ParentedNode {
                    parent: ItemLocalId::ZERO,
                    node: Node::Item(Box::new(Item {
                        hir_id,
                        owner_id,
                        kind: ItemKind::Const {
                            name: name.value,
                            ty,
                            body_id: Some(body_id),
                        },
                        span,
                        visibility,
                    })),
                }],
                bodies,
            },
            parenting: FxHashMap::default(),
        }
    }

    fn lower_fn(
        &mut self,
        def_id: DefId,
        f: &'a ast::Fn,
        span: Span,
        visibility: Visibility,
    ) -> OwnerInfo {
        let owner_id = OwnerId(def_id.0);
        let hir_id = HirId::make_owner(def_id);

        let params: ThinVec<Param> = f
            .parameters
            .iter()
            .map(|(name, ty, node_id)| {
                let param_hir_id = self.next_hir_id();
                self.register_local(*node_id, param_hir_id);
                Param {
                    hir_id: param_hir_id,
                    name: name.value,
                    ty: self.lower_type(ty),
                    span: name.span,
                }
            })
            .collect();

        let ret = self.lower_type(&f.return_type);

        let mut bodies = FxHashMap::default();
        let body_id = f.body.as_ref().map(|block| {
            let (body_id, body) = self.lower_block_body(block);
            bodies.insert(body_id, body);
            body_id
        });

        OwnerInfo {
            nodes: OwnerNodes {
                nodes: vec![ParentedNode {
                    parent: ItemLocalId::ZERO,
                    node: Node::Item(Box::new(Item {
                        hir_id,
                        owner_id,
                        kind: ItemKind::Fn {
                            sig: FnSig {
                                is_extern: f.is_extern,
                            },
                            decl: FnDecl { params, ret },
                            body_id,
                        },
                        span,
                        visibility,
                    })),
                }],
                bodies,
            },
            parenting: FxHashMap::default(),
        }
    }

    fn lower_block_body(&mut self, block: &ast::Block) -> (BodyId, Body) {
        let stmts = block
            .stmts
            .iter()
            .map(|stmt| self.lower_stmt(stmt))
            .collect();
        let block_expr = Expr {
            hir_id: self.next_hir_id(),
            kind: ExprKind::Block(Block {
                stmts,
                span: block.span,
            }),
            span: block.span,
        };
        let local_id = self.next_local_id();
        (BodyId(local_id), Body { value: block_expr })
    }
}
