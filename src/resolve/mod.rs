use thin_vec::{ThinVec, thin_vec};

use crate::ast::{Ast, ImportTree, Visibility};
use crate::hir::ModuleId;
use crate::hir::interner::{Interner, Symbol};

mod early;

#[derive(Debug, Clone, Copy)]
pub enum DefKind {
    Function,
    Struct,
    Interface,
    Static,
}

#[derive(Debug, Clone, Copy)]
pub struct Def {
    name: Symbol,
    kind: DefKind,
}

#[derive(Debug)]
pub struct PendingImport {
    pub module: ModuleId,
    pub import_item: ImportTree,
    pub visibility: Visibility,
}

#[derive(Debug)]
pub struct Resolver<'a> {
    asts: &'a ThinVec<Ast>,
    module_idx: usize,
    interner: &'a mut Interner,
    pending_imports: ThinVec<PendingImport>,
    defs: ThinVec<ThinVec<Def>>,
}

impl<'a> Resolver<'a> {
    pub fn new(asts: &'a ThinVec<Ast>, interner: &'a mut Interner) -> Self {
        Self {
            asts,
            interner,
            module_idx: 0,
            defs: thin_vec![ThinVec::new(); asts.len()],
            pending_imports: ThinVec::new(),
        }
    }

    pub fn dump(&self) {
        dbg!(&self.defs);
        dbg!(&self.pending_imports);
        dbg!(&self.interner);
    }
}
