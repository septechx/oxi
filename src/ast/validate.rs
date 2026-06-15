use crate::{
    ast::{
        AssocItem, AssocItemKind, Ast, Block, Expr, ExprKind, Fn, Ident, Item, ItemKind, Stmt,
        StmtKind,
        visit::{VisitAction, Visitable, Visitor},
    },
    context::{with_ctx, with_ctx_mut},
    errors::{
        builders,
        widgets::{CodeWidget, HighlightType, InfoWidget, LocationWidget},
    },
    hashmap::FxHashMap,
    hir::ModuleId,
    lexer::token::TokenKind,
};

struct AstValidator {
    module_id: ModuleId,
    in_function: bool,
    in_loop: bool,
    in_loop_body: bool,
    is_top_level: bool,
    in_interface: bool,
}

impl AstValidator {
    fn check_duplicate_names<'a, I>(&self, names: I, context: &str)
    where
        I: IntoIterator<Item = &'a Ident>,
    {
        let mut seen = FxHashMap::default();
        for ident in names {
            if let Some(first_span) = seen.insert(&ident.value, ident.span) {
                let ident_str = with_ctx(|ctx| ctx.interner.lookup(ident.value).to_string());
                let msg = format!("Duplicate definition of `{}` in {}", ident_str, context);

                let err = {
                    let loc_widget = LocationWidget::new(ident.span, self.module_id)
                        .expect("failed to get source location");
                    let code_widget =
                        CodeWidget::new(ident.span, self.module_id, HighlightType::Error)
                            .expect("failed to get source location");
                    let ident_str = with_ctx(|ctx| ctx.interner.lookup(ident.value).to_string());
                    let info_widget = InfoWidget::new(
                        first_span,
                        self.module_id,
                        format!("First definition of `{}` here", ident_str),
                    )
                    .expect("failed to get source location");
                    let first_loc_widget = LocationWidget::new(first_span, self.module_id)
                        .expect("failed to get source location");
                    let first_code_widget =
                        CodeWidget::new(first_span, self.module_id, HighlightType::Info)
                            .expect("failed to get source location");

                    builders::error(msg)
                        .add_widget(loc_widget)
                        .add_widget(code_widget)
                        .add_widget(info_widget)
                        .add_widget(first_loc_widget)
                        .add_widget(first_code_widget)
                };

                crate::CTX.with_borrow_mut(|ctx| {
                    let enable_printing = ctx.enable_printing;
                    ctx.errors.add(err, enable_printing);
                });
            }
        }
    }

    fn validate_fn_decl(&mut self, f: &Fn) {
        self.check_duplicate_names(f.parameters.iter().map(|a| &a.0), "function parameters");

        if f.is_extern {
            if f.body.is_some() {
                with_ctx_mut(|ctx| {
                    let enable_printing = ctx.enable_printing;
                    ctx.errors.add(
                        builders::error_at(
                            "Extern functions cannot have a body",
                            self.module_id,
                            f.name.span,
                            ctx,
                        ),
                        enable_printing,
                    );
                });
            }
        } else if f.body.is_none() && !self.in_interface {
            with_ctx_mut(|ctx| {
                let enable_printing = ctx.enable_printing;
                ctx.errors.add(
                    builders::error_at(
                        "Non-extern function must have a body",
                        self.module_id,
                        f.name.span,
                        ctx,
                    ),
                    enable_printing,
                );
            });
        }

        if let Some(body) = &f.body {
            let old_in_function = self.in_function;
            let old_top_level = self.is_top_level;
            let old_in_loop_body = self.in_loop_body;
            self.in_function = true;
            self.is_top_level = false;
            self.in_loop_body = false;
            self.validate_block(body);
            self.in_function = old_in_function;
            self.is_top_level = old_top_level;
            self.in_loop_body = old_in_loop_body;
        }
    }

    fn validate_assoc_item(&mut self, item: &AssocItem) {
        match &item.kind {
            AssocItemKind::Fn(f) => self.validate_fn_decl(f),
        }
    }

    fn validate_block(&mut self, block: &Block) {
        let stmts = &block.stmts;
        let len = stmts.len();

        for (i, stmt) in stmts.iter().enumerate() {
            if matches!(stmt.kind, StmtKind::Expr(_)) {
                if self.in_loop_body {
                    with_ctx_mut(|ctx| {
                        let enable_printing = ctx.enable_printing;
                        ctx.errors.add(
                            builders::error_at(
                                "Expression without semicolon is not allowed in loop bodies",
                                self.module_id,
                                stmt.span,
                                ctx,
                            ),
                            enable_printing,
                        );
                    });
                } else if i != len - 1 {
                    with_ctx_mut(|ctx| {
                        let enable_printing = ctx.enable_printing;
                        ctx.errors.add(
                            builders::error_at(
                                "Expression without semicolon must be at the end of a block",
                                self.module_id,
                                stmt.span,
                                ctx,
                            ),
                            enable_printing,
                        );
                    });
                }
            }
        }

        for stmt in stmts.iter() {
            stmt.visit(self);
        }
    }

