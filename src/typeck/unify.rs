use crate::hir::{FloatTy, IntTy, ModuleId, PrimTy};
use crate::span::Span;
use crate::typeck::infctx::{InferCtx, TyVarId, TyVarSource};
use crate::typeck::types::Ty;

#[derive(Debug, Clone)]
pub enum UnifyError {
    /// Cannot unify `expected` and `found`
    Mismatch {
        expected: Ty,
        found: Ty,
        span: Span,
        module_id: ModuleId,
    },
    /// Unification variable contains itself
    OccursCheck {
        var: TyVarId,
        span: Span,
        module_id: ModuleId,
    },
}

pub type UnifyResult<T> = Result<T, UnifyError>;

pub trait OrPushErr {
    fn or_push_err(self, icx: &mut InferCtx);
}

impl<T> OrPushErr for UnifyResult<T> {
    fn or_push_err(self, icx: &mut InferCtx) {
        if let Err(err) = self {
            icx.errors.push(err);
        }
    }
}

pub fn unify(
    icx: &mut InferCtx,
    a: &Ty,
    b: &Ty,
    span: Span,
    module_id: ModuleId,
) -> UnifyResult<()> {
    let a = icx.resolve(a);
    let b = icx.resolve(b);
    match (&a, &b) {
        (Ty::Error, _) | (_, Ty::Error) => Ok(()),
        (Ty::Never, _) | (_, Ty::Never) => Ok(()),
        (Ty::Var(v), t) | (t, Ty::Var(v)) => bind(icx, *v, t, span, module_id),
        (Ty::Prim(p1), Ty::Prim(p2)) => {
            if p1 == p2 {
                Ok(())
            } else {
                Err(mismatch(a, b, span, module_id))
            }
        }
        (Ty::Ptr(i1, m1), Ty::Ptr(i2, m2)) => {
            if m1 == m2 {
                unify(icx, i1, i2, span, module_id)
            } else {
                Err(mismatch(a, b, span, module_id))
            }
        }
        (Ty::Slice(i1), Ty::Slice(i2)) => unify(icx, i1, i2, span, module_id),
        (Ty::Adt(d1, g1), Ty::Adt(d2, g2)) => {
            if d1 == d2 {
                if let (Some(g1), Some(g2)) = (g1, g2) {
                    if g1.len() != g2.len() {
                        return Err(mismatch(a, b, span, module_id));
                    }
                    for (a, b) in g1.iter().zip(g2) {
                        unify(icx, a, b, span, module_id)?;
                    }
                }
                Ok(())
            } else {
                Err(mismatch(a, b, span, module_id))
            }
        }
        (Ty::Array(i1, n1), Ty::Array(i2, n2)) => {
            if n1 == n2 {
                unify(icx, i1, i2, span, module_id)
            } else {
                Err(mismatch(a, b, span, module_id))
            }
        }
        (
            Ty::Fn {
                params: p1,
                ret: r1,
            },
            Ty::Fn {
                params: p2,
                ret: r2,
            },
        ) => {
            if p1.len() != p2.len() {
                return Err(mismatch(a, b, span, module_id));
            }
            for (a, b) in p1.iter().zip(p2) {
                unify(icx, a, b, span, module_id)?;
            }
            unify(icx, r1, r2, span, module_id)
        }
        (Ty::Tuple(e1), Ty::Tuple(e2)) => {
            if e1.len() != e2.len() {
                return Err(mismatch(a, b, span, module_id));
            }
            for (a, b) in e1.iter().zip(e2) {
                unify(icx, a, b, span, module_id)?;
            }
            Ok(())
        }
        _ => Err(mismatch(a, b, span, module_id)),
    }
}

fn bind(
    icx: &mut InferCtx,
    var: TyVarId,
    to: &Ty,
    span: Span,
    module_id: ModuleId,
) -> UnifyResult<()> {
    let to = icx.resolve(to);
    if let Ty::Var(other) = &to
        && *other == var
    {
        return Ok(());
    }
    match icx.ty_var_source(var) {
        TyVarSource::IntLit => match &to {
            Ty::Var(to_var) if matches!(icx.ty_var_source(*to_var), TyVarSource::IntLit) => {}
            Ty::Prim(PrimTy::Int(_) | PrimTy::Uint(_)) => {}
            _ => {
                return Err(mismatch(
                    to.clone(),
                    Ty::Prim(PrimTy::Int(IntTy::I32)),
                    span,
                    module_id,
                ));
            }
        },
        TyVarSource::FloatLit => match &to {
            Ty::Var(to_var) if matches!(icx.ty_var_source(*to_var), TyVarSource::FloatLit) => {}
            Ty::Prim(PrimTy::Float(_)) => {}
            _ => {
                return Err(mismatch(
                    to.clone(),
                    Ty::Prim(PrimTy::Float(FloatTy::F64)),
                    span,
                    module_id,
                ));
            }
        },
        TyVarSource::Generic | TyVarSource::EmptyArray => {}
    }
    if occurs(icx, var, &to) {
        return Err(UnifyError::OccursCheck {
            var,
            span,
            module_id,
        });
    }
    let var_level = icx.ty_var(var).level;
    let adjusted = icx.adjust(&to, var_level + 1);
    icx.set_root(var, adjusted);
    Ok(())
}

