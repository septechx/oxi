mod expr;
mod item;
mod stmt;

use crate::ast::{self, NodeId};
use crate::hir::index::{AstIndex, AstOwner};
use crate::hir::owner::HirId;
use crate::hir::types::*;
use crate::hir::{AstLoweringContext, Crate, DefId};
use crate::resolve::{PartialRes, Res};

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

    pub(super) fn lower_qpath(&mut self, path: &ast::Path, node_id: NodeId) -> QPath {
        let partial = self
            .resolver
            .res_map
            .get(&node_id)
            .copied()
            .unwrap_or(PartialRes::new(Res::Err));
        let unresolved = partial.unresolved_segments();

        if unresolved == 0 {
            return QPath::Resolved(self.lower_resolved_path(path, partial.base_res()));
        }

        let start = path.segments.len() - unresolved;
        let prefix = ast::Path {
            segments: path.segments[..start].into(),
            span: path.span,
        };
        let resolved = self.lower_resolved_path(&prefix, partial.base_res());

        let tail = &path.segments[start..];
        tail.iter()
            .fold(QPath::Resolved(resolved), |qself, &segment| {
                QPath::TypeRelative {
                    qself: Box::new(qself),
                    segment,
                }
            })
    }

    fn lower_resolved_path(&self, path: &ast::Path, res: Res) -> Path {
        Path {
            res: self.lower_res(res),
            segments: path.segments.clone(),
            span: path.span,
        }
    }

    fn lower_res(&self, res: Res) -> Res<HirId> {
        match res {
            Res::Def(def_id) => Res::Def(def_id),
            Res::PrimTy(prim) => Res::PrimTy(prim),
            Res::Local(local_node_id) => match self.lookup_local(local_node_id) {
                Some(hir_id) => Res::Local(hir_id),
                None => Res::Err,
            },
            Res::SelfTyAlias { alias_to } => Res::SelfTyAlias { alias_to },
            Res::Err => Res::Err,
        }
    }
}
