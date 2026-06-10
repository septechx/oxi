use thin_vec::ThinVec;

use crate::ast::{Ident, Literal, Mutability};
use crate::context::Ctx;
use crate::errors::builders;
use crate::hashmap::{FxHashMap, FxHashSet};
use crate::hir::{
    self, AssocItemKind, BinOp, Block, Body, DefId, Expr, ExprKind, FloatTy, FnDecl, HirId, IntTy,
    ItemKind, MaybeOwner, ModuleId, Node, PosOp, PreOp, PrimTy, QPath, StmtKind, UintTy,
};
use crate::interner::Symbol;
use crate::resolve::{Res, ResolverOutputs};
use crate::span::Span;
use crate::typeck::env::ScopeEnv;
use crate::typeck::infctx::{InferCtx, TyVarSource};
use crate::typeck::types::{Scheme, Ty};
use crate::typeck::unify::{OrPushErr, UnifyError, UnifyResult, unify};
use crate::typeck::{CoherenceTable, MemberRes, MethodKind, Typeck};

// Labels aren't supported yet, so early returns are only checked for loops. AST Validation should
// catch uses of `break` outside of loops
#[derive(Debug)]
struct BlockTyRes {
    tail: Ty,
    early: Option<Ty>,
}

fn tail_span(expr: &Expr) -> Span {
    match &expr.kind {
        ExprKind::Block(block) => block
            .stmts
            .last()
            .and_then(|stmt| match &stmt.kind {
                StmtKind::Expr(expr) => Some(expr.span),
                _ => None,
            })
            .unwrap_or(expr.span),
        _ => expr.span,
    }
}

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(super) fn check_bodies(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        let mut node_types: FxHashMap<HirId, Ty> = FxHashMap::default();
        let mut member_res: FxHashMap<HirId, MemberRes> = FxHashMap::default();
        let mut local_schemes: FxHashMap<HirId, Scheme> = FxHashMap::default();

        let mut checker = BodyChecker {
            ctx: self.ctx,
            icx: &mut icx,
            item_schemes: &mut self.item_schemes,
            inherent_methods: &mut self.inherent_methods,
            interface_methods: &mut self.interface_methods,
            coherence: &mut self.coherence,
            resolver: self.resolver,
            node_types: &mut node_types,
            member_res: &mut member_res,
            local_schemes: &mut local_schemes,
            env: ScopeEnv::new(),
            module_id: ModuleId(0),
        };

        for (i, owner) in self.krate.owners.iter().enumerate() {
            let def_id = DefId(i as u32);

            let MaybeOwner::Owner(info) = owner else {
                continue;
            };

            let module_id = self.def_to_module.get(&def_id).expect("contains def id");
            checker.module_id = *module_id;

            match &info.nodes.nodes[0].node {
                Node::Item(item) => match &item.kind {
                    ItemKind::Fn(fun) => {
                        if let Some(body_id) = fun.body_id
                            && let Some(body) = info.nodes.body(body_id)
                        {
                            checker.check_fn_body(&fun.decl, body);
                        }
                    }
                    ItemKind::Const { ty, body_id, .. } => {
                        if let Some(body_id) = body_id
                            && let Some(body) = info.nodes.body(*body_id)
                        {
                            checker.check_const_body(ty, body);
                        }
                    }
                    _ => {}
                },
                Node::AssocItem(assoc) => {
                    let AssocItemKind::Fn(fun) = &assoc.kind;
                    if let Some(body_id) = fun.body_id
                        && let Some(body) = info.nodes.body(body_id)
                    {
                        checker.check_fn_body(&fun.decl, body);
                    }
                }
                _ => {}
            }
        }

        let int_ids = icx.vars_with_source(TyVarSource::IntLit);
        let i32_ty = Ty::Prim(PrimTy::Int(IntTy::I32));
        for var in int_ids {
            if icx.is_bound(var) {
                continue;
            }
            let var_span = icx.ty_var_span(var).unwrap_or(Span::new(0, 0));
            let var_module = icx.ty_var_module(var);
            unify(&mut icx, &Ty::Var(var), &i32_ty, var_span, var_module).or_push_err(&mut icx);
        }
        let float_ids = icx.vars_with_source(TyVarSource::FloatLit);
        let f64_ty = Ty::Prim(PrimTy::Float(FloatTy::F64));
        for var in float_ids {
            if icx.is_bound(var) {
                continue;
            }
            let var_span = icx.ty_var_span(var).unwrap_or(Span::new(0, 0));
            let var_module = icx.ty_var_module(var);
            unify(&mut icx, &Ty::Var(var), &f64_ty, var_span, var_module).or_push_err(&mut icx);
        }

        let resolved: FxHashMap<HirId, Ty> = node_types
            .iter()
            .map(|(&id, ty)| (id, icx.resolve(ty)))
            .collect();

        self.node_types.extend(resolved);
        self.member_res.extend(member_res);

        let errors = icx.take_errors();
        let resolver = self.resolver;
        for err in errors {
            let (msg, span, module_id) = format_unify_error(&err, resolver, &self.ctx.interner);
            self.ctx.errors.add(
                builders::error_at(msg, module_id, span, self.ctx),
                self.ctx.enable_printing,
            );
        }
    }
}

