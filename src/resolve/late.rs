use thin_vec::ThinVec;

use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{AssocItem, AssocItemKind, Fn, Item, ItemKind};
use crate::hashmap::FxHashMap;
use crate::hir::interner::Symbol;
use crate::resolve::{DefKind, Res, Resolver};

impl<'a> Resolver<'a> {
    pub(super) fn late_resolve(&mut self) {
        for (i, ast) in self.asts.iter().enumerate() {
            self.module_idx = i;
            ast.visit(&mut LateResolutionVisitor::new(self));
        }
    }
}

#[derive(Debug, Default)]
struct Rib {
    pub bindings: FxHashMap<Symbol, Res>,
    pub kind: RibKind,
}

#[derive(Debug, Default)]
enum RibKind {
    /// No restrictions
    #[default]
    Normal,
    // TODO: Add other kinds to restrict invalid references
}

#[derive(Debug)]
struct LateResolutionVisitor<'a, 'res> {
    resolver: &'a mut Resolver<'res>,
    ribs: ThinVec<Rib>,
}

impl<'a, 'res> LateResolutionVisitor<'a, 'res> {
    pub fn new(resolver: &'a mut Resolver<'res>) -> Self {
        Self {
            resolver,
            ribs: ThinVec::new(),
        }
    }

    fn with_rib(&mut self, rib: Rib, f: impl FnOnce(&mut Self)) {
        self.ribs.push(rib);
        f(self);
        self.ribs.pop();
    }

    fn resolve_fn(&mut self, fun: &Fn) {
        self.with_rib(Rib::default(), |this| {
            for arg in &fun.parameters {
                arg.1.visit(this);

                let sym = this.resolver.interner.intern(&arg.0.value);
                let rib = this.ribs.last_mut().expect("rib exists");
                rib.bindings.insert(sym, Res::Local(arg.2));
            }
            if let Some(body) = &fun.body {
                body.visit(this);
            }
            fun.return_type.visit(this);
        });
    }

    fn resolve_assoc_items(&mut self, items: &ThinVec<AssocItem>) {
        for item in items {
            match &item.kind {
                AssocItemKind::Fn(fun) => self.resolve_fn(fun),
            }
        }
    }
}

impl<'a, 'res> Visitor for LateResolutionVisitor<'a, 'res> {
    fn visit_item(&mut self, item: &Item) -> VisitAction {
        if matches!(item.kind, ItemKind::Import(_) | ItemKind::Impl { .. }) {
            return VisitAction::SkipChildren;
        }

        let def_id = self
            .resolver
            .def_id_for_node(item.node_id)
            .expect("def already resolved");
        let def = self.resolver.get_def(def_id);

        match def.kind {
            DefKind::Const => {
                let ItemKind::Const { value, ty, .. } = &item.kind else {
                    unreachable!()
                };

                self.with_rib(Rib::default(), |this| {
                    value.visit(this);
                    ty.visit(this);
                });
            }
            DefKind::Function => {
                let ItemKind::Fn(fun) = &item.kind else {
                    unreachable!()
                };

                self.resolve_fn(fun);
            }
            DefKind::Struct => {
                let ItemKind::Struct { fields, items, .. } = &item.kind else {
                    unreachable!()
                };

                self.with_rib(Rib::default(), |this| {
                    let self_sym = this.resolver.interner.intern("Self");
                    let rib = this.ribs.last_mut().expect("rib exists");
                    rib.bindings.insert(self_sym, Res::Def(def_id));

                    for field in fields {
                        field.1.visit(this);
                    }

                    this.resolve_assoc_items(items);
                });
            }
            DefKind::Interface => {
                let ItemKind::Interface { items, .. } = &item.kind else {
                    unreachable!()
                };

                self.with_rib(Rib::default(), |this| {
                    let self_sym = this.resolver.interner.intern("Self");
                    let rib = this.ribs.last_mut().expect("rib exists");
                    rib.bindings.insert(self_sym, Res::Def(def_id));

                    this.resolve_assoc_items(items);
                });
            }
        }

        VisitAction::SkipChildren
    }
}
