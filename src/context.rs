use crate::ast::NodeId;
use crate::errors::ErrorCollector;
use crate::span::sourcemaps::SourceMapManager;

#[derive(Debug)]
pub struct Ctx {
    pub errors: ErrorCollector,
    pub source_maps: SourceMapManager,
    pub enable_printing: bool,
    next_node_id: u32,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            errors: ErrorCollector::new(),
            source_maps: SourceMapManager::default(),
            next_node_id: 0,
            enable_printing: true,
        }
    }

    pub fn next_node_id(&mut self) -> NodeId {
        let node_id = self.next_node_id;
        self.next_node_id += 1;
        NodeId(node_id)
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