struct BodyChecker<'a, 'b, 'ctx, 'res> {
    icx: &'a mut InferCtx,
    item_schemes: &'b mut FxHashMap<DefId, Scheme>,
    inherent_methods: &'b mut FxHashMap<DefId, FxHashMap<Symbol, DefId>>,
    interface_methods: &'b mut FxHashMap<DefId, FxHashMap<Symbol, (DefId, DefId)>>,
    coherence: &'b mut CoherenceTable,
    ctx: &'ctx mut Ctx,
    resolver: &'res ResolverOutputs,
    node_types: &'a mut FxHashMap<HirId, Ty>,
    member_res: &'a mut FxHashMap<HirId, MemberRes>,
    local_schemes: &'a mut FxHashMap<HirId, Scheme>,
    env: ScopeEnv,
    module_id: ModuleId,
}

impl<'a, 'b, 'ctx, 'res> BodyChecker<'a, 'b, 'ctx, 'res> {
    fn unify_with_autoref(&mut self, param: &Ty, arg: &Ty, span: Span) -> UnifyResult<()> {
        let arg_r = self.icx.resolve(arg);
        let param_r = self.icx.resolve(param);
        if !matches!(arg_r, Ty::Ptr(..) | Ty::Var(_))
            && let Ty::Ptr(inner, _) = &param_r
        {
            return unify(self.icx, inner, arg, span, self.module_id);
        }
        unify(self.icx, param, arg, span, self.module_id)
    }

    fn check_const_body(&mut self, ty: &hir::Ty, body: &Body) {
        let expected = Ty::from_hir(self.icx, ty);
        let body_ty = self.check_expr(&body.value);
        if let Err(err) = unify(
            self.icx,
            &expected,
            &body_ty,
            body.value.span,
            self.module_id,
        ) {
            self.report_type_error(err);
        }
    }

    fn check_fn_body(&mut self, decl: &FnDecl, body: &Body) {
        self.env.push();
        for param in &decl.params {
            let param_ty = Ty::from_hir(self.icx, &param.ty);
            let scheme = Scheme::monomorphic(param_ty);
            self.local_schemes.insert(param.hir_id, scheme.clone());
            self.env.insert(param.hir_id, scheme);
        }
        let expected = Ty::from_hir(self.icx, &decl.ret);
        self.check_expr(&body.value);
        self.check_return_values(&body.value, &expected, body.value.span);
        if let Some(body_ty) = self.node_types.get(&body.value.hir_id).cloned()
            && let Err(err) = unify(
                self.icx,
                &expected,
                &body_ty,
                tail_span(&body.value),
                self.module_id,
            )
        {
            self.report_type_error(err);
        }
        self.env.pop();
    }

