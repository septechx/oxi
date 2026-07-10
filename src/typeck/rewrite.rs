use crate::hashmap::FxHashMap;
use crate::hir::{BodyId, Expr, ExprKind, HirId, MaybeOwner, QPath, Stmt, StmtKind};
use crate::span::Span;
use crate::typeck::{MemberRes, Typeck};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(super) fn rewrite_member_access(&mut self) {
        for owner in self.krate.owners.iter_mut() {
            let MaybeOwner::Owner(info) = owner else {
                continue;
            };
            let body_ids: Vec<BodyId> = info.nodes.bodies.keys().copied().collect();
            for body_id in body_ids {
                let body = info.nodes.bodies.get_mut(&body_id).expect("body exists");
                rewrite_expr(&mut body.value, &self.member_res);
            }
        }
    }
}

fn rewrite_expr(expr: &mut Expr, member_res: &FxHashMap<HirId, MemberRes>) {
    let res = member_res.get(&expr.hir_id).copied();
    match &mut expr.kind {
        ExprKind::MemberAccess { base, member } => {
            rewrite_expr(base, member_res);
            if let Some(MemberRes::Field { index }) = res {
                let base = std::mem::replace(
                    base,
                    Expr {
                        hir_id: expr.hir_id,
                        kind: ExprKind::Error,
                        span: Span::new(0, 0),
                    }
                    .into_box(),
                );
                expr.kind = ExprKind::Field {
                    base,
                    field: *member,
                    index,
                }
            }
        }
        ExprKind::Call { callee, params } => {
            let res = member_res.get(&callee.hir_id).copied();
            if let Some(MemberRes::Method { def_id, .. }) = res {
                let (mut base_owned, member_sym) =
                    match std::mem::replace(&mut callee.kind, ExprKind::Error) {
                        ExprKind::MemberAccess { base, member } => (base, member),
                        ExprKind::Path(QPath::TypeRelative { qself, segment }) => {
                            let receiver = Expr {
                                hir_id: callee.hir_id,
                                kind: ExprKind::Path(*qself),
                                span: callee.span,
                            }
                            .into_box();
                            (receiver, segment.value)
                        }
                        _ => unreachable!(),
                    };
                rewrite_expr(&mut base_owned, member_res);
                let mut params_owned = std::mem::take(params);
                for param in &mut params_owned {
                    rewrite_expr(param, member_res);
                }
                expr.kind = ExprKind::MethodCall {
                    receiver: base_owned,
                    method: member_sym,
                    params: params_owned,
                    def_id,
                };
            } else {
                rewrite_expr(callee, member_res);
                for param in params {
                    rewrite_expr(param, member_res);
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            rewrite_expr(left, member_res);
            rewrite_expr(right, member_res);
        }
        ExprKind::StructInit { fields, .. } => {
            for (_, field) in fields {
                rewrite_expr(field, member_res);
            }
        }
        ExprKind::ArrayInit { contents, .. } => {
            for expr in contents {
                rewrite_expr(expr, member_res);
            }
        }
        ExprKind::TupleInit(elements) => {
            for element in elements {
                rewrite_expr(element, member_res);
            }
        }
        ExprKind::Block(block) => {
            for stmt in &mut block.stmts {
                rewrite_stmt(stmt, member_res);
            }
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            rewrite_expr(cond, member_res);
            for stmt in &mut then_branch.stmts {
                rewrite_stmt(stmt, member_res);
            }
            if let Some(else_branch) = else_branch {
                rewrite_expr(else_branch, member_res);
            }
        }
        ExprKind::Loop(block) => {
            for stmt in &mut block.stmts {
                rewrite_stmt(stmt, member_res);
            }
        }
        ExprKind::Break(expr) | ExprKind::Return(expr) => {
            if let Some(expr) = expr {
                rewrite_expr(expr, member_res);
            }
        }
        ExprKind::Assign { target, value, .. } => {
            rewrite_expr(target, member_res);
            rewrite_expr(value, member_res);
        }
        ExprKind::Dereference { expr } => {
            rewrite_expr(expr, member_res);
        }
        ExprKind::Reference { expr, .. } => {
            rewrite_expr(expr, member_res);
        }
        ExprKind::Unary { right: expr, .. } => {
            rewrite_expr(expr, member_res);
        }
        ExprKind::As { expr, .. } => {
            rewrite_expr(expr, member_res);
        }
        ExprKind::MethodCall {
            receiver, params, ..
        } => {
            rewrite_expr(receiver, member_res);
            for param in params {
                rewrite_expr(param, member_res);
            }
        }
        ExprKind::Field { base, .. } => {
            rewrite_expr(base, member_res);
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::Error => {}
    }
}

fn rewrite_stmt(stmt: &mut Stmt, member_res: &FxHashMap<HirId, MemberRes>) {
    match &mut stmt.kind {
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
            rewrite_expr(expr, member_res);
        }
        StmtKind::Let { init, .. } => {
            if let Some(init) = init {
                rewrite_expr(init, member_res);
            }
        }
    }
}
