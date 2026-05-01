use thin_vec::ThinVec;

use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{ImportTree, ImportTreeKind, Item, ItemKind, Visibility};
use crate::hir::interner::Symbol;
use crate::hir::{DefId, ModuleId};
use crate::resolve::{Def, DefKind, PendingImport, Resolver};

impl<'a> Resolver<'a> {
    fn create_def(&mut self, name: Symbol, kind: DefKind, visibility: Visibility) -> DefId {
        let idx = self.defs[self.module_idx].len() as u32;
        self.defs[self.module_idx].push(Def {
            name,
            kind,
            visibility,
        });
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

    pub fn resolve_imports(&mut self) {
        let mut progress = true;
        while progress && !self.pending_imports.is_empty() {
            progress = false;

            // iterate with index so we can remove resolved entries in-place
            let mut i = 0usize;
            while i < self.pending_imports.len() {
                // try resolve; if resolved we remove from pending and set progress = true
                match self.try_resolve_import(i) {
                    ResolutionStatus::Resolved => {
                        self.pending_imports.swap_remove(i);
                        progress = true;
                        continue;
                    }
                    ResolutionStatus::Failed => {
                        self.pending_imports.swap_remove(i);
                        progress = true;
                        continue;
                    }
                    ResolutionStatus::Pending => {
                        // cannot resolve yet: keep item for next pass
                        i += 1;
                        continue;
                    }
                }
            }
        }

        if !self.pending_imports.is_empty() {
            for pi in &self.pending_imports {
                let segments: ThinVec<String> = pi
                    .import_item
                    .prefix
                    .segments
                    .iter()
                    .map(|ident| ident.value.to_string())
                    .collect();
                let path = segments.join("::");
                println!(
                    "Could not resolve import `{}` in module `{}`",
                    path, self.asts[pi.module.0 as usize].name
                );
            }
        }
    }

    fn try_resolve_import(&mut self, idx: usize) -> ResolutionStatus {
        let pi = &self.pending_imports[idx];

        ResolutionStatus::Failed
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
                self.resolver
                    .create_def(sym, DefKind::Static, item.visibility);
            }
            ItemKind::Struct { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver
                    .create_def(sym, DefKind::Struct, item.visibility);
            }
            ItemKind::Interface { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver
                    .create_def(sym, DefKind::Interface, item.visibility);
            }
            ItemKind::Fn(f) => {
                let sym = self.resolver.interner.intern(&f.name.value);
                self.resolver
                    .create_def(sym, DefKind::Function, item.visibility);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// Successfully resolved and applied to the module
    Resolved,
    /// Failed permanently
    Failed,
    /// Temporary failure (might succeed in later pass)
    Pending,
}
