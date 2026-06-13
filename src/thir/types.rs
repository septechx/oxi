use thin_vec::ThinVec;

use crate::ast::{Literal, Mutability};
use crate::hashmap::FxHashMap;
use crate::hir::{BinOp, DefId, HirId, PreOp};
use crate::interner::Symbol;
use crate::span::Span;
use crate::thir::scope::Scope;
use crate::typeck::Ty;

crate::newtype_ids!(ExprId, StmtId, BlockId, LocalVarId);

#[derive(Debug, Clone)]
pub struct ThirBody {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub blocks: Vec<Block>,
    pub params: Vec<Param>,
    pub locals: FxHashMap<HirId, LocalVarId>,
    pub body_expr: ExprId,
}

#[derive(Debug, Clone)]
pub struct ThirCrate {
    pub bodies: FxHashMap<DefId, ThirBody>,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
    pub span: Span,
    pub hir_id: HirId,
    pub temp_scope: Option<Scope>,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Scope {
        region_scope: Scope,
        value: ExprId,
    },
    Literal(Literal),
    VarRef(LocalVarId),
    Path {
        def_id: DefId,
    },
    /// Zero sized type: struct Unit {}
    ZstLiteral,
    Block(BlockId),
    If {
        cond: ExprId,
        then: ExprId,
        else_opt: Option<ExprId>,
    },
    Loop {
        body: ExprId,
    },
    Break {
        value: Option<ExprId>,
    },
    Return {
        value: Option<ExprId>,
    },
    Binary {
        op: BinOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    LogicalOp {
        op: LogOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    Unary {
        op: PreOp,
        arg: ExprId,
    },
    Deref {
        arg: ExprId,
    },
    Borrow {
        kind: Mutability,
        arg: ExprId,
    },
    Call {
        callee: ExprId,
        args: ThinVec<ExprId>,
    },
    StructInit {
        def_id: DefId,
        fields: ThinVec<(Symbol, ExprId)>,
    },
    ArrayInit {
        ty: Ty,
        elements: ThinVec<ExprId>,
    },
    TupleInit(ThinVec<ExprId>),
    Assign {
        target: ExprId,
        value: ExprId,
    },
    Field {
        base: ExprId,
        field_index: usize,
    },
    Cast {
        source: ExprId,
        target_ty: Ty,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
    pub hir_id: HirId,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Expr {
        expr: ExprId,
    },
    Semi {
        expr: ExprId,
    },
    Let {
        local_var: LocalVarId,
        name: Symbol,
        ty: Ty,
        init: Option<ExprId>,
        remainder_scope: Option<Scope>,
        init_scope: Option<Scope>,
    },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub region_scope: Scope,
    pub stmts: ThinVec<StmtId>,
    pub expr: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Symbol,
    pub ty: Ty,
    pub hir_id: HirId,
    pub local_var: LocalVarId,
}