fn occurs(icx: &InferCtx, var: TyVarId, to: &Ty) -> bool {
    match to {
        Ty::Var(v) => {
            if *v == var {
                return true;
            }
            match icx.root_of(*v) {
                Some(bound) => occurs(icx, var, bound),
                None => false,
            }
        }
        Ty::Ptr(inner, _) | Ty::Slice(inner) | Ty::Array(inner, _) => occurs(icx, var, inner),
        Ty::Fn { params, ret } => {
            params.iter().any(|param| occurs(icx, var, param)) || occurs(icx, var, ret)
        }
        Ty::Tuple(elements) => elements.iter().any(|element| occurs(icx, var, element)),
        Ty::Adt(_, generics) => {
            if let Some(generics) = generics {
                generics.iter().any(|ty| occurs(icx, var, ty))
            } else {
                false
            }
        }
        Ty::Prim(_) | Ty::Never | Ty::Error => false,
    }
}

fn mismatch(expected: Ty, found: Ty, span: Span, module_id: ModuleId) -> UnifyError {
    UnifyError::Mismatch {
        expected,
        found,
        span,
        module_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{DefId, IntTy, PrimTy, UintTy};
    use thin_vec::thin_vec;

    const NO_MODULE: ModuleId = ModuleId(0);

    fn no_span() -> Span {
        Span::new(0, 0)
    }

    fn int() -> Ty {
        Ty::Prim(PrimTy::Int(IntTy::I32))
    }

    #[test]
    fn unify_same_prim() {
        let mut icx = InferCtx::default();
        icx.push_level();
        assert!(unify(&mut icx, &int(), &int(), no_span(), NO_MODULE).is_ok());
    }

    #[test]
    fn unify_different_prims_fails() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let u8 = Ty::Prim(PrimTy::Uint(UintTy::U8));
        assert!(unify(&mut icx, &int(), &u8, no_span(), NO_MODULE).is_err());
    }

    #[test]
    fn unify_var_with_concrete() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let v = icx.next_ty_var();
        assert!(unify(&mut icx, &Ty::Var(v), &int(), no_span(), NO_MODULE).is_ok());
        assert!(matches!(
            icx.resolve(&Ty::Var(v)),
            Ty::Prim(PrimTy::Int(IntTy::I32))
        ));
    }

    #[test]
    fn unify_two_vars_binds_them() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let a = icx.next_ty_var();
        let b = icx.next_ty_var();
        assert!(unify(&mut icx, &Ty::Var(a), &Ty::Var(b), no_span(), NO_MODULE).is_ok());
        let ra = icx.resolve(&Ty::Var(a));
        let rb = icx.resolve(&Ty::Var(b));
        assert!(matches!(ra, Ty::Var(_)));
        assert!(matches!(rb, Ty::Var(_)));
    }

    #[test]
    fn unify_fn_types() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let a = Ty::Fn {
            params: thin_vec![int()],
            ret: Box::new(int()),
        };
        let b = Ty::Fn {
            params: thin_vec![int()],
            ret: Box::new(int()),
        };
        assert!(unify(&mut icx, &a, &b, no_span(), NO_MODULE).is_ok());
    }

    #[test]
    fn unify_fn_arity_mismatch() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let a = Ty::Fn {
            params: thin_vec![int()],
            ret: Box::new(int()),
        };
        let b = Ty::Fn {
            params: thin_vec![],
            ret: Box::new(int()),
        };
        assert!(unify(&mut icx, &a, &b, no_span(), NO_MODULE).is_err());
    }

    #[test]
    fn occurs_check() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let v = icx.next_ty_var();
        let bad = Ty::Fn {
            params: thin_vec![Ty::Var(v)],
            ret: Box::new(int()),
        };
        assert!(unify(&mut icx, &Ty::Var(v), &bad, no_span(), NO_MODULE).is_err());
    }

    #[test]
    fn error_ty_unifies_with_anything() {
        let mut icx = InferCtx::default();
        icx.push_level();
        assert!(unify(&mut icx, &Ty::Error, &int(), no_span(), NO_MODULE).is_ok());
        assert!(unify(&mut icx, &int(), &Ty::Error, no_span(), NO_MODULE).is_ok());
    }

    #[test]
    fn never_unifies_with_everything() {
        let mut icx = InferCtx::default();
        icx.push_level();
        assert!(unify(&mut icx, &Ty::Never, &int(), no_span(), NO_MODULE).is_ok());
        let v = icx.next_ty_var();
        assert!(unify(&mut icx, &Ty::Var(v), &Ty::Never, no_span(), NO_MODULE).is_ok());
    }

    #[test]
    fn unify_same_adt() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let a = Ty::Adt(DefId(7), None);
        let b = Ty::Adt(DefId(7), None);
        assert!(unify(&mut icx, &a, &b, no_span(), NO_MODULE).is_ok());
    }

    #[test]
    fn unify_different_adts_fails() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let a = Ty::Adt(DefId(7), None);
        let b = Ty::Adt(DefId(8), None);
        assert!(unify(&mut icx, &a, &b, no_span(), NO_MODULE).is_err());
    }

    #[test]
    fn unify_adt_with_ptr_inner_succeeds_after_autoref() {
        let mut icx = InferCtx::default();
        icx.push_level();
        let param = Ty::Ptr(
            Box::new(Ty::Adt(DefId(3), None)),
            crate::ast::Mutability::Constant,
        );
        let arg = Ty::Adt(DefId(3), None);
        let inner = match &param {
            Ty::Ptr(i, _) => i.as_ref().clone(),
            _ => unreachable!(),
        };
        assert!(unify(&mut icx, &inner, &arg, no_span(), NO_MODULE).is_ok());
    }
}
