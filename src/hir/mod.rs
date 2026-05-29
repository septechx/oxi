#[macro_use]
mod macros;
mod index;
mod lower;

use std::ffi::OsString;
use std::path;

use thin_vec::ThinVec;

use crate::ast::Ast;
use crate::context::Ctx;
use crate::hir::index::index_crate;
use crate::interner::{Symbol, sym};
use crate::resolve::{ModuleTree, ResolverOutputs};

impl_ids!(ModuleId, DefId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntTy {
    Isize,
    I8,
    I16,
    I32,
    I64,
    I128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UintTy {
    Usize,
    U8,
    U16,
    U32,
    U64,
    U128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatTy {
    F16,
    F32,
    F64,
    F128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimTy {
    Int(IntTy),
    Uint(UintTy),
    Float(FloatTy),
    Bool,
    Void,
}

impl PrimTy {
    pub fn from_name(s: Symbol) -> Option<Self> {
        Some(match s {
            sym::i8 => Self::Int(IntTy::I8),
            sym::i16 => Self::Int(IntTy::I16),
            sym::i32 => Self::Int(IntTy::I32),
            sym::i64 => Self::Int(IntTy::I64),
            sym::i128 => Self::Int(IntTy::I128),
            sym::isize => Self::Int(IntTy::Isize),
            sym::u8 => Self::Uint(UintTy::U8),
            sym::u16 => Self::Uint(UintTy::U16),
            sym::u32 => Self::Uint(UintTy::U32),
            sym::u64 => Self::Uint(UintTy::U64),
            sym::u128 => Self::Uint(UintTy::U128),
            sym::usize => Self::Uint(UintTy::Usize),
            sym::f16 => Self::Float(FloatTy::F16),
            sym::f32 => Self::Float(FloatTy::F32),
            sym::f64 => Self::Float(FloatTy::F64),
            sym::f128 => Self::Float(FloatTy::F128),
            sym::bool => Self::Bool,
            sym::void => Self::Void,
            _ => return None,
        })
    }

    pub fn name(self) -> Symbol {
        match self {
            PrimTy::Int(IntTy::I8) => sym::i8,
            PrimTy::Int(IntTy::I16) => sym::i16,
            PrimTy::Int(IntTy::I32) => sym::i32,
            PrimTy::Int(IntTy::I64) => sym::i64,
            PrimTy::Int(IntTy::I128) => sym::i128,
            PrimTy::Int(IntTy::Isize) => sym::isize,
            PrimTy::Uint(UintTy::U8) => sym::u8,
            PrimTy::Uint(UintTy::U16) => sym::u16,
            PrimTy::Uint(UintTy::U32) => sym::u32,
            PrimTy::Uint(UintTy::U64) => sym::u64,
            PrimTy::Uint(UintTy::U128) => sym::u128,
            PrimTy::Uint(UintTy::Usize) => sym::usize,
            PrimTy::Float(FloatTy::F16) => sym::f16,
            PrimTy::Float(FloatTy::F32) => sym::f32,
            PrimTy::Float(FloatTy::F64) => sym::f64,
            PrimTy::Float(FloatTy::F128) => sym::f128,
            PrimTy::Bool => sym::bool,
            PrimTy::Void => sym::void,
        }
    }
}

#[derive(Debug)]
pub struct AstLoweringContext<'a, 'ctx> {
    ctx: &'ctx mut Ctx,
    asts: &'a ThinVec<Ast>,
    module_tree: &'a ModuleTree,
    resolver: &'a ResolverOutputs,
}

impl<'a, 'ctx> AstLoweringContext<'a, 'ctx> {
    pub fn new(
        ctx: &'ctx mut Ctx,
        asts: &'a ThinVec<Ast>,
        module_tree: &'a ModuleTree,
        resolver: &'a ResolverOutputs,
    ) -> Self {
        Self {
            ctx,
            asts,
            module_tree,
            resolver,
        }
    }

    pub fn lower_crate(&mut self) {
        let index = index_crate(
            self.asts,
            self.module_tree,
            &self.resolver.def_map,
            self.resolver.defs.len(),
        );

        for def_id in index.indices() {
            self.lower_node(def_id, &index);
        }
    }
}

/// Convert a path like `a/b.oxi` into `a::b`.
pub fn path_to_mod<P: AsRef<path::Path>>(p: P) -> String {
    let path = p.as_ref();

    let mut normals: Vec<OsString> = path
        .components()
        .filter_map(|c| match c {
            path::Component::Normal(os) => Some(os.to_os_string()),
            _ => None,
        })
        .collect();

    if normals.is_empty() {
        return String::new();
    }

    let last = normals.pop().expect("normals isn't empty");
    let last_stem = path::Path::new(&last)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut parts: Vec<String> = normals
        .into_iter()
        .map(|os| os.to_string_lossy().into_owned())
        .collect();

    if !last_stem.is_empty() {
        parts.push(last_stem);
    }

    parts.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        assert_eq!(path_to_mod("a/b.oxi"), "a::b");
        assert_eq!(path_to_mod("a/b.test.oxi"), "a::b.test");
        assert_eq!(path_to_mod("single.oxi"), "single");
        assert_eq!(path_to_mod("single"), "single");
        assert_eq!(path_to_mod("/usr/local/pkg.oxi"), "usr::local::pkg");
    }
}
