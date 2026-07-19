use crate::ast::{AssocItem, Ast, Item, ItemKind, NodeMap};
use crate::hir::DefId;
use crate::resolve::ModuleTree;

#[derive(Debug, Clone, Copy)]
pub enum AstOwner<'ast> {
    NonOwner,
    Item(&'ast Item),
    AssocItem(&'ast AssocItem),
}

#[derive(Debug)]
pub struct AstIndex<'ast> {
    owners: Vec<AstOwner<'ast>>,
}

impl<'ast> AstIndex<'ast> {
    pub fn get(&self, def_id: DefId) -> AstOwner<'ast> {
        self.owners
            .get(def_id.0 as usize)
            .copied()
            .unwrap_or(AstOwner::NonOwner)
    }

    /// Iterate over all `DefId`s whose entry is not `NonOwner`.
    pub fn indices(&self) -> impl Iterator<Item = DefId> + '_ {
        (0..self.owners.len())
            .filter(|&i| !matches!(self.owners[i], AstOwner::NonOwner))
            .map(|i| DefId(i as u32))
    }
}

pub fn index_crate<'ast>(
    asts: &'ast [Ast],
    module_tree: &'ast ModuleTree,
    def_map: &NodeMap<DefId>,
    def_count: usize,
) -> AstIndex<'ast> {
    let mut owners = vec![AstOwner::NonOwner; def_count];
    index_module_rec(0, module_tree, asts, def_map, &mut owners);
    AstIndex { owners }
}

fn index_module_rec<'ast>(
    node_idx: usize,
    module_tree: &'ast ModuleTree,
    asts: &'ast [Ast],
    def_map: &NodeMap<DefId>,
    owners: &mut [AstOwner<'ast>],
) {
    let node = &module_tree.nodes[node_idx];

    let items: &'ast [Item] = node
        .ast_idx
        .map(|ast_idx| &asts[ast_idx].items[..])
        .unwrap_or_else(|| node.inline_body.as_deref().unwrap_or(&[]));

    for item in items {
        if let Some(&def_id) = def_map.get(&item.node_id) {
            let idx = def_id.0 as usize;
            if idx < owners.len() {
                owners[idx] = AstOwner::Item(item);
            }
        }

        match &item.kind {
            ItemKind::Struct { items: assoc, .. }
            | ItemKind::Trait { items: assoc, .. }
            | ItemKind::Impl { items: assoc, .. } => {
                for assoc_item in assoc {
                    if let Some(&def_id) = def_map.get(&assoc_item.node_id) {
                        let idx = def_id.0 as usize;
                        if idx < owners.len() {
                            owners[idx] = AstOwner::AssocItem(assoc_item);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for &child in &node.children {
        index_module_rec(child, module_tree, asts, def_map, owners);
    }
}
