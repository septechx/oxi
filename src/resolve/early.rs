use thin_vec::ThinVec;

use crate::ast::visit::{VisitAction, Visitable, Visitor, VisitorMut};
use crate::ast::{Ast, Expr, ImportTree, ImportTreeKind, Item, ItemKind, NodeId, Stmt, Visibility};
use crate::error_at;
use crate::hir::interner::Symbol;
use crate::hir::{DefId, ModuleId};
use crate::resolve::{Def, DefKind, NameResolution, PendingImport, Resolver};

impl<'a> Resolver<'a> {
    pub fn resolve(&mut self) {
        self.collect_definitions();
        self.build_graph();
        self.resolve_imports();
    }

    pub fn assign_node_ids(asts: &mut ThinVec<Ast>) {
        let mut ass = NodeIdAssigner::new();
        for ast in asts.iter_mut() {
            ast.visit_mut(&mut ass);
        }
    }

    fn create_def(&mut self, id: NodeId, name: Symbol, kind: DefKind, visibility: Visibility) {
        let idx = self.defs.len() as u32;
        self.defs.push(Def {
            name,
            kind,
            visibility,
        });
        self.def_map.insert(id, DefId(idx));
        self.current_module_mut()
            .resolutions
            .insert(name, NameResolution::non_glob_import(DefId(idx)));
    }

    fn register_import(&mut self, import_item: ImportTree, visibility: Visibility) {
        self.pending_imports.push(PendingImport {
            import_item,
            visibility,
            module: ModuleId(self.module_idx as u32),
        });
    }

    fn collect_definitions(&mut self) {
        for (i, ast) in self.asts.iter().enumerate() {
            self.module_idx = i;
            ast.visit(&mut DefCollector::new(self));
        }
    }

    fn build_graph(&mut self) {
        for (i, ast) in self.asts.iter().enumerate() {
            self.module_idx = i;
            ast.visit(&mut ImportCollector::new(self));
        }
    }

    fn resolve_imports(&mut self) {
        let mut progress = true;
        while progress && !self.pending_imports.is_empty() {
            progress = false;

            // iterate with index so we can remove resolved entries in-place
            let mut i = 0usize;
            while i < self.pending_imports.len() {
                // try resolve; if resolved we remove from pending and set progress = true
                match self.resolve_import(i) {
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
                error_at!(
                    pi.import_item.span,
                    ModuleId(pi.module.0),
                    format!("Could not resolve import `{}`", path)
                );
            }
        }
    }

    fn resolve_import(&mut self, idx: usize) -> ResolutionStatus {
        let pi = &self.pending_imports[idx];

        match &pi.import_item.kind {
            ImportTreeKind::Simple(_) => self.resolve_simple_import(idx),
            _ => todo!(),
        }
    }

    fn resolve_simple_import(&mut self, idx: usize) -> ResolutionStatus {
        let pi = &self.pending_imports[idx];
        let prefix = &pi.import_item.prefix;
        let segments = &prefix.segments;
        let ImportTreeKind::Simple(rename) = &pi.import_item.kind else {
            unreachable!()
        };

        if segments.len() < 2 {
            // An import with 0 segments would fail parsing, so the code must be `import lib`
            error_at!(
                pi.import_item.span,
                ModuleId(pi.module.0),
                "Cannot import module"
            );
            return ResolutionStatus::Failed;
        }

        let local_name = rename
            .as_ref()
            .map(|r| r.value.as_ref())
            .unwrap_or(unsafe { segments.last().unwrap_unchecked().value.as_ref() });
        let local_sym = self.interner.intern(local_name);

        // FIXME: Only supports 2-segment paths, else ignores the middle part(s)
        let target_mod_name = &segments[0].value;
        let target_def_name = &segments[segments.len() - 1].value;

        let target_mid = self.asts.iter().position(|m| m.name == *target_mod_name);
        let Some(target_mid) = target_mid else {
            error_at!(
                pi.import_item.span,
                ModuleId(pi.module.0),
                "Module not found"
            );
            return ResolutionStatus::Failed;
        };

        let target_sym = self.interner.intern(target_def_name);
        let target = self.modules[target_mid]
            .resolutions
            .get(&target_sym)
            .copied();
        let Some(target) = target else {
            return ResolutionStatus::Pending;
        };

        let target_def = self.defs[target.best_binding().0 as usize];
        if target_def.visibility != Visibility::Public {
            error_at!(
                pi.import_item.span,
                ModuleId(pi.module.0),
                "Cannot import private item"
            );
            return ResolutionStatus::Failed;
        }

        if pi.visibility == Visibility::Public {
            todo!("Implement re-exporting imports");
        }

        self.current_module_mut()
            .resolutions
            .insert(local_sym, target);

        ResolutionStatus::Resolved
    }
}

#[derive(Debug)]
struct NodeIdAssigner {
    next_node_id: u32,
}

impl NodeIdAssigner {
    pub fn new() -> Self {
        Self { next_node_id: 0 }
    }

    fn next_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        NodeId(id)
    }
}

impl VisitorMut for NodeIdAssigner {
    fn visit_item(&mut self, item: &mut Item) -> VisitAction {
        item.node_id = self.next_node_id();
        VisitAction::Continue
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) -> VisitAction {
        stmt.node_id = self.next_node_id();
        VisitAction::Continue
    }

    fn visit_expr(&mut self, expr: &mut Expr) -> VisitAction {
        expr.node_id = self.next_node_id();
        VisitAction::Continue
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
            ItemKind::Const { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver
                    .create_def(item.node_id, sym, DefKind::Const, item.visibility);
            }
            ItemKind::Struct { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver
                    .create_def(item.node_id, sym, DefKind::Struct, item.visibility);
            }
            ItemKind::Interface { name, .. } => {
                let sym = self.resolver.interner.intern(&name.value);
                self.resolver
                    .create_def(item.node_id, sym, DefKind::Interface, item.visibility);
            }
            ItemKind::Fn(f) => {
                let sym = self.resolver.interner.intern(&f.name.value);
                self.resolver
                    .create_def(item.node_id, sym, DefKind::Function, item.visibility);
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
