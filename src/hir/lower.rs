use crate::ast::{AssocItem, AssocItemKind, Item, ItemKind};
use crate::hir::index::{AstIndex, AstOwner};
use crate::hir::{AstLoweringContext, Crate, DefId};

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_node(
        &mut self,
        def_id: DefId,
        index: &AstIndex<'a>,
        hir_crate: &mut Crate,
    ) {
        match index.get(def_id) {
            AstOwner::NonOwner => {}
            AstOwner::Item(item) => self.lower_item(item, hir_crate),
            AstOwner::AssocItem(item) => self.lower_assoc_item(item, hir_crate),
        }
    }

    fn lower_item(&mut self, item: &'a Item, hir_crate: &mut Crate) {
        let _ = hir_crate;
        match &item.kind {
            ItemKind::Const { .. } => todo!("Implement lowering of const items"),
            ItemKind::Struct { .. } => todo!("Implement lowering of struct items"),
            ItemKind::Interface { .. } => todo!("Implement lowering of interface items"),
            ItemKind::Impl { .. } => todo!("Implement lowering of impl items"),
            ItemKind::Fn(_) => todo!("Implement lowering of function items"),
            ItemKind::Import(_) => {}
            ItemKind::Module { .. } => {}
        }
    }

    fn lower_assoc_item(&mut self, item: &'a AssocItem, hir_crate: &mut Crate) {
        let _ = hir_crate;
        match &item.kind {
            AssocItemKind::Fn(_) => todo!("Implement lowering of associated fn items"),
        }
    }
}
