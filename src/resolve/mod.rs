use std::ops::{Index, IndexMut};

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
    visibility: Visibility,
}

#[derive(Debug)]
pub struct PendingImport {
    pub module: ModuleId,
    pub import_item: ImportTree,
    pub visibility: Visibility,
}

#[derive(Debug)]
pub struct PerModule<T: Clone + Default>(ThinVec<T>);

impl<T: Clone + Default> PerModule<T> {
    pub fn new(len: usize) -> Self {
        Self(thin_vec![Default::default(); len])
    }
}

impl<T: Clone + Default> Index<usize> for PerModule<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T: Clone + Default> IndexMut<usize> for PerModule<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[derive(Debug)]
pub struct Resolver<'a> {
    asts: &'a ThinVec<Ast>,
    module_idx: usize,
    interner: &'a mut Interner,
    pending_imports: ThinVec<PendingImport>,
    defs: PerModule<ThinVec<Def>>,
    imports: PerModule<ThinVec<Def>>,
}

impl<'a> Resolver<'a> {
    pub fn new(asts: &'a ThinVec<Ast>, interner: &'a mut Interner) -> Self {
        Self {
            asts,
            interner,
            module_idx: 0,
            defs: PerModule::new(asts.len()),
            imports: PerModule::new(asts.len()),
            pending_imports: ThinVec::new(),
        }
    }

    pub fn dump(&self) {
        dbg!(&self.defs);
        dbg!(&self.pending_imports);
        dbg!(&self.interner);
    }
}
