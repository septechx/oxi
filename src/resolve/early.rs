use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{Item, ItemKind};
use crate::hir::DefId;
use crate::hir::interner::Symbol;
use crate::resolve::{Def, DefKind, Resolver};

impl<'a> Resolver<'a> {
    pub fn create_def(&mut self, name: Symbol, kind: DefKind) -> DefId {
        let idx = self.defs.len() as u32;
        self.defs.push(Def { name, kind });
        DefId(idx)
    }

    pub fn collect_definitions(&mut self) {
        for ast in self.asts {
            ast.visit(&mut DefCollector::new(self));
        }
    }

    pub fn resolve_imports(&mut self) {}
}

#[derive(Debug)]
struct DefCollector<'a, 'res> {
    resolver: &'a mut Resolver<'res>,
}

impl<'a, 'res> DefCollector<'a, 'res> {
    pub fn new(resolver: &'a mut Resolver<'res>) -> Self {
        Self { resolver }
    }
}

impl<'a, 'res> Visitor for DefCollector<'a, 'res> {
    fn visit_item(&mut self, item: &Item) -> VisitAction {
        match &item.kind {
            ItemKind::Static { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver.create_def(sym, DefKind::Static);
            }
            ItemKind::Struct { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver.create_def(sym, DefKind::Struct);
            }
            ItemKind::Interface { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver.create_def(sym, DefKind::Interface);
            }
            ItemKind::Fn(f) => {
                let sym = self.resolver.interner.intern(&f.name.value);
                self.resolver.create_def(sym, DefKind::Function);
            }
            _ => {}
        }
        VisitAction::Continue
    }
}
