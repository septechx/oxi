use crate::errors::ErrorCollector;
use crate::span::sourcemaps::SourceMapManager;

#[derive(Debug)]
pub struct Ctx {
    pub errors: ErrorCollector,
    pub source_maps: SourceMapManager,
    pub enable_printing: bool,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            errors: ErrorCollector::new(),
            source_maps: SourceMapManager::default(),
            enable_printing: true,
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
