use crate::ast::Visibility;
use crate::interner::Symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefKind {
    Mod,
    Struct,
    Interface,
    Function,
    Const,
    Impl,
    AssocFn,
    Import,
    Field,
    Local,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub name: Symbol,
    pub kind: DefKind,
    pub visibility: Visibility,
}
