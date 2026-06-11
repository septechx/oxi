use crate::hashmap::FxHashMap;
use crate::hir::HirId;
use crate::typeck::types::Scheme;

#[derive(Debug)]
pub struct ScopeEnv {
    frames: Vec<FxHashMap<HirId, Scheme>>,
}

impl ScopeEnv {
    pub fn new() -> Self {
        Self {
            frames: vec![FxHashMap::default()],
        }
    }

    pub fn push(&mut self) {
        self.frames.push(FxHashMap::default());
    }

    pub fn pop(&mut self) {
        assert!(self.frames.len() > 1, "Cannot pop the root frame");
        self.frames.pop();
    }

    pub fn insert(&mut self, id: HirId, scheme: Scheme) {
        self.frames
            .last_mut()
            .expect("has frames")
            .insert(id, scheme);
    }

    pub fn get(&self, id: &HirId) -> Option<&Scheme> {
        for frame in self.frames.iter().rev() {
            if let Some(s) = frame.get(id) {
                return Some(s);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{DefId, HirId, ItemLocalId, OwnerId};
    use crate::typeck::types::Ty;

    fn scheme(body: Ty) -> Scheme {
        Scheme {
            vars: thin_vec::thin_vec![],
            body,
        }
    }

    fn hir(owner: u32, local: u32) -> HirId {
        HirId {
            owner: OwnerId(owner),
            local_id: ItemLocalId(local),
        }
    }

    #[test]
    fn inner_scope_shadows_outer() {
        let mut env = ScopeEnv::new();
        let id = hir(0, 1);
        env.insert(
            id,
            scheme(Ty::Prim(crate::hir::PrimTy::Int(crate::hir::IntTy::I32))),
        );
        env.push();
        env.insert(id, scheme(Ty::Prim(crate::hir::PrimTy::Bool)));
        let resolved = env.get(&id).expect("id exists");
        match &resolved.body {
            Ty::Prim(crate::hir::PrimTy::Bool) => {}
            _ => panic!("expected inner shadowing"),
        }
        env.pop();
        let resolved = env.get(&id).expect("id exists");
        match &resolved.body {
            Ty::Prim(crate::hir::PrimTy::Int(crate::hir::IntTy::I32)) => {}
            _ => panic!("expected outer"),
        }
    }

    #[test]
    fn pop_returns_to_outer_value() {
        let mut env = ScopeEnv::new();
        let id = hir(0, 1);
        env.push();
        env.insert(id, scheme(Ty::Adt(DefId(7))));
        env.pop();
        assert!(env.get(&id).is_none());
    }
}
