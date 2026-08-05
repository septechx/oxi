use fxhash::{FxHashMap, FxHashSet};
use thin_vec::ThinVec;

use crate::ast::Mutability;
use crate::hir::{self, DefId, DefKind, ModuleId, Path, PrimTy, QPath, TyKind};
use crate::interner::Symbol;
use crate::resolve::Res;
use crate::span::Span;
use crate::typeck::Typeck;
use crate::typeck::fold::{fold_ty, resolve_scheme_with_args, substitute_ty_vars};
use crate::typeck::infctx::{InferCtx, TyVarId, TyVarSource};

#[derive(Debug, Clone)]
pub enum TyFromHirError {
    /// The qself path in a projection did not resolve to a trait
    ExpectedPathToTrait {
        span: Span,
        module_id: ModuleId,
        path: Path,
    },
    /// An associated type could not be resolved
    UnresolvedAssocType { span: Span, module_id: ModuleId },
    /// The wrong number of generic arguments were provided to a generic type
    UnexpectedGenericArgs {
        span: Span,
        module_id: ModuleId,
        expected: usize,
        found: usize,
    },
}

pub type TyFromHirResult<T> = Result<T, TyFromHirError>;

enum TraitAssocTypeLookup {
    Found(DefId, DefId),
    Ambiguous,
    NotFound,
}

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
    pub fn ty_from_hir(
        &mut self,
        icx: &mut InferCtx,
        hir_ty: &hir::Ty,
        module_id: ModuleId,
    ) -> TyFromHirResult<Ty> {
        match &hir_ty.kind {
            TyKind::Error => Ok(Ty::Error),
            TyKind::Never => Ok(Ty::Never),
            TyKind::Infer => Ok(icx.alloc_ty_var()),
            TyKind::PrimTy(prim) => Ok(Ty::Prim(*prim)),
            TyKind::Ptr(inner, m) => Ok(Ty::Ptr(
                self.ty_from_hir(icx, inner, module_id)?.into_box(),
                *m,
            )),
            TyKind::Slice(inner) => Ok(Ty::Slice(
                self.ty_from_hir(icx, inner, module_id)?.into_box(),
            )),
            TyKind::Array(inner, size) => Ok(Ty::Array(
                self.ty_from_hir(icx, inner, module_id)?.into_box(),
                *size,
            )),
            TyKind::Fn { params, ret } => Ok(Ty::Fn {
                params: params
                    .iter()
                    .map(|hir_ty| self.ty_from_hir(icx, hir_ty, module_id))
                    .collect::<TyFromHirResult<ThinVec<_>>>()?,
                ret: self.ty_from_hir(icx, ret, module_id)?.into_box(),
            }),
            TyKind::Tuple(elements) => Ok(Ty::Tuple(
                elements
                    .iter()
                    .map(|hir_ty| self.ty_from_hir(icx, hir_ty, module_id))
                    .collect::<TyFromHirResult<ThinVec<_>>>()?,
            )),
            TyKind::Path(qpath) => match qpath {
                QPath::Resolved(_, path) => match path.res {
                    Res::Def(def_id) => {
                        let generic_args = self.ty_hir_generic_args(icx, path, module_id)?;
                        Ok(Ty::Adt(def_id, generic_args))
                    }
                    Res::SelfTyAlias { alias_to } => {
                        self.resolve_self_ty(alias_to, icx, path, module_id)
                    }
                    Res::PrimTy(prim) => Ok(Ty::Prim(prim)),
                    Res::GenericParam(hir_id) => {
                        let ty_var = icx.hir_id_to_ty_var.get(&hir_id).expect("hir id exists");
                        Ok(Ty::Var(*ty_var))
                    }
                    Res::Local(_) | Res::Err => Ok(Ty::Error),
                },
                QPath::TypeRelative { qself, segment } => {
                    self.resolve_type_relative_projection(icx, qself, segment, module_id)
                }
            },
            TyKind::GenericParam(hir_id, _) => {
                let ty_var = icx.hir_id_to_ty_var.get(hir_id).expect("hir id exists");
                Ok(Ty::Var(*ty_var))
            }
        }
    }

    fn resolve_self_ty(
        &mut self,
        alias_to: DefId,
        icx: &mut InferCtx,
        path: &hir::Path,
        module_id: ModuleId,
    ) -> TyFromHirResult<Ty> {
        if let Some(Ty::Adt(id, args)) = &self.current_self_ty
            && *id == alias_to
        {
            Ok(Ty::Adt(alias_to, args.clone()))
        } else {
            let generic_args = self.ty_hir_generic_args(icx, path, module_id)?;
            Ok(Ty::Adt(alias_to, generic_args))
        }
    }

    /// Resolve `QPath::TypeRelative` to a `Ty::Projection`
    fn resolve_type_relative_projection(
        &mut self,
        icx: &mut InferCtx,
        qself: &QPath,
        segment: &hir::PathSegment,
        module_id: ModuleId,
    ) -> TyFromHirResult<Ty> {
        let assoc_name = segment.ident.value;

        let (trait_def_id, assoc_def_id, self_ty, trait_path_args) = match qself {
            // <Struct as Trait>::AssocType: explicit trait ref
            QPath::Resolved(Some(self_ty), path) => {
                let Res::Def(trait_def_id) = path.res else {
                    return Err(TyFromHirError::ExpectedPathToTrait {
                        span: path.span,
                        module_id,
                        path: path.clone(),
                    });
                };
                if self.resolver.def(trait_def_id).kind != DefKind::Trait {
                    return Err(TyFromHirError::ExpectedPathToTrait {
                        span: path.span,
                        module_id,
                        path: path.clone(),
                    });
                }
                let assoc_def_id = match self.find_assoc_type(trait_def_id, assoc_name) {
                    Some(id) => id,
                    None => {
                        return Err(TyFromHirError::UnresolvedAssocType {
                            span: segment.ident.span,
                            module_id,
                        });
                    }
                };
                let trait_path_args = self.ty_hir_generic_args(icx, path, module_id)?;
                (
                    trait_def_id,
                    assoc_def_id,
                    self.ty_from_hir(icx, self_ty, module_id)?,
                    trait_path_args,
                )
            }
            // `Struct::AssocType`: struct in impl body context
            QPath::Resolved(None, path) => match path.res {
                Res::Def(def_id) | Res::SelfTyAlias { alias_to: def_id } => {
                    let is_self_alias = matches!(path.res, Res::SelfTyAlias { .. });
                    let (self_ty, generic_args) =
                        self.shorthand_self_ty(def_id, is_self_alias, icx, path, module_id)?;
                    match self.resolver.def(def_id).kind {
                        DefKind::Trait => match self.find_assoc_type(def_id, assoc_name) {
                            Some(assoc_def_id) => (def_id, assoc_def_id, self_ty, None),
                            None => {
                                return Err(TyFromHirError::UnresolvedAssocType {
                                    span: segment.ident.span,
                                    module_id,
                                });
                            }
                        },
                        DefKind::Struct => {
                            if let Some(assoc_def_id) = self.find_assoc_type(def_id, assoc_name) {
                                let Some(scheme) = self.item_schemes.get(&assoc_def_id) else {
                                    return Ok(Ty::Error);
                                };
                                if let Some(args) = &generic_args {
                                    if let Some(resolved) =
                                        resolve_scheme_with_args(scheme, &generic_args)
                                    {
                                        return Ok(resolved);
                                    }
                                    return Err(TyFromHirError::UnexpectedGenericArgs {
                                        span: path.span,
                                        module_id,
                                        expected: scheme.vars.len(),
                                        found: args.len(),
                                    });
                                }
                                let scheme_body = scheme.body.clone();
                                let scheme_vars = scheme.vars.clone();
                                if scheme_vars.is_empty() {
                                    return Ok(scheme_body);
                                }
                                if let Some(info) =
                                    self.coherence.generic_params.get(&def_id).cloned()
                                    && info.defaults.iter().all(|d| d.is_some())
                                {
                                    let args: ThinVec<Ty> = info
                                        .defaults
                                        .iter()
                                        .map(|d| {
                                            self.ty_from_hir(
                                                icx,
                                                d.as_ref().expect("default exists"),
                                                module_id,
                                            )
                                        })
                                        .collect::<TyFromHirResult<ThinVec<_>>>()?;
                                    if args.len() == scheme_vars.len() {
                                        let args: ThinVec<Ty> = args
                                            .into_iter()
                                            .map(|arg| self.normalize_assoc_projections(&arg))
                                            .collect();
                                        let mapping: FxHashMap<TyVarId, Ty> =
                                            scheme_vars.into_iter().zip(args).collect();
                                        return Ok(substitute_ty_vars(&scheme_body, &mapping));
                                    }
                                }

                                return Err(TyFromHirError::UnexpectedGenericArgs {
                                    span: path.span,
                                    module_id,
                                    expected: scheme_vars.len(),
                                    found: 0,
                                });
                            }
                            match self.find_trait_assoc_type_for_struct(def_id, assoc_name) {
                                TraitAssocTypeLookup::Found(trait_id, assoc_id) => {
                                    (trait_id, assoc_id, self_ty, None)
                                }
                                TraitAssocTypeLookup::Ambiguous
                                | TraitAssocTypeLookup::NotFound => {
                                    return Err(TyFromHirError::UnresolvedAssocType {
                                        span: segment.ident.span,
                                        module_id,
                                    });
                                }
                            }
                        }
                        _ => return Ok(Ty::Error),
                    }
                }
                Res::GenericParam(hir_id) => {
                    if !icx.hir_id_to_ty_var.contains_key(&hir_id) {
                        return Ok(Ty::Error);
                    }
                    return Err(TyFromHirError::UnresolvedAssocType {
                        span: segment.ident.span,
                        module_id,
                    });
                }
                _ => return Ok(Ty::Error),
            },
            QPath::TypeRelative { .. } => return Ok(Ty::Error),
        };

        let segment_args = segment
            .generic_args
            .as_ref()
            .map(|args| {
                args.iter()
                    .map(|ty| self.ty_from_hir(icx, ty, module_id))
                    .collect::<TyFromHirResult<ThinVec<_>>>()
            })
            .transpose()?;

        Ok(Ty::Projection {
            trait_def_id,
            assoc_def_id,
            self_ty: Box::new(self_ty),
            generic_args: segment_args.or(trait_path_args),
        })
    }

    fn shorthand_self_ty(
        &mut self,
        def_id: DefId,
        is_self_alias: bool,
        icx: &mut InferCtx,
        path: &hir::Path,
        module_id: ModuleId,
    ) -> TyFromHirResult<(Ty, Option<ThinVec<Ty>>)> {
        if is_self_alias
            && let Some(Ty::Adt(id, args)) = &self.current_self_ty
            && *id == def_id
        {
            Ok((Ty::Adt(def_id, args.clone()), args.clone()))
        } else {
            let generic_args = self.ty_hir_generic_args(icx, path, module_id)?;
            Ok((Ty::Adt(def_id, generic_args.clone()), generic_args))
        }
    }

    fn find_trait_assoc_type_for_struct(
        &self,
        struct_def_id: DefId,
        assoc_name: Symbol,
    ) -> TraitAssocTypeLookup {
        let mut found: Option<(DefId, DefId)> = None;
        let mut seen_traits: FxHashSet<DefId> = FxHashSet::default();
        for &trait_id in self
            .coherence
            .struct_to_traits
            .get(&struct_def_id)
            .into_iter()
            .flatten()
        {
            if !seen_traits.insert(trait_id) {
                continue;
            }
            if let Some(&assoc_def_id) =
                self.coherence.assoc_type_index.get(&(trait_id, assoc_name))
            {
                if found.is_some() {
                    return TraitAssocTypeLookup::Ambiguous;
                }
                found = Some((trait_id, assoc_def_id));
            }
        }
        match found {
            Some(found) => TraitAssocTypeLookup::Found(found.0, found.1),
            None => TraitAssocTypeLookup::NotFound,
        }
    }

    fn find_assoc_type(&self, parent: DefId, name: Symbol) -> Option<DefId> {
        self.coherence
            .assoc_type_index
            .get(&(parent, name))
            .copied()
    }

    pub(super) fn ty_hir_generic_args(
        &mut self,
        icx: &mut InferCtx,
        path: &hir::Path,
        module_id: ModuleId,
    ) -> TyFromHirResult<Option<ThinVec<Ty>>> {
        // TODO: Handle generic args in spots other than the last segment.
        // Currently Adt's can only have generic args in the last segment, but
        // when support for associated types is added, this will need to be
        // implemented.
        path.segments
            .last()
            .expect("path has segments")
            .generic_args
            .as_ref()
            .map(|args| {
                args.iter()
                    .map(|ty| self.ty_from_hir(icx, ty, module_id))
                    .collect::<TyFromHirResult<ThinVec<_>>>()
            })
            .transpose()
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
