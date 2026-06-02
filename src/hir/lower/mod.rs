mod expr;
mod item;
mod stmt;

use crate::ast::{self, NodeId};
use crate::hir::index::{AstIndex, AstOwner};
use crate::hir::types::*;
use crate::hir::{AstLoweringContext, Crate, DefId};
use crate::resolve::Res;

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_node(
        &mut self,
        def_id: DefId,
        index: &AstIndex<'a>,
        hir_crate: &mut Crate,
    ) {
        match index.get(def_id) {
            AstOwner::NonOwner => {}
            AstOwner::Item(item) => self.lower_item(def_id, item, hir_crate),
            AstOwner::AssocItem(item) => self.lower_assoc_item(def_id, item, hir_crate),
        }
    }

    pub(super) fn lower_path(&self, path: &ast::Path, node_id: NodeId) -> Path {
        let partial = self.resolver.res_map.get(&node_id).copied();
        let res = match partial.and_then(|p| p.full_res()) {
            Some(Res::Def(def_id)) => Res::Def(def_id),
            Some(Res::PrimTy(prim)) => Res::PrimTy(prim),
            Some(Res::Local(local_node_id)) => match self.lookup_local(local_node_id) {
                Some(hir_id) => Res::Local(hir_id),
                None => Res::Err,
            },
            Some(Res::SelfTyAlias { alias_to }) => Res::SelfTyAlias { alias_to },
            Some(Res::Err) | None => Res::Err,
        };

        Path {
            res,
            segments: path.segments.clone(),
        }
    }
}
