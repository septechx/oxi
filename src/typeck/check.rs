use thin_vec::ThinVec;

use crate::ast::{Literal, Mutability};
use crate::context::Ctx;
use crate::errors::builders;
use crate::hashmap::FxHashMap;
use crate::hir::{
    self, AssocItemKind, BinOp, Block, Body, DefId, Expr, ExprKind, FloatTy, FnDecl, HirId, IntTy,
    ItemKind, MaybeOwner, ModuleId, Node, PosOp, PreOp, PrimTy, QPath, StmtKind, UintTy,
};
use crate::interner::Symbol;
use crate::resolve::Res;
use crate::span::Span;
use crate::typeck::env::ScopeEnv;
use crate::typeck::infctx::{InferCtx, TyVarSource};
use crate::typeck::types::{Scheme, Ty};
use crate::typeck::unify::{UnifyError, UnifyResult, unify};
use crate::typeck::{CoherenceTable, MemberRes, MethodKind, Typeck};

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(super) fn check_bodies(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        let def_to_module = self.build_def_to_module();

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

            let module_id = def_to_module.get(&def_id).expect("contains def id");
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
            if let Err(err) = unify(&mut icx, &Ty::Var(var), &i32_ty) {
                icx.errors.push(err);
            }
        }
        let float_ids = icx.vars_with_source(TyVarSource::FloatLit);
        let f64_ty = Ty::Prim(PrimTy::Float(FloatTy::F64));
        for var in float_ids {
            if icx.is_bound(var) {
                continue;
            }
            if let Err(err) = unify(&mut icx, &Ty::Var(var), &f64_ty) {
                icx.errors.push(err);
            }
        }

        let resolved: FxHashMap<HirId, Ty> = node_types
            .iter()
            .map(|(&id, ty)| (id, icx.resolve(ty)))
            .collect();

        self.node_types.extend(resolved);
        self.member_res.extend(member_res);

        let errors = icx.take_errors();
        for err in errors {
            let msg = format_unify_error(err);
            self.ctx
                .errors
                .add(builders::error(msg), self.ctx.enable_printing);
        }
    }

    fn build_def_to_module(&self) -> FxHashMap<DefId, ModuleId> {
        let mut map: FxHashMap<DefId, ModuleId> = FxHashMap::default();
        for (i, module) in self.resolver.modules.iter().enumerate() {
            for res in module.resolutions.values() {
                map.insert(res.best_binding().def_id, ModuleId(i as u32));
            }
            for methods in module.struct_methods.values() {
                for binding in methods.values() {
                    map.insert(binding.def_id, ModuleId(i as u32));
                }
            }
            for &impl_def_id in &module.impls {
                map.insert(impl_def_id, ModuleId(i as u32));
            }
            for &method_def_id in &module.methods {
                map.insert(method_def_id, ModuleId(i as u32));
            }
        }
        map
    }
}

struct BodyChecker<'a, 'b, 'ctx> {
    icx: &'a mut InferCtx,
    item_schemes: &'b mut FxHashMap<DefId, Scheme>,
    inherent_methods: &'b mut FxHashMap<DefId, FxHashMap<Symbol, DefId>>,
    interface_methods: &'b mut FxHashMap<DefId, FxHashMap<Symbol, (DefId, DefId)>>,
    coherence: &'b mut CoherenceTable,
    ctx: &'ctx mut Ctx,
    node_types: &'a mut FxHashMap<HirId, Ty>,
    member_res: &'a mut FxHashMap<HirId, MemberRes>,
    local_schemes: &'a mut FxHashMap<HirId, Scheme>,
    env: ScopeEnv,
    module_id: ModuleId,
}

impl<'a, 'b, 'ctx> BodyChecker<'a, 'b, 'ctx> {
    fn unify_with_autoref(&mut self, param: &Ty, arg: &Ty) -> UnifyResult<()> {
        let arg_r = self.icx.resolve(arg);
        let param_r = self.icx.resolve(param);
        if !matches!(arg_r, Ty::Ptr(..) | Ty::Var(_))
            && let Ty::Ptr(inner, _) = &param_r
        {
            return unify(self.icx, inner, arg);
        }
        unify(self.icx, param, arg)
    }

