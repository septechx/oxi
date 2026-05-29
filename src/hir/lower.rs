use crate::ast::{Item, ItemKind};
use crate::hir::index::{AstIndex, AstOwner};
use crate::hir::{AstLoweringContext, DefId};

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_node(&mut self, def_id: DefId, index: &AstIndex<'a>) {
        match index.get(def_id) {
            AstOwner::NonOwner => {}
            AstOwner::Item(item) => self.lower_item(item),
        }
    }

    fn lower_item(&mut self, item: &'a Item) {
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
}
