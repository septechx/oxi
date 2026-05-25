use thin_vec::ThinVec;

use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{
    AssocItem, AssocItemKind, Expr, ExprKind, Fn, Item, ItemKind, NodeId, Path, Stmt, StmtKind,
    Type, TypeKind,
};
use crate::errors::builders;
use crate::errors::widgets::{CodeWidget, HighlightType, LocationWidget};
use crate::hashmap::FxHashMap;
use crate::hir::DefId;
use crate::hir::interner::Symbol;
use crate::resolve::path::PathError;
use crate::resolve::{PrimTy, Res, Resolver};
use crate::span::Span;

impl<'a, 'ctx> Resolver<'a, 'ctx> {
    pub(super) fn late_resolve(&mut self) {
        self.traverse_tree(|this, node_idx, items| {
            this.module_idx = node_idx;
            let mut visitor = LateResolutionVisitor::new(this);
            for item in items {
                item.visit(&mut visitor);
            }
        });
    }
}

#[derive(Debug, Default)]
struct Rib {
    pub bindings: FxHashMap<Symbol, Res>,
    pub kind: RibKind,
}

impl Rib {
    fn new(kind: RibKind) -> Self {
        Self {
            bindings: FxHashMap::default(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RibKind {
    /// No restrictions on which bindings may be used.
    #[default]
    Normal,
    /// We have entered an item definition. Block upvars.
    /// ```ignore
    /// fn main() void {
    ///     let a = 0;
    ///     fn b() void {
    ///         do_stuff(a); // Block usage of `a` here
    ///     }
    /// }
    /// ```
    Item,
}

impl RibKind {
    fn blocks_enclosing_locals(self) -> bool {
        self == RibKind::Item
    }
}

#[derive(Debug)]
struct LateResolutionVisitor<'a, 'res, 'ctx> {
    resolver: &'a mut Resolver<'res, 'ctx>,
    ribs: ThinVec<Rib>,
}

impl<'a, 'res, 'ctx> LateResolutionVisitor<'a, 'res, 'ctx> {
    pub fn new(resolver: &'a mut Resolver<'res, 'ctx>) -> Self {
        Self {
            resolver,
            ribs: ThinVec::new(),
        }
    }

    fn with_rib(&mut self, kind: RibKind, f: impl FnOnce(&mut Self)) {
        self.ribs.push(Rib::new(kind));
        f(self);
        self.ribs.pop();
    }

    fn resolve_fn(&mut self, fun: &Fn) {
        self.with_rib(RibKind::Item, |this| {
            for arg in &fun.parameters {
                arg.1.visit(this);

                let sym = this.resolver.ctx.interner.intern(&arg.0.value);
                let rib = this.ribs.last_mut().expect("rib exists");
                rib.bindings.insert(sym, Res::Local(arg.2));
            }
            fun.return_type.visit(this);
            if let Some(body) = &fun.body {
                body.visit(this);
            }
        });
    }

    fn resolve_assoc_items(&mut self, items: &ThinVec<AssocItem>) {
        for item in items {
            match &item.kind {
                AssocItemKind::Fn(fun) => self.resolve_fn(fun),
            }
        }
    }

    fn resolve_path(&mut self, path: &Path, node_id: NodeId) -> Res {
        let segments = &path.segments;
        if segments.len() == 1 {
            let value = &segments[0].value;
            let sym = self.resolver.ctx.interner.intern(value);

            // 1st check if path is a primitive type
            if let Some(prim) = PrimTy::from_name(sym) {
                let res = Res::PrimTy(prim);
                self.resolver.res_map.insert(node_id, res);
                return res;
            }

            // 2nd check in ribs if path is a local
            let mut depth = 1;
            while depth <= self.ribs.len() {
                let rib_index = self.ribs.len() - depth;

                if let Some(&res) = self.ribs[rib_index].bindings.get(&sym) {
                    self.resolver.res_map.insert(node_id, res);
                    return res;
                }

                if self.ribs[rib_index].kind.blocks_enclosing_locals() {
                    break;
                }

                depth += 1;
            }

            // 3rd check if path is a module level item
            if let Some(res) = self.resolver.current_module().resolutions.get(&sym) {
                let res = Res::Def(res.best_binding().def_id);
                self.resolver.res_map.insert(node_id, res);
                return res;
            };

            // Not found, emit error
            let module_id = self.resolver.source_module_id();
            let loc_widget = LocationWidget::new_with_ctx(path.span, module_id, self.resolver.ctx)
                .expect("failed to create error");
            let code_widget = CodeWidget::new_with_ctx(
                path.span,
                module_id,
                HighlightType::Error,
                self.resolver.ctx,
            )
            .expect("failed to create error");
            let enable_printing = self.resolver.ctx.enable_printing;
            self.resolver.ctx.errors.add(
                builders::error("Failed to resolve path")
                    .add_widget(loc_widget)
                    .add_widget(code_widget),
                enable_printing,
            );

            Res::Err
        } else {
            // Multi-segment path: walk the module tree using shared path resolution
            let module_prefix = &segments[..segments.len() - 1];
            let module_node_idx = match self
                .resolver
                .resolve_module_path(self.resolver.module_idx, module_prefix)
            {
                Ok(idx) => idx,
                Err(err) => {
                    let (span, msg) = match err {
                        PathError::NoParentForSuper { span } => {
                            (span, "No parent module for `super`".into())
                        }
                        PathError::ModuleNotFound { name, span } => {
                            (span, format!("Module `{name}` not found"))
                        }
                    };
                    let module_id = self.resolver.source_module_id();
                    let loc_widget =
                        LocationWidget::new_with_ctx(span, module_id, self.resolver.ctx)
                            .expect("failed to create error");
                    let code_widget = CodeWidget::new_with_ctx(
                        span,
                        module_id,
                        HighlightType::Error,
                        self.resolver.ctx,
                    )
                    .expect("failed to create error");
                    let enable_printing = self.resolver.ctx.enable_printing;
                    self.resolver.ctx.errors.add(
                        builders::error(msg)
                            .add_widget(loc_widget)
                            .add_widget(code_widget),
                        enable_printing,
                    );
                    return Res::Err;
                }
            };

            // Last segment: look up in the target module's resolutions
            let last = &segments[segments.len() - 1];
            let sym = self.resolver.ctx.interner.intern(&last.value);

            if let Some(res) = self.resolver.modules[module_node_idx].resolutions.get(&sym) {
                let res = Res::Def(res.best_binding().def_id);
                self.resolver.res_map.insert(node_id, res);
                return res;
            };

            // Error
            let module_id = self.resolver.source_module_id();
            let loc_widget = LocationWidget::new_with_ctx(last.span, module_id, self.resolver.ctx)
                .expect("failed to create error");
            let code_widget = CodeWidget::new_with_ctx(
                last.span,
                module_id,
                HighlightType::Error,
                self.resolver.ctx,
            )
            .expect("failed to create error");
            let enable_printing = self.resolver.ctx.enable_printing;
            self.resolver.ctx.errors.add(
                builders::error(format!(
                    "Failed to resolve `{}` in module `{}`",
                    last.value,
                    Path {
                        span: Span::new(0, 0),
                        segments: segments[..segments.len() - 1].into()
                    }
                ))
                .add_widget(loc_widget)
                .add_widget(code_widget),
                enable_printing,
            );

            Res::Err
        }
    }

    fn inject_self_ty(&mut self, node_id: NodeId) {
        let def_id = self.resolver.def_id_for_node(node_id).expect("resolved");
        self.inject_self_ty_from_def_id(def_id);
    }

    fn inject_self_ty_from_def_id(&mut self, def_id: DefId) {
        let self_sym = self.resolver.ctx.interner.intern("Self");
        let rib = self.ribs.last_mut().expect("rib exists");
        rib.bindings
            .insert(self_sym, Res::SelfTyAlias { alias_to: def_id });
    }
}

impl<'a, 'res, 'ctx> Visitor for LateResolutionVisitor<'a, 'res, 'ctx> {
    fn visit_item(&mut self, item: &Item) -> VisitAction {
        match &item.kind {
            // Already resolved in a past stage
            ItemKind::Import(_) | ItemKind::Module { .. } => {}
            ItemKind::Const { value, ty, .. } => {
                value.visit(self);
                ty.visit(self);
            }
            ItemKind::Fn(fun) => {
                self.resolve_fn(fun);
            }
            ItemKind::Struct { fields, items, .. } => {
                self.with_rib(RibKind::Item, |this| {
                    this.inject_self_ty(item.node_id);
                    for field in fields {
                        field.1.visit(this);
                    }
                    this.resolve_assoc_items(items);
                });
            }
            ItemKind::Interface { items, .. } => {
                self.with_rib(RibKind::Item, |this| {
                    this.inject_self_ty(item.node_id);
                    this.resolve_assoc_items(items);
                });
            }
            ItemKind::Impl {
                self_ty,
                interface,
                items,
            } => {
                let self_ty_res = self.resolve_path(&self_ty.0, self_ty.1);
                self.resolve_path(&interface.0, interface.1);

                if let Res::Def(def_id) = self_ty_res {
                    self.with_rib(RibKind::Item, |this| {
                        this.inject_self_ty_from_def_id(def_id);
                        this.resolve_assoc_items(items);
                    });
                }
            }
        }

        VisitAction::SkipChildren
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> VisitAction {
        match &stmt.kind {
            StmtKind::Let {
                name, ty, value, ..
            } => {
                ty.visit(self);
                if let Some(value) = value {
                    value.visit(self);
                }

                let sym = self.resolver.ctx.interner.intern(&name.value);
                let rib = self.ribs.last_mut().expect("rib exists");
                rib.bindings.insert(sym, Res::Local(stmt.node_id));
            }
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => expr.visit(self),
        }

        VisitAction::SkipChildren
    }

    fn visit_expr(&mut self, expr: &Expr) -> VisitAction {
        match &expr.kind {
            ExprKind::Symbol(path) => {
                self.resolve_path(path, expr.node_id);
            }
            ExprKind::Literal(_) => {}
            ExprKind::Binary { left, right, .. } => {
                left.visit(self);
                right.visit(self);
            }
            ExprKind::Postfix { left, .. } => left.visit(self),
            ExprKind::Prefix { right, .. } => right.visit(self),
            ExprKind::Assignment {
                assignee, value, ..
            } => {
                assignee.visit(self);
                value.visit(self);
            }
            ExprKind::StructInstantiation { fields, path } => {
                self.resolve_path(path, expr.node_id);
                for (_, expr) in fields {
                    expr.visit(self);
                }
            }
            ExprKind::ArrayLiteral {
                underlying,
                contents,
                ..
            } => {
                underlying.visit(self);
                for elem in contents {
                    elem.visit(self);
                }
            }
            ExprKind::FunctionCall { callee, parameters } => {
                callee.visit(self);
                parameters.visit(self);
            }
            ExprKind::MemberAccess { base, .. } => base.visit(self),
            ExprKind::As { expr, ty } => {
                expr.visit(self);
                ty.visit(self);
            }
            ExprKind::TupleLiteral { elements } => {
                for elem in elements {
                    elem.visit(self);
                }
            }
            ExprKind::Block(b) => {
                self.with_rib(RibKind::Normal, |this| {
                    b.visit(this);
                });
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.visit(self);
                self.with_rib(RibKind::Normal, |this| {
                    then_branch.visit(this);
                });
                if let Some(else_expr) = else_branch {
                    else_expr.visit(self);
                }
            }
            ExprKind::While { condition, body } => {
                condition.visit(self);
                self.with_rib(RibKind::Normal, |this| {
                    body.visit(this);
                });
            }
            ExprKind::Loop(b) => {
                self.with_rib(RibKind::Normal, |this| {
                    b.visit(this);
                });
            }
            ExprKind::Break(val) => {
                if let Some(expr) = val {
                    expr.visit(self);
                }
            }
            ExprKind::Return(val) => {
                if let Some(expr) = val {
                    expr.visit(self);
                }
            }
        }

        VisitAction::SkipChildren
    }

    fn visit_type(&mut self, ty: &Type) -> VisitAction {
        match &ty.kind {
            TypeKind::Symbol(path) => {
                self.resolve_path(path, ty.node_id);
            }
            TypeKind::Pointer(ty, _) => ty.visit(self),
            TypeKind::Slice(ty) => ty.visit(self),
            TypeKind::FixedArray(ty, _) => {
                ty.visit(self);
            }
            TypeKind::Function { params, ret } => {
                params.visit(self);
                ret.visit(self);
            }
            TypeKind::Tuple(elements) => {
                elements.visit(self);
            }
            TypeKind::Infer => {}
            TypeKind::Never => {}
        }

        VisitAction::SkipChildren
    }
}
