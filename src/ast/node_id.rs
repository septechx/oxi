use std::ops::{Index, IndexMut};

use thin_vec::ThinVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl Default for NodeId {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

#[derive(Debug, Clone)]
pub struct NodeMap<T> {
    vec: ThinVec<T>,
}

impl<T> NodeMap<T> {
    pub fn new() -> Self {
        Self {
            vec: ThinVec::new(),
        }
    }
}

impl<T> Default for NodeMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<NodeId> for NodeMap<T> {
    type Output = T;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.vec[index.0 as usize]
    }
}

impl<T> IndexMut<NodeId> for NodeMap<T> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.vec[index.0 as usize]
    }
}
