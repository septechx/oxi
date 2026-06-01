use thin_vec::ThinVec;

use crate::ast::{self, NodeId};
use crate::errors::builders;
use crate::hir::types::{Expr, ExprKind, FromToken};
use crate::hir::{AstLoweringContext, DefId};
use crate::interner::Symbol;
use crate::lexer::token::Token;
use crate::resolve::Res;

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_expr(&mut self, expr: &ast::Expr) -> Expr {
        let hir_id = self.next_hir_id();
        let kind = self.lower_expr_kind(expr);
        Expr {
            hir_id,
            kind,
            span: expr.span,
        }
    }

    fn lower_expr_kind(&mut self, expr: &ast::Expr) -> ExprKind {
        match &expr.kind {
            ast::ExprKind::Symbol(path) => {
                let hir_path = self.lower_path(path, expr.node_id);
                ExprKind::Path(hir_path)
            }
            ast::ExprKind::Literal(lit) => ExprKind::Literal(*lit),
            ast::ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.lower_expr(left).into_box();
                let right = self.lower_expr(right).into_box();
                let Some(op) = self.lower_operator(operator, "binary") else {
                    return ExprKind::Error;
                };
                ExprKind::Binary { left, op, right }
            }
            ast::ExprKind::Prefix { operator, right } => {
                let right = self.lower_expr(right).into_box();
                let Some(op) = self.lower_operator(operator, "prefix") else {
                    return ExprKind::Error;
                };
                ExprKind::Prefix { op, right }
            }
            ast::ExprKind::Postfix { left, operator } => {
                let left = self.lower_expr(left).into_box();
                let Some(op) = self.lower_operator(operator, "postfix") else {
                    return ExprKind::Error;
                };
                ExprKind::Postfix { left, op }
            }
            ast::ExprKind::FunctionCall { callee, parameters } => {
                let callee = self.lower_expr(callee).into_box();
                let params: ThinVec<Expr> = parameters
                    .iter()
                    .map(|expr| self.lower_expr(expr))
                    .collect();
                ExprKind::Call { callee, params }
            }
            ast::ExprKind::StructInstantiation { fields, .. } => {
                let def = match self.resolve_def_id(&expr.node_id) {
                    Some(def_id) => def_id,
                    _ => return ExprKind::Error,
                };
                let fields: ThinVec<(Symbol, Expr)> = fields
                    .iter()
                    .map(|(name, expr)| (name.value, self.lower_expr(expr)))
                    .collect();
                ExprKind::StructInit { def, fields }
            }
            _ => todo!("Lowering of {:?} not yet implemented", expr.kind),
        }
    }

    fn resolve_def_id(&self, node_id: &NodeId) -> Option<DefId> {
        match self.resolver.res_map.get(node_id) {
            Some(Res::Def(def_id)) => Some(*def_id),
            Some(Res::SelfTyAlias { alias_to }) => Some(*alias_to),
            _ => None,
        }
    }

    fn lower_operator<T: FromToken<T>>(&mut self, op: &Token, kind: &str) -> Option<T> {
        T::from_token(op).or_else(|| {
            self.ctx.errors.add(
                builders::error_at(
                    format!("Invalid {kind} operator"),
                    op.module_id,
                    op.span,
                    self.ctx,
                ),
                self.ctx.enable_printing,
            );
            None
        })
    }
}
