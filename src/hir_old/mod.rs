#![allow(unused)]

use thin_vec::ThinVec;

use crate::{
    ast::{Ast, Literal, Mutability, Visibility},
    hashmap::FxHashMap,
    hir2::lower::LoweringContext,
    interner::{Interner, Symbol, sym},
    lexer::token::TokenKind,
    span::Span,
};

pub use resolve::path_to_mod;

mod lower;
mod resolve;

#[allow(unreachable_code)]
pub fn lower_crate(asts: ThinVec<Ast>) -> HirCrate {
    let mut ctx = LoweringContext::new();
    ctx.lower_crate(asts);
    for diag in &ctx.krate.diagnostics {
        crate::error!(diag.clone());
    }
    ctx.krate
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId {
    pub owner: DefId,
    pub local_id: LocalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StmtId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModuleId(pub u32);

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplItemId(pub u32);

impl From<u32> for DefId {
    fn from(v: u32) -> Self {
        DefId(v)
    }
}
impl From<u32> for ExprId {
    fn from(v: u32) -> Self {
        ExprId(v)
    }
}
impl From<u32> for TypeId {
    fn from(v: u32) -> Self {
        TypeId(v)
    }
}
impl From<u32> for LocalId {
    fn from(v: u32) -> Self {
        LocalId(v)
    }
}
impl From<u32> for StmtId {
    fn from(v: u32) -> Self {
        StmtId(v)
    }
}
impl From<u32> for ModuleId {
    fn from(v: u32) -> Self {
        ModuleId(v)
    }
}
impl From<u32> for BodyId {
    fn from(v: u32) -> Self {
        BodyId(v)
    }
}
impl From<u32> for ImplItemId {
    fn from(v: u32) -> Self {
        ImplItemId(v)
    }
}

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

#[derive(Debug, Default)]
pub struct HirCrate {
    pub modules: ThinVec<ModuleInfo>,
    pub items: ThinVec<HirItem>,
    pub impl_items: ThinVec<ImplItem>,
    pub exprs: ThinVec<HirExpr>,
    pub types: ThinVec<HirType>,
    pub stmts: ThinVec<HirStmt>,
    pub bodies: ThinVec<Body>,
    pub interner: Interner,
    pub diagnostics: ThinVec<String>,
}

impl HirCrate {
    pub fn item(&self, id: DefId) -> &HirItem {
        debug_assert!(
            (id.0 as usize) < self.items.len(),
            "DefId({}) out of bounds (len={})",
            id.0,
            self.items.len()
        );
        &self.items[id.0 as usize]
    }

    pub fn expr(&self, id: ExprId) -> &HirExpr {
        debug_assert!(
            (id.0 as usize) < self.exprs.len(),
            "ExprId({}) out of bounds (len={})",
            id.0,
            self.exprs.len()
        );
        &self.exprs[id.0 as usize]
    }

    pub fn stmt(&self, id: StmtId) -> &HirStmt {
        debug_assert!(
            (id.0 as usize) < self.stmts.len(),
            "StmtId({}) out of bounds (len={})",
            id.0,
            self.stmts.len()
        );
        &self.stmts[id.0 as usize]
    }

    pub fn body(&self, id: BodyId) -> &Body {
        debug_assert!(
            (id.0 as usize) < self.bodies.len(),
            "BodyId({}) out of bounds (len={})",
            id.0,
            self.bodies.len()
        );
        &self.bodies[id.0 as usize]
    }

    pub fn ty(&self, id: TypeId) -> &HirType {
        debug_assert!(
            (id.0 as usize) < self.types.len(),
            "TypeId({}) out of bounds (len={})",
            id.0,
            self.types.len()
        );
        &self.types[id.0 as usize]
    }

    pub fn impl_item(&self, id: ImplItemId) -> &ImplItem {
        debug_assert!(
            (id.0 as usize) < self.impl_items.len(),
            "ImplItemId({}) out of bounds (len={})",
            id.0,
            self.impl_items.len()
        );
        &self.impl_items[id.0 as usize]
    }

    pub fn mut_item(&mut self, id: DefId) -> &mut HirItem {
        debug_assert!(
            (id.0 as usize) < self.items.len(),
            "DefId({}) out of bounds (len={})",
            id.0,
            self.items.len()
        );
        &mut self.items[id.0 as usize]
    }

    pub fn mut_expr(&mut self, id: ExprId) -> &mut HirExpr {
        debug_assert!(
            (id.0 as usize) < self.exprs.len(),
            "ExprId({}) out of bounds (len={})",
            id.0,
            self.exprs.len()
        );
        &mut self.exprs[id.0 as usize]
    }

    pub fn mut_stmt(&mut self, id: StmtId) -> &mut HirStmt {
        debug_assert!(
            (id.0 as usize) < self.stmts.len(),
            "StmtId({}) out of bounds (len={})",
            id.0,
            self.stmts.len()
        );
        &mut self.stmts[id.0 as usize]
    }

    pub fn mut_body(&mut self, id: BodyId) -> &mut Body {
        debug_assert!(
            (id.0 as usize) < self.bodies.len(),
            "BodyId({}) out of bounds (len={})",
            id.0,
            self.bodies.len()
        );
        &mut self.bodies[id.0 as usize]
    }

    pub fn mut_ty(&mut self, id: TypeId) -> &mut HirType {
        debug_assert!(
            (id.0 as usize) < self.types.len(),
            "TypeId({}) out of bounds (len={})",
            id.0,
            self.types.len()
        );
        &mut self.types[id.0 as usize]
    }

    pub fn mut_impl_item(&mut self, id: ImplItemId) -> &mut ImplItem {
        debug_assert!(
            (id.0 as usize) < self.impl_items.len(),
            "ImplItemId({}) out of bounds (len={})",
            id.0,
            self.impl_items.len()
        );
        &mut self.impl_items[id.0 as usize]
    }
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub exports: FxHashMap<Symbol, ExportEntry>,
    pub items: ThinVec<DefId>,
    pub imports: FxHashMap<Symbol, DefId>,
    /// Maps struct DefId -> method name -> metadata
    pub struct_methods: FxHashMap<DefId, FxHashMap<Symbol, MethodMeta>>,
    pub struct_fields: FxHashMap<DefId, FxHashMap<Symbol, Visibility>>,
    /// Maps struct DefId -> impl block DefIds
    pub struct_impls: FxHashMap<DefId, ThinVec<DefId>>,
    /// Maps interface DefId -> impl block DefIds
    pub interface_impls: FxHashMap<DefId, ThinVec<DefId>>,
}

#[derive(Debug, Clone)]
pub struct ExportEntry {
    pub def: DefId,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct HirItem {
    pub defid: DefId,
    pub kind: HirItemKind,
    pub span: Span,
}

impl HirItem {
    pub fn module(&self) -> ModuleId {
        self.kind.module()
    }
}

#[derive(Debug, Clone)]
pub enum HirItemKind {
    Placeholder(ModuleId),
    Function(Function),
    Struct(Struct),
    Interface(Interface),
    Variable(Variable),
    Impl(Impl),
}

impl HirItemKind {
    pub fn module(&self) -> ModuleId {
        match self {
            HirItemKind::Placeholder(modid) => *modid,
            HirItemKind::Function(f) => f.module,
            HirItemKind::Struct(s) => s.module,
            HirItemKind::Interface(i) => i.module,
            HirItemKind::Variable(v) => v.module,
            HirItemKind::Impl(imp) => imp.module,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Symbol,
    pub params: ThinVec<(Symbol, TypeId)>,
    pub ret: TypeId,
    pub body: Option<BodyId>,
    pub module: ModuleId,
    /// struct defid if this is a method
    pub associated: Option<DefId>,
}

#[derive(Debug, Clone)]
pub struct Body {
    pub stmts: ThinVec<StmtId>,
}

#[derive(Debug, Clone)]
pub enum LoopSource {
    /// `loop { ... }`
    Loop,
    /// `while x { ... }`
    While,
    /// `for x : y { ... }`
    For,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: Symbol,
    pub ty: TypeId,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: Symbol,
    pub fields: ThinVec<StructField>,
    pub module: ModuleId,
}

#[derive(Debug, Clone)]
pub struct Impl {
    pub self_ty: DefId,
    pub of_interface: DefId,
    pub items: ThinVec<ImplItemId>,
    pub module: ModuleId,
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: Symbol,
    pub methods: ThinVec<InterfaceMethod>,
    pub module: ModuleId,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethod {
    pub name: Symbol,
    pub params: ThinVec<TypeId>,
    pub ret: TypeId,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: Symbol,
    pub ty: Option<TypeId>,
    pub init: Option<ExprId>,
    pub module: ModuleId,
}

#[derive(Debug, Clone)]
pub struct MethodMeta {
    pub def: DefId,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct ImplItem {
    pub defid: DefId,
    pub kind: ImplItemKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImplItemKind {
    Fn(Function),
}

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub hir_id: HirId,
    pub kind: HirExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    Error,
    Literal(Literal),
    Local(LocalId),
    Global(DefId),
    Call {
        callee: ExprId,
        args: ThinVec<ExprId>,
    },
    MethodCall {
        base: ExprId,
        method: Symbol,
        args: ThinVec<ExprId>,
    },
    Field {
        base: ExprId,
        field: Symbol,
    },
    StructInit {
        def: DefId,
        fields: ThinVec<(Symbol, ExprId)>,
    },
    /// Block expression containing a sequence of statements.
    /// NOTE: Statements are stored inline as [ThinVec] instead of using [BodyId]
    /// because blocks are simple expression values that don't need to be referenced
    /// independently.
    Block {
        stmts: ThinVec<StmtId>,
    },
    Binary {
        left: ExprId,
        op: BinOp,
        right: ExprId,
    },
    If {
        cond: ExprId,
        /// Will always point to a [HirExprKind::Block].
        /// NOTE: Using an [ExprId] instead of a [Body] is intentional
        then_branch: ExprId,
        /// Will point to a [HirExprKind::Block] or [HirExprKind::If].
        else_branch: Option<ExprId>,
    },
    Loop {
        body: BodyId,
        source: LoopSource,
    },
    Break {
        value: Option<ExprId>,
    },
    Return {
        value: Option<ExprId>,
    },
}

#[derive(Debug, Clone)]
pub struct HirStmt {
    pub hir_id: HirId,
    pub kind: HirStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    /// Expression without a trailing semicolon
    Expr(ExprId),
    /// Expression with a trailing semicolon
    Semi(ExprId),
    Let {
        name: Symbol,
        ty: Option<TypeId>,
        init: Option<ExprId>,
        local: LocalId,
    },
}

#[derive(Debug, Clone)]
pub enum HirType {
    Error,
    PrimTy(PrimTy),
    Adt(DefId),
    Pointer(TypeId, Mutability),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Math
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    // Comparisons
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
}

impl From<TokenKind> for BinOp {
    fn from(value: TokenKind) -> Self {
        use BinOp as B;
        use TokenKind as T;
        match value {
            T::Plus => B::Add,
            T::Dash => B::Sub,
            T::Star => B::Mul,
            T::Slash => B::Div,
            T::Percent => B::Rem,
            T::ShiftLeft => B::Shl,
            T::ShiftRight => B::Shr,
            T::Reference => B::BitAnd,
            T::Bar => B::BitOr,
            T::Xor => B::BitXor,
            T::EqualsEquals => B::Eq,
            T::NotEquals => B::Ne,
            T::Less => B::Lt,
            T::LessEquals => B::Le,
            T::More => B::Gt,
            T::MoreEquals => B::Ge,
            T::And => B::And,
            T::Or => B::Or,
            _ => panic!("Cannot convert token {} to BinOp", value),
        }
    }
}
