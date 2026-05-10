use crate::errors::ErrorCollector;
use crate::hir::interner::Interner;
use crate::span::sourcemaps::SourceMapManager;

#[derive(Debug)]
pub struct Ctx {
    pub errors: ErrorCollector,
    pub source_maps: SourceMapManager,
    pub interner: Interner,
    pub enable_printing: bool,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            errors: ErrorCollector::new(),
            source_maps: SourceMapManager::default(),
            interner: Interner::new(),
            enable_printing: true,
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

pub fn with_ctx<T>(callback: impl FnOnce(&Ctx) -> T) -> T {
    crate::CTX.with_borrow(callback)
}

pub fn with_ctx_mut<T>(callback: impl FnOnce(&mut Ctx) -> T) -> T {
    crate::CTX.with_borrow_mut(callback)
}
