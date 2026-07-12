use crate::typeck::Ty;

pub fn fold_ty<F>(ty: &Ty, f: &mut F) -> Ty
where
    F: FnMut(Ty) -> Ty,
{
    let ty = match ty {
        Ty::Var(_) | Ty::Prim(_) | Ty::Never | Ty::Error => ty.clone(),
        Ty::Adt(d, generics) => Ty::Adt(
            *d,
            generics
                .as_ref()
                .map(|args| args.iter().map(|ty| fold_ty(ty, f)).collect()),
        ),
        Ty::Ptr(inner, m) => Ty::Ptr(fold_ty(inner, f).into_box(), *m),
        Ty::Slice(inner) => Ty::Slice(fold_ty(inner, f).into_box()),
        Ty::Array(inner, n) => Ty::Array(fold_ty(inner, f).into_box(), *n),
        Ty::Fn { params, ret } => Ty::Fn {
            params: params.iter().map(|ty| fold_ty(ty, f)).collect(),
            ret: fold_ty(ret, f).into_box(),
        },
        Ty::Tuple(elements) => Ty::Tuple(elements.iter().map(|ty| fold_ty(ty, f)).collect()),
    };

    f(ty)
}