    fn check_return_values(&mut self, expr: &Expr, expected: &Ty, span: Span) {
        match &expr.kind {
            ExprKind::Return(inner) => {
                let inner = if let Some(inner) = inner {
                    self.node_types
                        .get(&inner.hir_id)
                        .cloned()
                        .unwrap_or(Ty::Error)
                } else {
                    Ty::Prim(PrimTy::Void)
                };
                if let Err(err) = unify(self.icx, expected, &inner, span, self.module_id) {
                    self.report_type_error(err);
                }
            }
            ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.check_return_values_in_block(then_branch, expected);
                if let Some(else_branch) = else_branch {
                    self.check_return_values(else_branch, expected, span);
                }
            }
            ExprKind::Block(block) => self.check_return_values_in_block(block, expected),
            ExprKind::Loop(block) => self.check_return_values_in_block(block, expected),
            _ => {}
        }
    }

    fn check_return_values_in_block(&mut self, block: &Block, expected: &Ty) {
        for stmt in &block.stmts {
            if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &stmt.kind {
                self.check_return_values(expr, expected, stmt.span);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Ty {
        let id = expr.hir_id;
        let ty = self.check_expr_kind(&expr.kind, id, expr.span);
        self.node_types.insert(id, ty.clone());
        ty
    }

    fn check_expr_kind(&mut self, kind: &ExprKind, hir_id: HirId, expr_span: Span) -> Ty {
        match kind {
            ExprKind::Error => Ty::Error,
            ExprKind::Literal(lit) => self.check_lit(lit, expr_span),
            ExprKind::Path(qpath) => self.check_path(qpath),
            ExprKind::Binary { left, op, right } => self.check_binary(left, *op, right),
            ExprKind::Prefix { op, right } => self.check_prefix(*op, right),
            ExprKind::Postfix { left, op } => self.check_postfix(left, *op),
            ExprKind::Call { callee, params } => self.check_call(callee, params, expr_span),
            ExprKind::StructInit { def, fields } => self.check_struct_init(*def, fields, expr_span),
            ExprKind::ArrayInit { ty, contents } => {
                let elem_ty = Ty::from_hir(self.icx, ty);
                for expr in contents {
                    let expr_ty = self.check_expr(expr);
                    if let Err(err) = unify(self.icx, &elem_ty, &expr_ty, expr.span, self.module_id)
                    {
                        self.report_type_error(err);
                    }
                }
                Ty::Slice(elem_ty.into_box())
            }
            ExprKind::TupleInit(elements) => {
                Ty::Tuple(elements.iter().map(|expr| self.check_expr(expr)).collect())
            }
            ExprKind::Block(block) => self.check_block(block).tail,
            ExprKind::Loop(block) => self.check_loop(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.check_expr(cond);
                let bool_ty = Ty::Prim(PrimTy::Bool);
                if let Err(err) = unify(self.icx, &bool_ty, &cond_ty, cond.span, self.module_id) {
                    self.report_type_error(err);
                }
                let then_span = then_branch.span;
                let then_ty = self.check_block(then_branch).tail;
                let else_ty = else_branch.as_ref().map(|expr| self.check_expr(expr));
                match else_ty {
                    Some(else_ty) => {
                        if let Err(err) =
                            unify(self.icx, &then_ty, &else_ty, then_span, self.module_id)
                        {
                            self.report_type_error(err);
                        }
                        then_ty
                    }
                    None => Ty::Prim(PrimTy::Void),
                }
            }
            ExprKind::Break(expr) | ExprKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr);
                }
                Ty::Never
            }
            ExprKind::Assign { target, value, .. } => {
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);
                if let Err(err) = unify(self.icx, &target_ty, &value_ty, value.span, self.module_id)
                {
                    self.report_type_error(err);
                }
                Ty::Prim(PrimTy::Void)
            }
            ExprKind::MemberAccess { base, member } => {
                self.check_member_access(*member, base, hir_id)
            }
            ExprKind::As { ty, .. } => Ty::from_hir(self.icx, ty),
            ExprKind::MethodCall { .. } | ExprKind::Field { .. } => {
                // MethodCall and Field cannot exitst yet
                unreachable!()
            }
        }
    }

    fn check_lit(&mut self, lit: &Literal, span: Span) -> Ty {
        match lit {
            Literal::Integer(_) => Ty::Var(self.icx.next_int_var(span, self.module_id)),
            Literal::Float(_) => Ty::Var(self.icx.next_float_var(span, self.module_id)),
            Literal::Bool(_) => Ty::Prim(PrimTy::Bool),
            Literal::Char(_) => Ty::Prim(PrimTy::Uint(UintTy::U8)),
            Literal::String(_) => Ty::Slice(Ty::Prim(PrimTy::Uint(UintTy::U8)).into_box()),
        }
    }

    fn check_path(&mut self, qpath: &QPath) -> Ty {
        match qpath {
            QPath::Resolved(path) => match &path.res {
                Res::Def(def_id) => match self.item_schemes.get(def_id) {
                    Some(scheme) => self.icx.instantiate(scheme),
                    None => Ty::Error,
                },
                Res::Local(id) => {
                    if let Some(scheme) = self.env.get(id) {
                        self.icx.instantiate(scheme)
                    } else {
                        Ty::Error
                    }
                }
                Res::PrimTy(prim) => Ty::Prim(*prim),
                Res::SelfTyAlias { alias_to } => Ty::Adt(*alias_to),
                Res::Err => Ty::Error,
            },
            QPath::TypeRelative { .. } => Ty::Error,
        }
    }

    fn qpath_recv_ty(&self, qpath: &QPath) -> Option<Ty> {
        let base = match qpath {
            QPath::Resolved(path) => match &path.res {
                Res::Def(def_id) => Ty::Adt(*def_id),
                Res::SelfTyAlias { alias_to } => Ty::Adt(*alias_to),
                _ => return None,
            },
            QPath::TypeRelative { qself, .. } => return self.qpath_recv_ty(qself),
        };
        let base = self.icx.resolve(&base);
        if matches!(base, Ty::Adt(_)) {
            Some(base)
        } else {
            None
        }
    }

    fn check_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> Ty {
        let left_span = left.span;
        let right_span = right.span;
        let left = self.check_expr(left);
        let right = self.check_expr(right);
        match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                unify(self.icx, &left, &right, left_span, self.module_id).or_push_err(self.icx);
                Ty::Prim(PrimTy::Bool)
            }
            BinOp::And | BinOp::Or => {
                let bool_ty = Ty::Prim(PrimTy::Bool);
                unify(self.icx, &bool_ty, &left, left_span, self.module_id).or_push_err(self.icx);
                unify(self.icx, &bool_ty, &right, right_span, self.module_id).or_push_err(self.icx);
                bool_ty
            }
            _ => {
                unify(self.icx, &left, &right, left_span, self.module_id).or_push_err(self.icx);
                left
            }
        }
    }

    fn check_prefix(&mut self, op: PreOp, right: &Expr) -> Ty {
        let right_span = right.span;
        let right = self.check_expr(right);
        match op {
            PreOp::Not => {
                let bool_ty = Ty::Prim(PrimTy::Bool);
                unify(self.icx, &bool_ty, &right, right_span, self.module_id).or_push_err(self.icx);
                bool_ty
            }
            PreOp::Neg => {
                let resolved = self.icx.resolve(&right);
                if let Ty::Prim(PrimTy::Int(_) | PrimTy::Float(_)) = resolved {
                    return resolved;
                }
                right
            }
            PreOp::Ref => Ty::Ptr(right.into_box(), Mutability::Constant),
        }
    }

    fn check_postfix(&mut self, left: &Expr, _op: PosOp) -> Ty {
        let span = left.span;
        let left = self.check_expr(left);
        match left {
            Ty::Ptr(inner, _) => (*inner).clone(),
            _ => {
                self.ctx.errors.add(
                    builders::error_at(
                        "Cannot dereference non-pointer type",
                        self.module_id,
                        span,
                        self.ctx,
                    ),
                    self.ctx.enable_printing,
                );
                Ty::Error
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, params: &ThinVec<Expr>, call_span: Span) -> Ty {
        let callee_span = callee.span;

        if let Some((recv_ty, member, is_method_call)) = match &callee.kind {
            ExprKind::MemberAccess { base, member } => Some((self.check_expr(base), *member, true)),
            ExprKind::Path(QPath::TypeRelative { qself, segment }) => self
                .qpath_recv_ty(qself)
                .map(|ty| (ty, segment.value, false)),
            _ => None,
        } {
            return self.check_member_call(
                callee,
                callee_span,
                call_span,
                recv_ty,
                member,
                is_method_call,
                params,
            );
        }

        self.check_direct_call(callee, callee_span, call_span, params)
    }

    #[allow(clippy::too_many_arguments)]
    fn check_member_call(
        &mut self,
        callee: &Expr,
        callee_span: Span,
        call_span: Span,
        recv_ty: Ty,
        member: Symbol,
        is_method_call: bool,
        params: &ThinVec<Expr>,
    ) -> Ty {
        let Some((def_id, kind)) = self.resolve_method(&recv_ty, member) else {
            self.ctx.errors.add(
                builders::error_at("Method not found", self.module_id, callee_span, self.ctx),
                self.ctx.enable_printing,
            );
            return Ty::Error;
        };

        let Some(scheme) = self.item_schemes.get(&def_id).cloned() else {
            return Ty::Error;
        };

        let instantiated = self.icx.instantiate(&scheme);
        let Ty::Fn {
            params: param_tys,
            ret,
        } = instantiated
        else {
            return Ty::Error;
        };

        if !self.check_call_args(
            params,
            &param_tys,
            Some(&recv_ty),
            is_method_call,
            call_span,
        ) {
            return Ty::Error;
        }

        self.member_res
            .insert(callee.hir_id, MemberRes::Method { def_id, kind });

        *ret
    }

    fn check_direct_call(
        &mut self,
        callee: &Expr,
        callee_span: Span,
        call_span: Span,
        params: &ThinVec<Expr>,
    ) -> Ty {
        let callee_ty = self.check_expr(callee);

        match callee_ty {
            Ty::Fn {
                params: param_tys,
                ret,
            } => {
                if !self.check_call_args(params, &param_tys, None, false, call_span) {
                    return Ty::Error;
                }

                *ret
            }
            _ => {
                self.ctx.errors.add(
                    builders::error_at(
                        "Cannot call non-function expression",
                        self.module_id,
                        callee_span,
                        self.ctx,
                    ),
                    self.ctx.enable_printing,
                );
                Ty::Error
            }
        }
    }

    fn check_call_args(
        &mut self,
        args: &ThinVec<Expr>,
        param_tys: &[Ty],
        recv_ty: Option<&Ty>,
        is_method_call: bool,
        call_span: Span,
    ) -> bool {
        let arg_tys = if is_method_call && !param_tys.is_empty() {
            &param_tys[1..]
        } else {
            param_tys
        };

        if args.len() != arg_tys.len() {
            let expected = arg_tys.len();
            let err = format!(
                "expected {} parameter{}, found {}",
                expected,
                if expected == 1 { "" } else { "s" },
                args.len(),
            );
            self.ctx.errors.add(
                builders::error_at(err, self.module_id, call_span, self.ctx),
                self.ctx.enable_printing,
            );
            return false;
        }

        if let Some(recv_ty) = recv_ty
            && is_method_call
            && !param_tys.is_empty()
        {
            let first = param_tys.first().expect("method has at least 1 param");

            self.unify_with_autoref(first, recv_ty, call_span)
                .or_push_err(self.icx);
        }

        for (i, arg) in args.iter().enumerate() {
            let arg_span = arg.span;
            let arg_ty = self.check_expr(arg);
            let expected_ty = &arg_tys[i];

            if i == 0 {
                self.unify_with_autoref(expected_ty, &arg_ty, arg_span)
                    .or_push_err(self.icx);
            } else {
                unify(self.icx, expected_ty, &arg_ty, arg_span, self.module_id)
                    .or_push_err(self.icx);
            }
        }

        true
    }

    fn resolve_method(&self, recv_ty: &Ty, member: Symbol) -> Option<(DefId, MethodKind)> {
        let recv_ty = self.icx.resolve(recv_ty);
        let Ty::Adt(struct_id) = recv_ty else {
            return None;
        };

        if let Some(method) = self.inherent_methods.get(&struct_id)
            && let Some(&method_def_id) = method.get(&member)
        {
            return Some((method_def_id, MethodKind::Inherent));
        }

        if let Some(method) = self.interface_methods.get(&struct_id)
            && let Some(&(iface, method_def_id)) = method.get(&member)
        {
            let impl_def = *self
                .coherence
                .impls
                .get(&(iface, struct_id))
                .expect("impl exists");
            return Some((method_def_id, MethodKind::Interface { iface, impl_def }));
        }
        None
    }

    fn check_struct_init(&mut self, def: DefId, fields: &ThinVec<(Ident, Expr)>, span: Span) -> Ty {
        let struct_ty = Ty::Adt(def);
        let field_table = self
            .coherence
            .struct_fields
            .get(&def)
            .cloned()
            .unwrap_or_default();

        let sym_to_span: FxHashMap<Symbol, Span> = fields
            .iter()
            .map(|(name, _)| (name.value, name.span))
            .collect();
        let init_names: FxHashSet<Symbol> = fields.iter().map(|(name, _)| name.value).collect();
        let struct_names: FxHashSet<Symbol> = field_table.keys().copied().collect();

        for name in init_names.difference(&struct_names) {
            self.ctx.errors.add(
                builders::error_at(
                    format!("unknown field `{}`", self.ctx.interner.lookup(*name)),
                    self.module_id,
                    *sym_to_span.get(name).expect("field exists"),
                    self.ctx,
                ),
                self.ctx.enable_printing,
            );
        }

        for name in struct_names.difference(&init_names) {
            self.ctx.errors.add(
                builders::error_at(
                    format!("missing field `{}`", self.ctx.interner.lookup(*name)),
                    self.module_id,
                    span,
                    self.ctx,
                ),
                self.ctx.enable_printing,
            );
        }

        for (name, expr) in fields {
            let expr_span = expr.span;
            let expr = self.check_expr(expr);
            if let Some(field_ty) = field_table.get(&name.value)
                && let Err(err) = unify(self.icx, field_ty, &expr, expr_span, self.module_id)
            {
                self.icx.errors.push(err);
            }
        }

        struct_ty
    }

    fn check_block(&mut self, block: &Block) -> BlockTyRes {
        self.env.push();
        let mut break_ty = None;
        let mut last_ty = Ty::Prim(PrimTy::Void);
        let mut diverged = false;
        for stmt in &block.stmts {
            if diverged {
                break;
            }
            match &stmt.kind {
                StmtKind::Let {
                    ty, init, local, ..
                } => {
                    let ty = Ty::from_hir(self.icx, ty);
                    self.icx.push_level();
                    let init_span = init.as_ref().map(|e| e.span).unwrap_or(stmt.span);
                    let bound = init
                        .as_ref()
                        .map(|expr| self.check_expr(expr))
                        .unwrap_or_else(|| ty.clone());
                    if let Err(err) = unify(self.icx, &ty, &bound, init_span, self.module_id) {
                        self.report_type_error(err);
                    }
                    let scope = self.icx.current_level();
                    let parent = scope.saturating_sub(1);
                    let quantified = self.icx.generalize(&ty, parent);
                    let scheme = Scheme {
                        vars: quantified,
                        body: ty.clone(),
                    };
                    self.local_schemes.insert(*local, scheme.clone());
                    self.env.insert(*local, scheme);
                    self.icx.pop_level();
                }
                StmtKind::Expr(expr) => {
                    'blk: {
                        if let ExprKind::Break(break_expr) = &expr.kind {
                            let break_value_ty = break_expr
                                .as_ref()
                                .map_or(Ty::Prim(PrimTy::Void), |expr| self.check_expr(expr));

                            if let Some(existing_break_ty) = &break_ty
                                && let Err(err) = unify(
                                    self.icx,
                                    existing_break_ty,
                                    &break_value_ty,
                                    expr.span,
                                    self.module_id,
                                )
                            {
                                self.report_type_error(err);
                                break 'blk;
                            }

                            break_ty = Some(break_value_ty);
                        }
                    }

                    last_ty = self.check_expr(expr);
                    if matches!(last_ty, Ty::Never) {
                        diverged = true;
                    }
                }
                StmtKind::Semi(expr) => {
                    'blk: {
                        if let ExprKind::Break(break_expr) = &expr.kind {
                            let break_value_ty = break_expr
                                .as_ref()
                                .map_or(Ty::Prim(PrimTy::Void), |expr| self.check_expr(expr));

                            if let Some(existing_break_ty) = &break_ty
                                && let Err(err) = unify(
                                    self.icx,
                                    existing_break_ty,
                                    &break_value_ty,
                                    expr.span,
                                    self.module_id,
                                )
                            {
                                self.report_type_error(err);
                                break 'blk;
                            }

                            break_ty = Some(break_value_ty);
                        }
                    }

                    let expr = self.check_expr(expr);
                    if matches!(expr, Ty::Never) {
                        last_ty = Ty::Never;
                        diverged = true;
                    } else {
                        last_ty = Ty::Prim(PrimTy::Void)
                    }
                }
            }
        }
        self.env.pop();
        BlockTyRes {
            tail: if diverged { Ty::Never } else { last_ty },
            early: break_ty,
        }
    }

    fn check_loop(&mut self, block: &Block) -> Ty {
        self.check_block(block)
            .early
            .unwrap_or_else(|| Ty::Prim(PrimTy::Void))
    }

    fn check_member_access(&mut self, member: Symbol, base: &Expr, hir_id: HirId) -> Ty {
        let recv_ty = self.check_expr(base);
        let recv_ty = self.icx.resolve(&recv_ty);
        if let Ty::Adt(struct_id) = &recv_ty
            && let Some(fields) = self.coherence.struct_fields.get(struct_id)
            && let Some(field_ty) = fields.get(&member)
        {
            let index = fields
                .keys()
                .enumerate()
                .find_map(|(i, &name)| if name == member { Some(i) } else { None });
            if let Some(index) = index {
                self.member_res.insert(hir_id, MemberRes::Field { index });
            }
            return field_ty.clone();
        }
        Ty::Error
    }

    fn report_type_error(&mut self, err: UnifyError) {
        let (msg, span, module_id) = format_unify_error(&err, self.resolver, &self.ctx.interner);
        self.ctx.errors.add(
            builders::error_at(msg, module_id, span, self.ctx),
            self.ctx.enable_printing,
        );
    }
}

