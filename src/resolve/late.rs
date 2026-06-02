use thin_vec::ThinVec;

use crate::ast::visit::{VisitAction, Visitable, Visitor};
use crate::ast::{
    AssocItem, AssocItemKind, Expr, ExprKind, Fn, Item, ItemKind, NodeId, Path, Stmt, StmtKind,
    Type, TypeKind,
};
use crate::errors::builders;
use crate::errors::widgets::{CodeWidget, HighlightType, LocationWidget};
use crate::hashmap::FxHashMap;
use crate::hir::{DefId, DefKind};
use crate::interner::{Symbol, sym};
use crate::resolve::path::PathError;
use crate::resolve::{NameBinding, PartialRes, PrimTy, Res, Resolver};
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

                let sym = arg.0.value;
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

    fn resolve_path(&mut self, path: &Path, node_id: NodeId) -> PartialRes {
        let segments = &path.segments;
        let partial_res = if segments.len() == 1 {
            PartialRes::new(self.resolve_ident(path, segments[0].value))
        } else if let Some(partial_res) = self.resolve_type_relative_path(path) {
            partial_res
        } else {
            self.resolve_module_path(path)
        };
        self.resolver.res_map.insert(node_id, partial_res);
        partial_res
    }

    fn resolve_ident(&mut self, path: &Path, name: Symbol) -> Res {
        // Check if the symbol is a primitive type.
        if let Some(prim) = PrimTy::from_name(name) {
            return Res::PrimTy(prim);
        }

        // Check if the symbol is a local variable.
        let mut depth = 1;
        while depth <= self.ribs.len() {
            let rib_index = self.ribs.len() - depth;

            if let Some(&res) = self.ribs[rib_index].bindings.get(&name) {
                return res;
            }

            if name != sym::Self_ && self.ribs[rib_index].kind.blocks_enclosing_locals() {
                break;
            }

            depth += 1;
        }

        // Check if the symbol is a module-level definition.
        if let Some(res) = self.resolver.current_module().resolutions.get(&name) {
            return Res::Def(res.best_binding().def_id);
        }

        self.report_path_error(path, "Failed to resolve path");
        Res::Err
    }

    fn resolve_type_relative_path(&mut self, path: &Path) -> Option<PartialRes> {
        let segments = &path.segments;
        let seg_count = segments.len();

        // First segment as a type in the current module.
        let first_sym = segments[0].value;
        if let Some(resolution) = self.resolver.current_module().resolutions.get(&first_sym) {
            let type_def = resolution.best_binding().def_id;
            if self.is_type_def(type_def) && seg_count >= 2 {
                return Some(self.resolve_assoc_segments(type_def, 0, &segments[1..], path));
            }
        }

        // Longest module prefix, then type, then associated item.
        for prefix_len in (1..seg_count).rev() {
            let module_prefix = &segments[..prefix_len];
            let type_seg = &segments[prefix_len];
            let module_node_idx = self
                .resolver
                .resolve_module_path(self.resolver.module_idx, module_prefix)
                .ok()?;
            let type_resolution = self.resolver.modules[module_node_idx]
                .resolutions
                .get(&type_seg.value)?;
            let type_def = type_resolution.best_binding().def_id;
            if !self.is_type_def(type_def) {
                continue;
            }
            if prefix_len + 1 >= seg_count {
                continue;
            }
            return Some(self.resolve_assoc_segments(
                type_def,
                module_node_idx,
                &segments[prefix_len + 1..],
                path,
            ));
        }

        None
    }

    fn resolve_assoc_segments(
        &mut self,
        mut type_def: DefId,
        module_node_idx: usize,
        segments: &[crate::ast::Ident],
        path: &Path,
    ) -> PartialRes {
        let mut base_res = Res::Def(type_def);
        for (i, segment) in segments.iter().enumerate() {
            let method_sym = segment.value;
            let Some(binding) = self.lookup_struct_method(type_def, method_sym, module_node_idx)
            else {
                let msg = format!(
                    "No associated item `{}` on `{}`",
                    self.resolver.ctx.interner.lookup(segment.value),
                    self.resolver.ctx.interner.lookup(
                        self.resolver.defs[type_def.0 as usize]
                            .name
                            .expect("type has a name"),
                    ),
                );
                self.report_path_error_at(segment.span, &msg);
                return PartialRes::new(Res::Err);
            };
            if !self.check_assoc_visibility(binding, module_node_idx, segment.span) {
                return PartialRes::new(Res::Err);
            }
            base_res = Res::Def(binding.def_id);
            if i + 1 == segments.len() {
                return PartialRes::new(base_res);
            }
            type_def = binding.def_id;
            if !self.is_type_def(type_def) {
                self.report_path_error(
                    path,
                    &format!(
                        "`{}` is not a type",
                        self.resolver.ctx.interner.lookup(segment.value)
                    ),
                );
                return PartialRes::new(Res::Err);
            }
        }
        PartialRes::new(base_res)
    }

    fn lookup_struct_method(
        &self,
        struct_def: DefId,
        method_sym: Symbol,
        module_node_idx: usize,
    ) -> Option<NameBinding> {
        self.resolver.modules[module_node_idx]
            .struct_methods
            .get(&struct_def)?
            .get(&method_sym)
            .copied()
    }

    fn check_assoc_visibility(
        &mut self,
        binding: NameBinding,
        defining_module: usize,
        span: Span,
    ) -> bool {
        if defining_module == self.resolver.module_idx {
            return true;
        }
        if binding.visibility == crate::ast::Visibility::Public {
            return true;
        }
        let module_id = self.resolver.source_module_id();
        let loc_widget = LocationWidget::new_with_ctx(span, module_id, self.resolver.ctx)
            .expect("failed to create error");
        let code_widget =
            CodeWidget::new_with_ctx(span, module_id, HighlightType::Error, self.resolver.ctx)
                .expect("failed to create error");
        let enable_printing = self.resolver.ctx.enable_printing;
        self.resolver.ctx.errors.add(
            builders::error("Associated item is private")
                .add_widget(loc_widget)
                .add_widget(code_widget),
            enable_printing,
        );
        false
    }

    fn resolve_module_path(&mut self, path: &Path) -> PartialRes {
        let segments = &path.segments;
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
                self.report_path_error_at(span, &msg);
                return PartialRes::new(Res::Err);
            }
        };

        let last = &segments[segments.len() - 1];
        let sym = last.value;

        if let Some(res) = self.resolver.modules[module_node_idx].resolutions.get(&sym) {
            let res = res.best_binding().def_id;
            if self.is_type_def(res) && segments.len() > 1 {
                return PartialRes::with_unresolved_segments(Res::Def(res), 0);
            }
            return PartialRes::new(Res::Def(res));
        }

        let err_str = format!(
            "Failed to resolve `{}` in module `{}`",
            self.resolver.ctx.interner.lookup(last.value),
            Path {
                segments: segments[..segments.len() - 1].into(),
                span: Span::new(0, 0),
            }
            .display(self.resolver.ctx)
        );
        self.report_path_error_at(last.span, &err_str);
        PartialRes::new(Res::Err)
    }

    fn is_type_def(&self, def_id: DefId) -> bool {
        matches!(
            self.resolver.defs.get(def_id.0 as usize).map(|d| d.kind),
            Some(DefKind::Struct | DefKind::Interface)
        )
    }

    fn register_impl_methods(&mut self, struct_def_id: DefId, items: &ThinVec<AssocItem>) {
        for item in items {
            let AssocItemKind::Fn(f) = &item.kind;
            let Some(method_def_id) = self.resolver.def_id_for_node(item.node_id) else {
                continue;
            };
            let binding = NameBinding {
                def_id: method_def_id,
                visibility: item.visibility,
            };
            self.resolver
                .current_module_mut()
                .struct_methods
                .entry(struct_def_id)
                .or_default()
                .insert(f.name.value, binding);
        }
    }

    fn report_path_error(&mut self, path: &Path, msg: &str) {
        self.report_path_error_at(path.span, msg);
    }

    fn report_path_error_at(&mut self, span: Span, msg: &str) {
        let module_id = self.resolver.source_module_id();
        let loc_widget = LocationWidget::new_with_ctx(span, module_id, self.resolver.ctx)
            .expect("failed to create error");
        let code_widget =
            CodeWidget::new_with_ctx(span, module_id, HighlightType::Error, self.resolver.ctx)
                .expect("failed to create error");
        let enable_printing = self.resolver.ctx.enable_printing;
        self.resolver.ctx.errors.add(
            builders::error(msg)
                .add_widget(loc_widget)
                .add_widget(code_widget),
            enable_printing,
        );
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

                if let Some(Res::Def(struct_def_id)) = self_ty_res.full_res() {
                    self.register_impl_methods(struct_def_id, items);
                    self.with_rib(RibKind::Item, |this| {
                        this.inject_self_ty_from_def_id(struct_def_id);
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

                let sym = name.value;
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