    fn is_lvalue(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Symbol(_) | ExprKind::MemberAccess { .. } => true,
            ExprKind::Postfix { operator, .. } if operator.kind == TokenKind::At => true,
            _ => false,
        }
    }
}

impl Visitor for AstValidator {
    fn visit_item(&mut self, item: &Item) -> VisitAction {
        match &item.kind {
            ItemKind::Fn(f) => {
                self.validate_fn_decl(f);
                VisitAction::SkipChildren
            }
            ItemKind::Impl { items, .. } => {
                let old_top_level = self.is_top_level;
                self.is_top_level = false;
                for item in items.iter() {
                    self.validate_assoc_item(item);
                }
                self.is_top_level = old_top_level;
                VisitAction::SkipChildren
            }
            ItemKind::Struct { fields, items, .. } => {
                self.check_duplicate_names(fields.iter().map(|f| &f.0), "struct fields");
                self.check_duplicate_names(
                    items.iter().map(|item| match &item.kind {
                        AssocItemKind::Fn(f) => &f.name,
                    }),
                    "struct methods",
                );

                let old_top_level = self.is_top_level;
                self.is_top_level = false;
                for item in items.iter() {
                    self.validate_assoc_item(item);
                }
                self.is_top_level = old_top_level;

                VisitAction::SkipChildren
            }
            ItemKind::Interface { items, .. } => {
                self.check_duplicate_names(
                    items.iter().map(|item| match &item.kind {
                        AssocItemKind::Fn(f) => &f.name,
                    }),
                    "interface methods",
                );

                let old_top_level = self.is_top_level;
                let old_in_interface = self.in_interface;
                self.is_top_level = false;
                self.in_interface = true;
                for item in items.iter() {
                    self.validate_assoc_item(item);
                }
                self.is_top_level = old_top_level;
                self.in_interface = old_in_interface;

                VisitAction::SkipChildren
            }
            ItemKind::Const { .. } => VisitAction::Continue,
            ItemKind::Import(_) => VisitAction::Continue,
            ItemKind::Module { body, .. } => {
                if let Some(items) = body {
                    let mut names = Vec::new();
                    for item in items.iter() {
                        match &item.kind {
                            ItemKind::Fn(f) => names.push(&f.name),
                            ItemKind::Struct { name, .. } => names.push(name),
                            ItemKind::Interface { name, .. } => names.push(name),
                            ItemKind::Const { name, .. } => names.push(name),
                            ItemKind::Module { name, .. } => names.push(name),
                            ItemKind::Impl { .. } | ItemKind::Import(_) => {}
                        }
                    }
                    self.check_duplicate_names(names, "module scope");

                    let old_top_level = self.is_top_level;
                    self.is_top_level = true;
                    body.visit(self);
                    self.is_top_level = old_top_level;
                    VisitAction::SkipChildren
                } else {
                    VisitAction::SkipChildren
                }
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> VisitAction {
        match &stmt.kind {
            StmtKind::Let {
                name: _,
                ty: _,
                value,
                mutability: _,
            } => {
                if let Some(val) = value {
                    val.visit(self);
                }

                VisitAction::SkipChildren
            }
            _ => VisitAction::Continue,
        }
    }

    fn visit_expr(&mut self, expr: &Expr) -> VisitAction {
        match &expr.kind {
            ExprKind::StructInstantiation { path: _, fields } => {
                let mut seen = FxHashMap::default();
                for (ident, val) in fields.iter() {
                    if let Some(first_span) = seen.insert(&ident.value, ident.span) {
                        let ident_str =
                            with_ctx(|ctx| ctx.interner.lookup(ident.value).to_string());
                        let msg =
                            format!("Duplicate field `{}` in struct instantiation", ident_str);

                        let err = {
                            let loc_widget = LocationWidget::new(ident.span, self.module_id)
                                .expect("failed to get source location");
                            let code_widget =
                                CodeWidget::new(ident.span, self.module_id, HighlightType::Error)
                                    .expect("failed to get source location");
                            let info_widget = InfoWidget::new(
                                first_span,
                                self.module_id,
                                with_ctx(|ctx| {
                                    format!(
                                        "First initialization of `{}` here",
                                        ctx.interner.lookup(ident.value)
                                    )
                                }),
                            )
                            .expect("failed to get source location");
                            let first_loc_widget = LocationWidget::new(first_span, self.module_id)
                                .expect("failed to get source location");
                            let first_code_widget =
                                CodeWidget::new(first_span, self.module_id, HighlightType::Info)
                                    .expect("failed to get source location");

                            builders::error(msg)
                                .add_widget(loc_widget)
                                .add_widget(code_widget)
                                .add_widget(info_widget)
                                .add_widget(first_loc_widget)
                                .add_widget(first_code_widget)
                        };

                        crate::CTX.with_borrow_mut(|ctx| {
                            let enable_printing = ctx.enable_printing;
                            ctx.errors.add(err, enable_printing);
                        });
                    }
                    val.visit(self);
                }

                VisitAction::SkipChildren
            }
            ExprKind::Block(b) => {
                let old_top_level = self.is_top_level;
                let old_in_loop_body = self.in_loop_body;
                self.is_top_level = false;
                self.in_loop_body = false;
                self.validate_block(b);
                self.is_top_level = old_top_level;
                self.in_loop_body = old_in_loop_body;

                VisitAction::SkipChildren
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.visit(self);
                let old_in_loop_body = self.in_loop_body;
                self.in_loop_body = false;
                self.validate_block(then_branch);
                if let Some(else_expr) = else_branch {
                    else_expr.visit(self);
                }
                self.in_loop_body = old_in_loop_body;
                VisitAction::SkipChildren
            }
            ExprKind::While { condition, body } => {
                condition.visit(self);
                let old_in_loop = self.in_loop;
                let old_in_loop_body = self.in_loop_body;
                self.in_loop = true;
                self.in_loop_body = true;
                self.validate_block(body);
                self.in_loop = old_in_loop;
                self.in_loop_body = old_in_loop_body;
                VisitAction::SkipChildren
            }
            ExprKind::Loop(body) => {
                let old_in_loop = self.in_loop;
                let old_in_loop_body = self.in_loop_body;
                self.in_loop = true;
                self.in_loop_body = true;
                self.validate_block(body);
                self.in_loop = old_in_loop;
                self.in_loop_body = old_in_loop_body;
                VisitAction::SkipChildren
            }
            ExprKind::Break(_) => {
                if !self.in_loop {
                    with_ctx_mut(|ctx| {
                        let enable_printing = ctx.enable_printing;
                        ctx.errors.add(
                            builders::error_at(
                                "Break statement outside of loop",
                                self.module_id,
                                expr.span,
                                ctx,
                            ),
                            enable_printing,
                        );
                    });
                }
                VisitAction::Continue
            }
            ExprKind::Return(_) => {
                if !self.in_function {
                    with_ctx_mut(|ctx| {
                        let enable_printing = ctx.enable_printing;
                        ctx.errors.add(
                            builders::error_at(
                                "Return statement outside of function",
                                self.module_id,
                                expr.span,
                                ctx,
                            ),
                            enable_printing,
                        );
                    });
                }
                VisitAction::Continue
            }
            ExprKind::Assignment { assignee, .. } => {
                if !Self::is_lvalue(assignee) {
                    with_ctx_mut(|ctx| {
                        ctx.errors.add(
                            builders::error_at(
                                "Invalid assignment target",
                                self.module_id,
                                assignee.span,
                                ctx,
                            ),
                            ctx.enable_printing,
                        );
                    });
                }
                VisitAction::Continue
            }

            _ => VisitAction::Continue,
        }
    }
}

pub fn validate_ast(ast: &Ast, module_id: ModuleId) {
    let mut validator = AstValidator {
        module_id,
        in_function: false,
        in_loop: false,
        in_loop_body: false,
        is_top_level: true,
        in_interface: false,
    };

    let mut top_level_names = Vec::new();
    for item in ast.items.iter() {
        match &item.kind {
            ItemKind::Fn(f) => top_level_names.push(&f.name),
            ItemKind::Struct { name, .. } => top_level_names.push(name),
            ItemKind::Interface { name, .. } => top_level_names.push(name),
            ItemKind::Const { name, .. } => top_level_names.push(name),
            ItemKind::Module { name, .. } => top_level_names.push(name),
            ItemKind::Impl { .. } | ItemKind::Import(_) => {}
        }
    }
    validator.check_duplicate_names(top_level_names, "module scope");

    ast.visit(&mut validator);
}
