use thin_vec::ThinVec;

use crate::ast::Ast;
use crate::hir::interner::{Interner, Symbol};

mod early;

#[derive(Debug)]
enum DefKind {
    Function,
    Struct,
    Interface,
    Static,
}

#[derive(Debug)]
struct Def {
    name: Symbol,
    kind: DefKind,
}

#[derive(Debug)]
pub struct Resolver<'a> {
    asts: &'a ThinVec<Ast>,
    interner: &'a mut Interner,
    defs: ThinVec<Def>,
}

impl<'a> Resolver<'a> {
    pub fn new(asts: &'a ThinVec<Ast>, interner: &'a mut Interner) -> Self {
        Self {
            asts,
            interner,
            defs: ThinVec::new(),
        }
    }

    pub fn dump(&self) {
        dbg!(&self.defs);
        dbg!(&self.interner);
    }
}
