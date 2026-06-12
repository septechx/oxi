use thin_vec::ThinVec;

use crate::ast::Mutability;
use crate::hir::{self, DefId, PrimTy, QPath, TyKind};
use crate::resolve::Res;
use crate::typeck::infctx::{InferCtx, TyVarId, TyVarSource};

#[derive(Debug, Clone)]
pub enum Ty {
    Var(TyVarId),
    Prim(PrimTy),
    Ptr(Box<Ty>, Mutability),
    Slice(Box<Ty>),
    Array(Box<Ty>, usize),
    Fn { params: ThinVec<Ty>, ret: Box<Ty> },
    Tuple(ThinVec<Ty>),
    Adt(DefId),
    Interface(DefId),
    Never,
    Error,
}

impl Ty {
    pub fn from_hir(icx: &mut InferCtx, hir_ty: &hir::Ty) -> Ty {
        match &hir_ty.kind {
            TyKind::Error => Ty::Error,
            TyKind::Never => Ty::Never,
            TyKind::Infer => icx.alloc_ty_var(),
            TyKind::PrimTy(prim) => Ty::Prim(*prim),
            TyKind::Ptr(inner, m) => Ty::Ptr(Ty::from_hir(icx, inner).into_box(), *m),
            TyKind::Slice(inner) => Ty::Slice(Ty::from_hir(icx, inner).into_box()),
            TyKind::Array(inner, size) => Ty::Array(Ty::from_hir(icx, inner).into_box(), *size),
            TyKind::Fn { params, ret } => Ty::Fn {
                params: params
                    .iter()
                    .map(|hir_ty| Ty::from_hir(icx, hir_ty))
                    .collect(),
                ret: Ty::from_hir(icx, ret).into_box(),
            },
            TyKind::Tuple(elements) => Ty::Tuple(
                elements
                    .iter()
                    .map(|hir_ty| Ty::from_hir(icx, hir_ty))
                    .collect(),
            ),
            TyKind::Path(qpath) => match qpath {
                QPath::Resolved(path) => match path.res {
                    Res::Def(def_id) => {
                        // We don't know if it's a struct or interface. Tag it as Adt
                        // and fix it downstream
                        Ty::Adt(def_id)
                    }
                    Res::PrimTy(prim) => Ty::Prim(prim),
                    Res::SelfTyAlias { alias_to } => {
                        // Same as Res::Def
                        Ty::Adt(alias_to)
                    }
                    Res::Local(_) | Res::Err => Ty::Error,
                },
                QPath::TypeRelative { .. } => Ty::Error,
            },
        }
    }

    pub fn is_numeric(&self, icx: &InferCtx) -> bool {
        match self {
            Ty::Prim(PrimTy::Int(_) | PrimTy::Uint(_) | PrimTy::Float(_)) => true,
            Ty::Var(id) => match icx.root_of(*id) {
                Some(resolved) => resolved.is_numeric(icx),
                None => matches!(
                    icx.ty_var_source(*id),
                    TyVarSource::IntLit | TyVarSource::FloatLit
                ),
            },
            _ => false,
        }
    }

    pub fn into_box(self) -> Box<Self> {
        Box::new(self)
    }

    pub fn reject_vars(self) -> Self {
        match self {
            Ty::Var(_) => Ty::Error,
            Ty::Ptr(inner, m) => Ty::Ptr(Box::new(inner.reject_vars()), m),
            Ty::Slice(inner) => Ty::Slice(Box::new(inner.reject_vars())),
            Ty::Array(inner, size) => Ty::Array(Box::new(inner.reject_vars()), size),
            Ty::Fn { params, ret } => Ty::Fn {
                params: params.into_iter().map(|p| p.reject_vars()).collect(),
                ret: Box::new(ret.reject_vars()),
            },
            Ty::Tuple(elements) => {
                Ty::Tuple(elements.into_iter().map(|e| e.reject_vars()).collect())
            }
            other => other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scheme {
    pub vars: ThinVec<TyVarId>,
    pub body: Ty,
}

impl Scheme {
    #[inline]
    pub fn monomorphic(body: Ty) -> Self {
        Self {
            vars: ThinVec::new(),
            body,
        }
    }
}
