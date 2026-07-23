use thin_vec::ThinVec;

use crate::ast::Mutability;
use crate::hir::{self, DefId, DefKind, PrimTy, QPath, TyKind};
use crate::interner::Symbol;
use crate::resolve::Res;
use crate::typeck::Typeck;
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
    Projection {
        trait_def_id: DefId,
        assoc_def_id: DefId,
        self_ty: Box<Ty>,
        generic_args: Option<ThinVec<Ty>>,
    },
    Never,
    /// Dummy type for the synthetic `Path` callee created during THIR lowering
    /// of method calls. This callee `Path` expression is never type-checked, it
    /// unifies with everything. Downstream consumers (IR emission, etc.) resolve
    /// the actual method `DefId` via `TypeckOutputs::member_res`.
    MethodCallee,
    Error,
}

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub fn ty_from_hir(&self, icx: &mut InferCtx, hir_ty: &hir::Ty) -> Ty {
        match &hir_ty.kind {
            TyKind::Error => Ty::Error,
            TyKind::Never => Ty::Never,
            TyKind::Infer => icx.alloc_ty_var(),
            TyKind::PrimTy(prim) => Ty::Prim(*prim),
            TyKind::Ptr(inner, m) => Ty::Ptr(self.ty_from_hir(icx, inner).into_box(), *m),
            TyKind::Slice(inner) => Ty::Slice(self.ty_from_hir(icx, inner).into_box()),
            TyKind::Array(inner, size) => Ty::Array(self.ty_from_hir(icx, inner).into_box(), *size),
            TyKind::Fn { params, ret } => Ty::Fn {
                params: params
                    .iter()
                    .map(|hir_ty| self.ty_from_hir(icx, hir_ty))
                    .collect(),
                ret: self.ty_from_hir(icx, ret).into_box(),
            },
            TyKind::Tuple(elements) => Ty::Tuple(
                elements
                    .iter()
                    .map(|hir_ty| self.ty_from_hir(icx, hir_ty))
                    .collect(),
            ),
            TyKind::Path(qpath) => match qpath {
                QPath::Resolved(_, path) => match path.res {
                    Res::Def(def_id) | Res::SelfTyAlias { alias_to: def_id } => {
                        let generic_args = self.ty_hir_generic_args(icx, path);
                        Ty::Adt(def_id, generic_args)
                    }
                    Res::PrimTy(prim) => Ty::Prim(prim),
                    Res::GenericParam(hir_id) => {
                        let ty_var = icx.hir_id_to_ty_var.get(&hir_id).expect("hir id exists");
                        Ty::Var(*ty_var)
                    }
                    Res::Local(_) | Res::Err => Ty::Error,
                },
                QPath::TypeRelative { qself, segment } => {
                    self.resolve_type_relative_projection(icx, qself, segment)
                }
            },
            TyKind::GenericParam(hir_id, _) => {
                let ty_var = icx.hir_id_to_ty_var.get(hir_id).expect("hir id exists");
                Ty::Var(*ty_var)
            }
        }
    }

    /// Resolve `QPath::TypeRelative` to a `Ty::Projection`
    fn resolve_type_relative_projection(
        &self,
        icx: &mut InferCtx,
        qself: &QPath,
        segment: &hir::PathSegment,
    ) -> Ty {
        let assoc_name = segment.ident.value;

        let (trait_def_id, assoc_def_id, self_ty) = match qself {
            // <Struct as Trait>::AssocType: explicit trait ref
            QPath::Resolved(Some(self_ty), path) => {
                let Res::Def(trait_def_id) = path.res else {
                    return Ty::Error;
                };
                if self.resolver.def(trait_def_id).kind != DefKind::Trait {
                    return Ty::Error;
                }
                let assoc_def_id = match self.find_assoc_type(trait_def_id, assoc_name) {
                    Some(id) => id,
                    None => return Ty::Error,
                };
                (trait_def_id, assoc_def_id, self.ty_from_hir(icx, self_ty))
            }
            // `Struct::AssocType`: struct in impl body context
            QPath::Resolved(None, path) => match path.res {
                Res::Def(def_id) | Res::SelfTyAlias { alias_to: def_id } => {
                    let generic_args = self.ty_hir_generic_args(icx, path);
                    let self_ty = Ty::Adt(def_id, generic_args);
                    match self.resolver.def(def_id).kind {
                        DefKind::Trait => match self.find_assoc_type(def_id, assoc_name) {
                            Some(assoc_def_id) => (def_id, assoc_def_id, self_ty),
                            None => return Ty::Error,
                        },
                        DefKind::Struct => {
                            if let Some(assoc_def_id) = self.find_assoc_type(def_id, assoc_name) {
                                if let Some(scheme) = self.item_schemes.get(&assoc_def_id) {
                                    return scheme.body.clone();
                                }
                                return Ty::Error;
                            }
                            match self.find_trait_assoc_type_for_struct(def_id, assoc_name) {
                                Some((trait_id, assoc_id)) => (trait_id, assoc_id, self_ty),
                                None => return Ty::Error,
                            }
                        }
                        _ => return Ty::Error,
                    }
                }
                Res::GenericParam(hir_id) => {
                    let ty_var = icx.hir_id_to_ty_var.get(&hir_id);
                    match ty_var {
                        Some(&var) => {
                            return Ty::Var(var);
                        }
                        None => return Ty::Error,
                    }
                }
                _ => return Ty::Error,
            },
            QPath::TypeRelative { .. } => return Ty::Error,
        };

        Ty::Projection {
            trait_def_id,
            assoc_def_id,
            self_ty: Box::new(self_ty),
            generic_args: segment
                .generic_args
                .as_ref()
                .map(|args| args.iter().map(|ty| self.ty_from_hir(icx, ty)).collect()),
        }
    }

    fn find_trait_assoc_type_for_struct(
        &self,
        struct_def_id: DefId,
        assoc_name: Symbol,
    ) -> Option<(DefId, DefId)> {
        for &trait_id in self.coherence.struct_to_traits.get(&struct_def_id)? {
            if let Some(&assoc_def_id) =
                self.coherence.assoc_type_index.get(&(trait_id, assoc_name))
            {
                return Some((trait_id, assoc_def_id));
            }
        }
        None
    }

    fn find_assoc_type(&self, parent: DefId, name: Symbol) -> Option<DefId> {
        self.coherence
            .assoc_type_index
            .get(&(parent, name))
            .copied()
    }

    pub(super) fn ty_hir_generic_args(
        &self,
        icx: &mut InferCtx,
        path: &hir::Path,
    ) -> Option<ThinVec<Ty>> {
        // TODO: Handle generic args in spots other than the last segment.
        // Currently Adt's can only have generic args in the last segment, but
        // when support for associated types is added, this will need to be
        // implemented.
        path.segments
            .last()
            .expect("path has segments")
            .generic_args
            .as_ref()
            .map(|args| args.iter().map(|ty| self.ty_from_hir(icx, ty)).collect())
    }
}

impl Ty {
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
