use crate::ast;
use crate::hir::AstLoweringContext;
use crate::hir::types::{Expr, ExprKind};

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
            _ => todo!("Lowering of {:?} not yet implemented", expr.kind),
        }
    }
}
