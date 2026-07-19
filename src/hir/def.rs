use crate::ast::Visibility;
use crate::interner::Symbol;
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefKind {
    Mod,
    Struct,
    Trait,
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
    pub name: Option<Symbol>,
    pub visibility: Option<Visibility>,
    pub kind: DefKind,
    pub span: Span,
}
