use thin_vec::ThinVec;

use crate::ast::Mutability;
use crate::hir::{self, DefId, PrimTy, QPath, TyKind};
use crate::resolve::Res;
use crate::typeck::fold::fold_ty;
use crate::typeck::infctx::{InferCtx, TyVarId, TyVarSource};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Var(TyVarId),
    Prim(PrimTy),
    Ptr(Box<Ty>, Mutability),
    Slice(Box<Ty>),
    Array(Box<Ty>, usize),
    Fn {
        params: ThinVec<Ty>,
        ret: Box<Ty>,
    },
    Tuple(ThinVec<Ty>),
    Adt(DefId, Option<ThinVec<Ty>>),
    Never,
    /// Dummy type for the synthetic `Path` callee created during THIR lowering
    /// of method calls. This callee `Path` expression is never type-checked, it
    /// unifies with everything. Downstream consumers (IR emission, etc.) resolve
    /// the actual method `DefId` via `TypeckOutputs::member_res`.
    MethodCallee,
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
                    Res::Def(def_id) | Res::SelfTyAlias { alias_to: def_id } => {
                        let generic_params = Self::hir_generic_params(icx, path);
                        Ty::Adt(def_id, generic_params)
                    }
                    Res::PrimTy(prim) => Ty::Prim(prim),
                    Res::GenericParam(hir_id) => {
                        let ty_var = icx.hir_id_to_ty_var.get(&hir_id).expect("hir id exists");
                        Ty::Var(*ty_var)
                    }
                    Res::Local(_) | Res::Err => Ty::Error,
                },
                QPath::TypeRelative { .. } => Ty::Error,
            },
            TyKind::GenericParam(hir_id, _) => {
                let ty_var = icx.hir_id_to_ty_var.get(hir_id).expect("hir id exists");
                Ty::Var(*ty_var)
            }
        }
    }

    pub(super) fn hir_generic_params(icx: &mut InferCtx, path: &hir::Path) -> Option<ThinVec<Ty>> {
        // TODO: Handle generic args in spots other than the last segment.
        // Currently Adt's can only have generic args in the last segment, but
        // when support for associated types is added, this will need to be
        // implemented.
        path.segments
            .last()
            .expect("path has segments")
            .generic_params
            .as_ref()
            .map(|params| params.iter().map(|ty| Ty::from_hir(icx, ty)).collect())
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
        fold_ty(&self, &mut |ty| match ty {
            Ty::Var(_) => Ty::Error,
            ty => ty,
        })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
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