fn format_unify_error(
    err: &UnifyError,
    resolver: &ResolverOutputs,
    interner: &crate::interner::Interner,
) -> (String, Span, ModuleId) {
    match err {
        UnifyError::Mismatch {
            expected,
            found,
            span,
            module_id,
        } => (
            format!(
                "Type mismatch: expected `{}`, found `{}`",
                ty_display(expected, resolver, interner),
                ty_display(found, resolver, interner)
            ),
            *span,
            *module_id,
        ),
        UnifyError::OcurrsCheck {
            span, module_id, ..
        } => ("Recursive type detected".to_string(), *span, *module_id),
    }
}

fn ty_display(ty: &Ty, resolver: &ResolverOutputs, interner: &crate::interner::Interner) -> String {
    match ty {
        Ty::Var(_) => "<var>".to_string(),
        Ty::Prim(p) => p.name_str().to_string(),
        Ty::Ptr(inner, m) => format!(
            "&{}{}",
            if matches!(m, crate::ast::Mutability::Mutable) {
                "mut "
            } else {
                ""
            },
            ty_display(inner, resolver, interner)
        ),
        Ty::Slice(inner) => format!("[]{}", ty_display(inner, resolver, interner)),
        Ty::Array(inner, n) => format!("[{}]{}", n, ty_display(inner, resolver, interner)),
        Ty::Fn { params, ret } => {
            let ps: Vec<String> = params
                .iter()
                .map(|t| ty_display(t, resolver, interner))
                .collect();
            format!(
                "({}) -> {}",
                ps.join(", "),
                ty_display(ret, resolver, interner)
            )
        }
        Ty::Tuple(elements) => {
            let es: Vec<String> = elements
                .iter()
                .map(|t| ty_display(t, resolver, interner))
                .collect();
            format!("({})", es.join(", "))
        }
        Ty::Adt(d) => resolver.defs[d.0 as usize]
            .name
            .map(|sym| interner.lookup(sym).to_string())
            .unwrap_or_else(|| format!("Struct#{}", d.0)),
        Ty::Interface(d) => resolver.defs[d.0 as usize]
            .name
            .map(|sym| interner.lookup(sym).to_string())
            .unwrap_or_else(|| format!("Interface#{}", d.0)),
        Ty::Never => "!".to_string(),
        Ty::Error => "<error>".to_string(),
    }
}
