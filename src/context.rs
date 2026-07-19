use std::cell::RefCell;

use crate::errors::ErrorCollector;
use crate::interner::Interner;
use crate::span::sourcemaps::SourceMapManager;

// TODO: Make this not global
thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::new());
}

#[derive(Debug)]
pub struct Ctx {
    pub errors: ErrorCollector,
    pub source_maps: SourceMapManager,
    pub interner: Interner,
    pub enable_printing: bool,
    pub next_node_id: u32,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            errors: ErrorCollector::new(),
            source_maps: SourceMapManager::default(),
            interner: Interner::new(),
            enable_printing: true,
            next_node_id: 0,
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

pub fn with_ctx<T>(callback: impl FnOnce(&Ctx) -> T) -> T {
    CTX.with_borrow(callback)
}

pub fn with_ctx_mut<T>(callback: impl FnOnce(&mut Ctx) -> T) -> T {
    CTX.with_borrow_mut(callback)
}
