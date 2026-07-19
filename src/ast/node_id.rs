use fxhash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

impl Default for NodeId {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

pub type NodeMap<T> = FxHashMap<NodeId, T>;