    fn check_const_body(&mut self, ty: &hir::Ty, body: &Body) {
        let expected = Ty::from_hir(self.icx, ty);
        let body_ty = self.check_expr(&body.value);
        if let Err(err) = unify(self.icx, &expected, &body_ty) {
            self.report_type_error(body.value.span, err);
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
                if let Err(err) = unify(self.icx, expected, &inner) {
                    self.report_type_error(span, err);
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
        let ty = self.check_expr_kind(&expr.kind, id);
        self.node_types.insert(id, ty.clone());
        ty
    }

    fn check_expr_kind(&mut self, kind: &ExprKind, hir_id: HirId) -> Ty {
        match kind {
            ExprKind::Error => Ty::Error,
            ExprKind::Literal(lit) => self.check_lit(lit),
            ExprKind::Path(qpath) => self.check_path(qpath),
            ExprKind::Binary { left, op, right } => self.check_binary(left, *op, right),
            ExprKind::Prefix { op, right } => self.check_prefix(*op, right),
            ExprKind::Postfix { left, op } => self.check_postfix(left, *op),
            ExprKind::Call { callee, params } => self.check_call(callee, params),
            ExprKind::StructInit { def, fields } => self.check_struct_init(*def, fields),
            ExprKind::ArrayInit { ty, contents } => {
                let elem_ty = Ty::from_hir(self.icx, ty);
                for expr in contents {
                    let expr_ty = self.check_expr(expr);
                    if let Err(err) = unify(self.icx, &elem_ty, &expr_ty) {
                        self.report_type_error(expr.span, err);
                    }
                }
                Ty::Slice(elem_ty.into_box())
            }
            ExprKind::TupleInit(elements) => {
                Ty::Tuple(elements.iter().map(|expr| self.check_expr(expr)).collect())
            }
            ExprKind::Block(block) => self.check_block(block),
            ExprKind::Loop(block) => {
                // FIXME: Loops should be able to return things using `break`
                self.check_block(block);
                Ty::Prim(PrimTy::Void)
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.check_expr(cond);
                let bool_ty = Ty::Prim(PrimTy::Bool);
                if let Err(err) = unify(self.icx, &bool_ty, &cond_ty) {
                    self.report_type_error(cond.span, err);
                }
                let then_branch = self.check_block(then_branch);
                let else_branch = else_branch.as_ref().map(|expr| self.check_expr(expr));
                match else_branch {
                    Some(else_branch) => {
                        if let Err(err) = unify(self.icx, &then_branch, &else_branch) {
                            // FIXME: Use the correct span
                            self.report_type_error(Span::new(0, 0), err);
                        }
                        then_branch
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
                if let Err(err) = unify(self.icx, &target_ty, &value_ty) {
                    self.report_type_error(value.span, err);
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

    fn check_lit(&mut self, lit: &Literal) -> Ty {
        match lit {
            Literal::Integer(_) => Ty::Var(self.icx.next_int_var()),
            Literal::Float(_) => Ty::Var(self.icx.next_float_var()),
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
        let left = self.check_expr(left);
        let right = self.check_expr(right);
        match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                if let Err(err) = unify(self.icx, &left, &right) {
                    self.icx.errors.push(err);
                }
                Ty::Prim(PrimTy::Bool)
            }
            BinOp::And | BinOp::Or => {
                let bool_ty = Ty::Prim(PrimTy::Bool);
                if let Err(err) = unify(self.icx, &bool_ty, &left) {
                    self.icx.errors.push(err);
                };
                if let Err(err) = unify(self.icx, &bool_ty, &right) {
                    self.icx.errors.push(err);
                }
                bool_ty
            }
            _ => {
                if let Err(err) = unify(self.icx, &left, &right) {
                    self.icx.errors.push(err);
                }
                left
            }
        }
    }

    fn check_prefix(&mut self, op: PreOp, right: &Expr) -> Ty {
        let right = self.check_expr(right);
        match op {
            PreOp::Not => {
                let bool_ty = Ty::Prim(PrimTy::Bool);
                if let Err(err) = unify(self.icx, &bool_ty, &right) {
                    self.icx.errors.push(err);
                }
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
        let left = self.check_expr(left);
        match left {
            Ty::Ptr(inner, _) => (*inner).clone(),
            _ => {
                // TODO: Better error reporting, needs ModuleId in Span
                panic!("ERROR: Cannot dereference non pointer type");
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, params: &ThinVec<Expr>) -> Ty {
        let call_info: Option<(Ty, Symbol, bool)> = match &callee.kind {
            ExprKind::MemberAccess { base, member } => Some((self.check_expr(base), *member, true)),
            ExprKind::Path(QPath::TypeRelative { qself, segment }) => {
                self.qpath_recv_ty(qself).map(|t| (t, segment.value, false))
            }
            _ => None,
        };
        if let Some((recv_ty, member, is_method_call)) = call_info {
            if let Some((def_id, kind)) = self.resolve_method(&recv_ty, member) {
                let scheme = match self.item_schemes.get(&def_id) {
                    Some(scheme) => scheme.clone(),
                    None => return Ty::Error,
                };
                let instantiated = self.icx.instantiate(&scheme);
                let (param_tys, ret) = match instantiated {
                    Ty::Fn { params, ret } => (params, *ret),
                    _ => return Ty::Error,
                };
                if param_tys.is_empty() {
                    if !params.is_empty() {
                        // TODO: Better error reporting, needs ModuleId in Span
                        panic!("ERROR: Function arity mismatch");
                    }
                    self.member_res
                        .insert(callee.hir_id, MemberRes::Method { def_id, kind });
                    return ret;
                }
                if is_method_call {
                    if !params.is_empty() {
                        // TODO: Better error reporting, needs ModuleId in Span
                        panic!("ERROR: Function arity mismatch");
                    }
                    if let Err(err) = self.unify_with_autoref(&param_tys[0], &recv_ty) {
                        self.icx.errors.push(err);
                    }
                } else {
                    if param_tys.len() != params.len() {
                        // TODO: Better error reporting, needs ModuleId in Span
                        panic!("ERROR: Function arity mismatch");
                    }
                    for (i, param) in params.iter().enumerate() {
                        let param = self.check_expr(param);
                        if i == 0 {
                            if let Err(err) = self.unify_with_autoref(&param_tys[i], &param) {
                                self.icx.errors.push(err);
                            }
                        } else if let Err(err) = unify(self.icx, &param_tys[i], &param) {
                            self.icx.errors.push(err);
                        }
                    }
                }
                self.member_res
                    .insert(callee.hir_id, MemberRes::Method { def_id, kind });
                return ret;
            } else {
                // TODO: Better error reporting, needs ModuleId in Span
                panic!("ERROR: Method not found");
            }
        }

        let callee = self.check_expr(callee);
        match callee {
            Ty::Fn {
                params: param_tys,
                ret,
            } => {
                if param_tys.len() != params.len() {
                    // TODO: Better error reporting, needs ModuleId in Span
                    panic!("ERROR: Function arity mismatch");
                }
                for (i, param) in params.iter().enumerate() {
                    let param = self.check_expr(param);
                    if i == 0 {
                        if let Err(err) = self.unify_with_autoref(&param_tys[i], &param) {
                            self.icx.errors.push(err);
                        }
                    } else if let Err(err) = unify(self.icx, &param_tys[i], &param) {
                        self.icx.errors.push(err);
                    }
                }
                *ret
            }
            _ => {
                // TODO: Better error reporting, needs ModuleId in Span
                panic!("ERROR: Cannot call non-function expression");
            }
        }
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

    fn check_struct_init(&mut self, def: DefId, fields: &ThinVec<(Symbol, Expr)>) -> Ty {
        //  FIXME: Check for extra/missing fields
        let struct_ty = Ty::Adt(def);
        let field_table = self
            .coherence
            .struct_fields
            .get(&def)
            .cloned()
            .unwrap_or_default();
        let field_types: Vec<_> = field_table.values().collect();
        for (i, (_, expr)) in fields.iter().enumerate() {
            let expr = self.check_expr(expr);
            if let Some(field_ty) = field_types.get(i)
                && let Err(err) = unify(self.icx, field_ty, &expr)
            {
                self.icx.errors.push(err);
            }
        }
        struct_ty
    }

    fn check_block(&mut self, block: &Block) -> Ty {
        self.env.push();
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
                    let bound = init
                        .as_ref()
                        .map(|expr| self.check_expr(expr))
                        .unwrap_or_else(|| ty.clone());
                    if let Err(err) = unify(self.icx, &ty, &bound) {
                        self.report_type_error(stmt.span, err);
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
                    last_ty = self.check_expr(expr);
                    if matches!(last_ty, Ty::Never) {
                        diverged = true;
                    }
                }
                StmtKind::Semi(expr) => {
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
        if diverged { Ty::Never } else { last_ty }
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

    fn report_type_error(&mut self, span: Span, err: UnifyError) {
        let msg = format_unify_error(err);
        self.ctx.errors.add(
            builders::error_at(msg, self.module_id, span, self.ctx),
            self.ctx.enable_printing,
        );
    }
}

fn format_unify_error(err: UnifyError) -> String {
    match err {
        UnifyError::Mismatch { expected, found } => format!(
            "Type mismatch: expected `{}', found `{}`",
            ty_display(&expected),
            ty_display(&found)
        ),
        UnifyError::OcurrsCheck(_) => "Recursive type detected".to_string(),
    }
}

fn ty_display(ty: &Ty) -> String {
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
            ty_display(inner)
        ),
        Ty::Slice(inner) => format!("[]{}", ty_display(inner)),
        Ty::Array(inner, n) => format!("[{}]{}", n, ty_display(inner)),
        Ty::Fn { params, ret } => {
            let ps: Vec<String> = params.iter().map(ty_display).collect();
            format!("({}) -> {}", ps.join(", "), ty_display(ret))
        }
        Ty::Tuple(elements) => {
            let es: Vec<String> = elements.iter().map(ty_display).collect();
            format!("({})", es.join(", "))
        }
        Ty::Adt(d) => format!("Struct#{}", d.0),
        Ty::Interface(d) => format!("Interface#{}", d.0),
        Ty::Never => "!".to_string(),
        Ty::Error => "<error>".to_string(),
    }
}
