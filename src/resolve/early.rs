use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{ImportTree, Item, ItemKind, Visibility};
use crate::hir::interner::Symbol;
use crate::hir::{DefId, ModuleId};
use crate::resolve::{Def, DefKind, PendingImport, Resolver};

impl<'a> Resolver<'a> {
    fn create_def(&mut self, name: Symbol, kind: DefKind) -> DefId {
        let idx = self.defs[self.module_idx].len() as u32;
        self.defs[self.module_idx].push(Def { name, kind });
        DefId(idx)
    }

    fn register_import(&mut self, import_item: ImportTree, visibility: Visibility) {
        self.pending_imports.push(PendingImport {
            import_item,
            visibility,
            module: ModuleId(self.module_idx as u32),
        });
    }

    pub fn collect_definitions(&mut self) {
        for (i, ast) in self.asts.iter().enumerate() {
            self.module_idx = i;
            ast.visit(&mut DefCollector::new(self));
        }
    }

    pub fn build_graph(&mut self) {
        for (i, ast) in self.asts.iter().enumerate() {
            self.module_idx = i;
            ast.visit(&mut ImportCollector::new(self));
        }
    }
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
        VisitAction::SkipChildren
    }
}

#[derive(Debug)]
struct ImportCollector<'a, 'res> {
    resolver: &'a mut Resolver<'res>,
}

impl<'a, 'res> ImportCollector<'a, 'res> {
    pub fn new(resolver: &'a mut Resolver<'res>) -> Self {
        Self { resolver }
    }
}

impl<'a, 'res> Visitor for ImportCollector<'a, 'res> {
    fn visit_item(&mut self, item: &Item) -> VisitAction {
        if let ItemKind::Import(tree) = &item.kind {
            self.resolver.register_import(tree.clone(), item.visibility);
        }
        VisitAction::SkipChildren
    }
}
