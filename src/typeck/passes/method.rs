use crate::hir::{DefId, ItemKind, MaybeOwner, Node};
use crate::typeck::Typeck;

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(crate) fn build_method_tables(&mut self) {
        self.collect_inherent_methods();
        self.collect_interface_methods();
    }

    fn collect_inherent_methods(&mut self) {
        for (i, owner) in self.krate.owners.iter().enumerate() {
            let MaybeOwner::Owner(info) = owner else {
                continue;
            };
            let Node::Item(item) = &info.nodes.nodes[0].node else {
                continue;
            };
            let ItemKind::Struct { items, .. } = &item.kind else {
                continue;
            };
            let def_id = DefId(i as u32);
            let entry = self.inherent_methods.entry(def_id).or_default();
            for &item in items {
                entry.insert(
                    self.resolver.defs[item.0 as usize]
                        .name
                        .expect("item has name"),
                    item,
                );
            }
        }
    }

    fn collect_interface_methods(&mut self) {
        for ((iface_def_id, struct_def_id), impl_def_id) in self.coherence.impls.iter() {
            let Some(MaybeOwner::Owner(info)) = self.krate.owner(*impl_def_id) else {
                continue;
            };
            let Node::Item(item) = &info.nodes.nodes[0].node else {
                continue;
            };
            let ItemKind::Impl { items, .. } = &item.kind else {
                continue;
            };
            let entry = self.interface_methods.entry(*struct_def_id).or_default();
            for &item in items {
                entry.insert(
                    self.resolver.defs[item.0 as usize]
                        .name
                        .expect("item has name"),
                    (*iface_def_id, item),
                );
            }
        }
    }
}
