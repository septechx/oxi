use thin_vec::ThinVec;

use crate::hir::{DefId, HirId};
use crate::resolve::Res;
use crate::typeck::Ty;
use crate::typeck::infctx::TyVarId;
use crate::typeck::types::Scheme;
use fxhash::{FxHashMap, FxHashSet};

pub fn res_to_def_id(res: Res<HirId>) -> Option<DefId> {
    match res {
        Res::Def(def_id) | Res::SelfTyAlias { alias_to: def_id } => Some(def_id),
        _ => None,
    }
}

// TODO: Refactor into trait.
pub fn fold_ty<F>(ty: &Ty, f: &mut F) -> Ty
where
    F: FnMut(Ty) -> Ty,
{
    let ty = match ty {
        Ty::Var(_) | Ty::Prim(_) | Ty::Never | Ty::MethodCallee | Ty::Error => ty.clone(),
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
        Ty::Projection {
            trait_def_id,
            assoc_def_id,
            self_ty,
            generic_args,
        } => Ty::Projection {
            self_ty: fold_ty(self_ty, f).into_box(),
            generic_args: generic_args
                .as_ref()
                .map(|args| args.iter().map(|ty| fold_ty(ty, f)).collect()),
            trait_def_id: *trait_def_id,
            assoc_def_id: *assoc_def_id,
        },
    };

    f(ty)
}

pub fn substitute_ty_vars(ty: &Ty, mapping: &FxHashMap<TyVarId, Ty>) -> Ty {
    fold_ty(ty, &mut |ty| match ty {
        Ty::Var(v) => mapping.get(&v).cloned().unwrap_or(Ty::Var(v)),
        ty => ty,
    })
}

pub fn resolve_scheme_with_args(scheme: &Scheme, generic_args: &Option<ThinVec<Ty>>) -> Option<Ty> {
    let resolved = match generic_args {
        Some(args) if args.len() == scheme.vars.len() => {
            let mapping: FxHashMap<TyVarId, Ty> = scheme
                .vars
                .iter()
                .copied()
                .zip(args.iter().cloned())
                .collect();
            substitute_ty_vars(&scheme.body, &mapping)
        }
        _ if scheme.vars.is_empty() => scheme.body.clone(),
        _ => return None,
    };
    Some(resolved)
}

pub enum AliasExpand {
    Expanded(Ty),
    Cyclic,
    NoScheme,
    ArityMismatch { expected: usize },
}

pub fn expand_type_alias(
    def_id: DefId,
    generic_args: &Option<ThinVec<Ty>>,
    item_schemes: &FxHashMap<DefId, Scheme>,
    in_progress: &mut FxHashSet<DefId>,
) -> AliasExpand {
    if !in_progress.insert(def_id) {
        return AliasExpand::Cyclic;
    }
    match item_schemes.get(&def_id) {
        Some(scheme) => match resolve_scheme_with_args(scheme, generic_args) {
            Some(resolved) => AliasExpand::Expanded(resolved),
            None => AliasExpand::ArityMismatch {
                expected: scheme.vars.len(),
            },
        },
        None => AliasExpand::NoScheme,
    }
}
