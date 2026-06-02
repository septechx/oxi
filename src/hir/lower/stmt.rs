use crate::ast;
use crate::hir::AstLoweringContext;
use crate::hir::types::{Stmt, StmtKind, Ty, TyKind};

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub(super) fn lower_stmt(&mut self, stmt: &ast::Stmt) -> Stmt {
        let hir_id = self.next_hir_id();
        let kind = match &stmt.kind {
            ast::StmtKind::Let {
                name,
                ty,
                value,
                mutability,
            } => {
                let init = value.as_ref().map(|expr| self.lower_expr(expr));
                let ty = self.lower_type(ty);

                // Register the local's HirId so path expressions can resolve to it
                self.register_local(stmt.node_id, hir_id);

                StmtKind::Let {
                    name: name.value,
                    ty,
                    init,
                    local: hir_id,
                    mutability: *mutability,
                }
            }
            ast::StmtKind::Expr(expr) => StmtKind::Expr(self.lower_expr(expr)),
            ast::StmtKind::Semi(expr) => StmtKind::Semi(self.lower_expr(expr)),
        };

        Stmt {
            hir_id,
            kind,
            span: stmt.span,
        }
    }

    pub(super) fn lower_type(&mut self, ty: &ast::Type) -> Ty {
        let hir_id = self.next_hir_id();
        let kind = match &ty.kind {
            ast::TypeKind::Symbol(path) => {
                let qpath = self.lower_qpath(path, ty.node_id);
                TyKind::Path(qpath)
            }
            ast::TypeKind::Pointer(inner, mutability) => {
                TyKind::Ptr(Box::new(self.lower_type(inner)), *mutability)
            }
            ast::TypeKind::Slice(inner) => TyKind::Slice(Box::new(self.lower_type(inner))),
            ast::TypeKind::FixedArray(inner, size) => {
                TyKind::Array(Box::new(self.lower_type(inner)), *size)
            }
            ast::TypeKind::Function { params, ret } => {
                let params = params.iter().map(|p| self.lower_type(p)).collect();
                TyKind::Fn {
                    params,
                    ret: Box::new(self.lower_type(ret)),
                }
            }
            ast::TypeKind::Tuple(elements) => {
                TyKind::Tuple(elements.iter().map(|e| self.lower_type(e)).collect())
            }
            ast::TypeKind::Infer => TyKind::Infer,
            ast::TypeKind::Never => TyKind::Never,
        };

        Ty {
            hir_id,
            kind,
            span: ty.span,
        }
    }
}
