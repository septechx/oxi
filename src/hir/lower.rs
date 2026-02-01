use thin_vec::{ThinVec, thin_vec};

use crate::{
    ast::{
        AssocItem, AssocItemKind, Ast, Expr, ExprKind, Fn, Ident, ItemKind, Stmt, StmtKind, Type,
        TypeKind, Visibility,
    },
    hashmap::FxHashMap,
    hir::{
        Body, BodyId, DefId, ExportEntry, ExprId, Function, HirCrate, HirExpr, HirExprKind, HirId,
        HirItem, HirItemKind, HirStmt, HirStmtKind, HirType, Impl, ImplItem, ImplItemId,
        ImplItemKind, Interface, InterfaceMethod, LocalId, LoopSource, MethodMeta, ModuleId,
        ModuleInfo, StmtId, Struct, StructField, TypeId, Variable, interner::Symbol,
    },
    lexer::token::TokenKind,
    span::Span,
};

const BUILTIN_TYPES: [&str; 15] = [
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "f128",
    "bool", "void",
];

#[derive(Debug, Default)]
pub struct LoweringContext {
    pub krate: HirCrate,
    pub current_module: Option<ModuleId>,
    current_struct: Option<DefId>,
    /// The current owner for generating HirIds
    current_owner: Option<DefId>,
    /// Counter for local ids within the current owner
    next_local_id: u32,
    local_stack: ThinVec<FxHashMap<Symbol, LocalId>>,
    next_def: u32,
    next_expr: u32,
    next_type: u32,
    next_stmt: u32,
    next_body: u32,
    next_hir_id: u32,
}

impl LoweringContext {
    pub fn new() -> Self {
        LoweringContext::default()
    }

    /// Create a new HirId for the given local id within the current owner
    fn hir_id(&self, local_id: LocalId) -> HirId {
        HirId {
            owner: self.current_owner.expect("current owner must be set"),
            local_id,
        }
    }

    /// Allocate a new local id for HIR elements (expressions/statements) within the current owner
    fn alloc_hir_id(&mut self) -> HirId {
        let local_id = LocalId(self.next_hir_id);
        self.next_hir_id += 1;
        self.hir_id(local_id)
    }

    /// Set the current owner for generating HirIds
    fn with_owner<T>(&mut self, owner: DefId, f: impl FnOnce(&mut Self) -> T) -> T {
        let old_owner = self.current_owner;
        let old_local_id = self.next_local_id;
        let old_hir_id = self.next_hir_id;
        self.current_owner = Some(owner);
        self.next_local_id = 0;
        self.next_hir_id = 0;
        let result = f(self);
        self.current_owner = old_owner;
        self.next_local_id = old_local_id;
        self.next_hir_id = old_hir_id;
        result
    }

    pub fn lower_crate(&mut self, asts: ThinVec<Ast>) {
        for ast in &asts {
            let modinfo = ModuleInfo {
                name: ast.name.to_string(),
                exports: FxHashMap::default(),
                items: ThinVec::new(),
                imports: FxHashMap::default(),
                struct_methods: FxHashMap::default(),
                struct_fields: FxHashMap::default(),
                struct_impls: FxHashMap::default(),
                interface_impls: FxHashMap::default(),
            };
            self.krate.modules.push(modinfo);
        }

        self.collect_definitions(&asts);
        self.resolve_all_imports(&asts);
        self.lower_bodies(asts);
    }

    fn collect_definitions(&mut self, asts: &[Ast]) {
        // PASS 1: Collect top-level definitions
        for (mid, ast) in asts.iter().enumerate() {
            self.current_module = Some(ModuleId(mid as u32));
            for item in ast.items.iter() {
                match &item.kind {
                    ItemKind::Fn(f) => {
                        let sym = self.krate.interner.intern(&f.name.value);
                        let defid = self.alloc_item_placeholder(item.span);
                        self.krate.modules[mid].exports.insert(
                            sym,
                            ExportEntry {
                                def: defid,
                                visibility: item.visibility,
                            },
                        );
                        self.krate.modules[mid].items.push(defid);
                    }
                    ItemKind::Struct {
                        name,
                        fields,
                        items,
                    } => {
                        let sym = self.krate.interner.intern(&name.value);
                        let defid = self.alloc_item_placeholder(item.span);
                        self.krate.modules[mid].exports.insert(
                            sym,
                            ExportEntry {
                                def: defid,
                                visibility: item.visibility,
                            },
                        );
                        self.krate.modules[mid].items.push(defid);

                        let mut method_map = FxHashMap::default();
                        for item in items.iter() {
                            let AssocItemKind::Fn(fn_decl) = &item.kind;
                            let method_sym = self.krate.interner.intern(&fn_decl.name.value);
                            let method_defid = self.alloc_item_placeholder(item.span);
                            method_map.insert(
                                method_sym,
                                MethodMeta {
                                    def: method_defid,
                                    is_static: item.is_static,
                                    visibility: item.visibility,
                                },
                            );
                        }
                        self.krate.modules[mid]
                            .struct_methods
                            .insert(defid, method_map);

                        let mut field_map = FxHashMap::default();
                        for field in fields.iter() {
                            let field_sym = self.krate.interner.intern(&field.0.value);
                            field_map.insert(field_sym, field.2);
                        }
                        self.krate.modules[mid]
                            .struct_fields
                            .insert(defid, field_map);
                    }
                    ItemKind::Interface { name, .. } => {
                        let sym = self.krate.interner.intern(&name.value);
                        let defid = self.alloc_item_placeholder(item.span);
                        self.krate.modules[mid].exports.insert(
                            sym,
                            ExportEntry {
                                def: defid,
                                visibility: item.visibility,
                            },
                        );
                        self.krate.modules[mid].items.push(defid);
                    }
                    ItemKind::Static { name, .. } => {
                        let sym = self.krate.interner.intern(&name.value);
                        let defid = self.alloc_item_placeholder(item.span);
                        self.krate.modules[mid].exports.insert(
                            sym,
                            ExportEntry {
                                def: defid,
                                visibility: item.visibility,
                            },
                        );
                        self.krate.modules[mid].items.push(defid);
                    }
                    ItemKind::Impl { .. } => {} // Processed in lowering pass 3
                    ItemKind::Import(_) => {}   // Processed in lowering pass 2
                }
            }
        }
    }

    fn lower_bodies(&mut self, asts: ThinVec<Ast>) {
        // PASS 3: Lower definition bodies
        for (mid, ast) in asts.into_iter().enumerate() {
            self.current_module = Some(ModuleId(mid as u32));
            for item in ast.items {
                let span = item.span;
                match item.kind {
                    ItemKind::Fn(f) => self.lower_fn_decl(f),
                    ItemKind::Struct {
                        name,
                        fields,
                        items,
                    } => self.lower_struct_decl(name, fields, items, span),
                    ItemKind::Interface { name, items } => {
                        self.lower_interface_decl(name, items, span)
                    }
                    ItemKind::Impl {
                        self_ty,
                        interface,
                        items,
                    } => self.lower_impl_stmt(self_ty, interface, items),
                    ItemKind::Static { name, ty, value } => {
                        self.lower_static_item(name, ty, value, span)
                    }
                    ItemKind::Import(_) => {} // Processed in lowering pass 2
                }
            }
        }
    }

    fn alloc_item_placeholder(&mut self, span: Span) -> DefId {
        let modid = self.current_module.expect("current module set");
        let defid = DefId(self.next_def);
        self.next_def += 1;
        self.krate.items.push(HirItem {
            defid,
            kind: HirItemKind::Placeholder(modid),
            span,
        });
        defid
    }

    fn alloc_expr(&mut self, kind: HirExprKind, span: Span) -> ExprId {
        let id = ExprId(self.next_expr);
        self.next_expr += 1;
        let hir_id = self.alloc_hir_id();
        self.krate.exprs.push(HirExpr { hir_id, kind, span });
        id
    }

    fn alloc_type(&mut self, ty: HirType) -> TypeId {
        let id = TypeId(self.next_type);
        self.next_type += 1;
        self.krate.types.push(ty);
        id
    }

    /// Allocate a new local id for variables in scopes
    fn alloc_local(&mut self) -> LocalId {
        let id = LocalId(self.next_local_id);
        self.next_local_id += 1;
        id
    }

    fn alloc_stmt(&mut self, kind: HirStmtKind, span: Span) -> StmtId {
        let id = StmtId(self.next_stmt);
        self.next_stmt += 1;
        let hir_id = self.alloc_hir_id();
        self.krate.stmts.push(HirStmt { hir_id, kind, span });
        id
    }

    fn alloc_body(&mut self, body: Body) -> BodyId {
        let id = BodyId(self.next_body);
        self.next_body += 1;
        self.krate.bodies.push(body);
        id
    }

    fn lower_fn_impl(&mut self, f: Fn, defid: DefId, associated: Option<DefId>, is_static: bool) {
        let sym = self.krate.interner.intern(&f.name.value);
        let modid = self.current_module.expect("current module set");

        let params = f
            .parameters
            .into_iter()
            .map(|(pname, pty)| {
                (
                    self.krate.interner.intern(&pname.value),
                    self.lower_type(pty),
                )
            })
            .collect();

        let ret = self.lower_type(f.return_type);

        let func = Function {
            name: sym,
            params,
            ret,
            body: None,
            module: modid,
            associated,
            static_method: is_static,
        };

        let param_names: ThinVec<Symbol> = func.params.iter().map(|(name, _)| *name).collect();

        // Store the function in the item with the owner set to itself
        self.with_owner(defid, |ctx| {
            ctx.krate.items[defid.0 as usize].kind = HirItemKind::Function(func);

            if let Some(body) = f.body {
                ctx.local_stack.push(FxHashMap::default());

                for pname in param_names {
                    let local = ctx.alloc_local();
                    ctx.local_stack
                        .last_mut()
                        .expect("local stack exists")
                        .insert(pname, local);
                }

                let stmt_ids: ThinVec<StmtId> = body
                    .stmts
                    .into_iter()
                    .map(|stmt| ctx.lower_stmt(stmt))
                    .collect();

                let body_id = ctx.alloc_body(Body { stmts: stmt_ids });
                if let HirItemKind::Function(func) = &mut ctx.krate.items[defid.0 as usize].kind {
                    func.body = Some(body_id);
                }
                ctx.local_stack.pop();
            }
        });
    }

    fn lower_fn_decl(&mut self, f: Fn) {
        let sym = self.krate.interner.intern(&f.name.value);
        let defid = self.lookup_in_current_module(sym).expect("def must exist");
        self.lower_fn_impl(f, defid, None, false);
    }

    fn lower_struct_decl(
        &mut self,
        sname: Ident,
        sfields: ThinVec<(Ident, Type, Visibility)>,
        sitems: ThinVec<AssocItem>,
        span: Span,
    ) {
        let sym = self.krate.interner.intern(&sname.value);
        let modid = self.current_module.expect("current module set");
        let defid = self.lookup_in_current_module(sym).expect("def must exist");

        let fields = sfields
            .into_iter()
            .map(|(fname, fty, fvis)| StructField {
                name: self.krate.interner.intern(&fname.value),
                ty: self.lower_type(fty),
                visibility: fvis,
            })
            .collect();

        let st = Struct {
            name: sym,
            fields,
            module: modid,
        };
        let item = &mut self.krate.items[defid.0 as usize];
        item.kind = HirItemKind::Struct(st);
        item.span = span;

        let prev_struct = self.current_struct;
        self.current_struct = Some(defid);

        let mut method_fns = Vec::with_capacity(sitems.len());
        if let Some(methods_map) = self.krate.modules[modid.0 as usize]
            .struct_methods
            .get(&defid)
        {
            for item in sitems.into_iter() {
                let AssocItemKind::Fn(fn_decl) = item.kind;
                let method_sym = self.krate.interner.intern(&fn_decl.name.value);
                let meta = methods_map
                    .get(&method_sym)
                    .expect("method placeholder must exist");

                let method_defid = meta.def;

                method_fns.push((fn_decl, method_defid, defid, item.is_static));
            }
        }

        for (fn_decl, method_defid, defid, is_static) in method_fns {
            self.lower_fn_impl(fn_decl, method_defid, Some(defid), is_static);
        }

        self.current_struct = prev_struct;
    }

    fn alloc_impl_item(&mut self, kind: ImplItemKind, span: Span) -> ImplItemId {
        let id = ImplItemId(self.krate.impl_items.len() as u32);
        self.krate.impl_items.push(ImplItem {
            defid: DefId(self.next_def),
            kind,
            span,
        });
        self.next_def += 1;
        id
    }

    fn lower_interface_decl(&mut self, iname: Ident, iitems: ThinVec<AssocItem>, span: Span) {
        let sym = self.krate.interner.intern(&iname.value);
        let modid = self.current_module.expect("current module set");
        let defid = self
            .lookup_in_current_module(sym)
            .expect("interface def must exist");

        let mut methods = ThinVec::with_capacity(iitems.len());
        for item in iitems.into_iter() {
            let AssocItemKind::Fn(fn_decl) = item.kind;
            let method_name = self.krate.interner.intern(&fn_decl.name.value);

            let param_tys = fn_decl
                .parameters
                .into_iter()
                .map(|arg| self.lower_type(arg.1))
                .collect::<ThinVec<_>>();

            let ret_ty = self.lower_type(fn_decl.return_type);

            methods.push(InterfaceMethod {
                name: method_name,
                params: param_tys,
                ret: ret_ty,
            })
        }

        let iface = Interface {
            name: sym,
            module: modid,
            methods,
        };
        self.krate.items[defid.0 as usize].kind = HirItemKind::Interface(iface);
        self.krate.items[defid.0 as usize].span = span;
    }

    fn lower_impl_stmt(&mut self, self_ty: Type, iface: Ident, items: ThinVec<AssocItem>) {
        let interface_sym = self.krate.interner.intern(&iface.value);
        let interface_def = match self.lookup_in_current_module(interface_sym) {
            Some(def) => def,
            None => {
                self.krate
                    .diagnostics
                    .push(format!("Unknown interface `{}`", iface.value));
                return;
            }
        };

        let self_defid = match &self_ty.kind {
            TypeKind::Symbol(s) => {
                let sym = self.krate.interner.intern(&s.name.value);
                match self.lookup_in_current_module(sym) {
                    Some(def) => def,
                    None => {
                        self.krate
                            .diagnostics
                            .push(format!("Unknown type `{}` in impl", s.name.value));
                        return;
                    }
                }
            }
            _ => {
                self.krate
                    .diagnostics
                    .push("Impl target must be a named type".to_string());
                return;
            }
        };

        let modid = self.current_module.expect("current module set");
        match &self.krate.items[self_defid.0 as usize].kind {
            HirItemKind::Struct(_) => {}
            _ => {
                self.krate
                    .diagnostics
                    .push("Impl target is not a struct".to_string());
                return;
            }
        }

        let impl_defid = self.alloc_item_placeholder(self_ty.span);
        let self_ty_id = self.lower_type(self_ty);

        let mut impl_item_ids = ThinVec::with_capacity(items.len());

        for item in items.into_iter() {
            let AssocItemKind::Fn(fn_decl) = item.kind;
            let method_sym = self.krate.interner.intern(&fn_decl.name.value);

            let param_names: ThinVec<Symbol> = fn_decl
                .parameters
                .iter()
                .map(|(pname, _)| self.krate.interner.intern(&pname.value))
                .collect();

            let params = fn_decl
                .parameters
                .into_iter()
                .map(|(pname, pty)| {
                    (
                        self.krate.interner.intern(&pname.value),
                        self.lower_type(pty),
                    )
                })
                .collect();

            let ret = self.lower_type(fn_decl.return_type);

            let func = Function {
                name: method_sym,
                params,
                ret,
                body: None,
                module: modid,
                associated: Some(self_defid),
                static_method: item.is_static,
            };

            let impl_item_id = self.alloc_impl_item(ImplItemKind::Fn(func), item.span);
            impl_item_ids.push(impl_item_id);

            let method_defid = self.krate.impl_items[impl_item_id.0 as usize].defid;
            let method_map = &mut self.krate.modules[modid.0 as usize]
                .struct_methods
                .entry(self_defid)
                .or_default();
            method_map.insert(
                method_sym,
                MethodMeta {
                    def: method_defid,
                    is_static: item.is_static,
                    visibility: item.visibility,
                },
            );

            if let Some(body) = fn_decl.body {
                self.with_owner(method_defid, |ctx| {
                    ctx.local_stack.push(FxHashMap::default());

                    for pname in param_names {
                        let local = ctx.alloc_local();
                        ctx.local_stack
                            .last_mut()
                            .expect("local stack exists")
                            .insert(pname, local);
                    }

                    let stmt_ids: ThinVec<StmtId> = body
                        .stmts
                        .into_iter()
                        .map(|stmt| ctx.lower_stmt(stmt))
                        .collect();

                    let body_id = ctx.alloc_body(Body { stmts: stmt_ids });
                    let ImplItemKind::Fn(func) =
                        &mut ctx.krate.impl_items[impl_item_id.0 as usize].kind;
                    func.body = Some(body_id);
                    ctx.local_stack.pop();
                });
            }
        }

        let impl_item = Impl {
            self_ty: self_ty_id,
            of_interface: interface_def,
            items: impl_item_ids,
            module: modid,
        };

        self.krate.items[impl_defid.0 as usize].kind = HirItemKind::Impl(impl_item);

        self.krate.modules[modid.0 as usize]
            .struct_impls
            .entry(self_defid)
            .or_default()
            .push(impl_defid);

        self.krate.modules[modid.0 as usize]
            .interface_impls
            .entry(interface_def)
            .or_default()
            .push(impl_defid);
    }

    fn lower_static_item(&mut self, sname: Ident, sty: Type, svalue: Expr, span: Span) {
        let sym = self.krate.interner.intern(&sname.value);
        let modid = self.current_module.expect("current module set");
        let defid = self.lookup_in_current_module(sym).expect("def must exist");

        let ty = if let TypeKind::Infer = sty.kind {
            None
        } else {
            Some(self.lower_type(sty))
        };

        // Lower the init expression with the owner set to the static item itself
        let init = self.with_owner(defid, |ctx| Some(ctx.lower_expr(svalue)));

        let var = Variable {
            name: sym,
            ty,
            init,
            module: modid,
        };
        self.krate.items[defid.0 as usize].kind = HirItemKind::Variable(var);
        self.krate.items[defid.0 as usize].span = span;
    }

    fn lower_expr(&mut self, expr: Expr) -> ExprId {
        debug_assert!(self.current_owner.is_some());
        let span = expr.span;
        match expr.kind {
            ExprKind::Literal(l) => self.alloc_expr(HirExprKind::Literal(l), span),
            ExprKind::Symbol(s) => {
                let sym = self.krate.interner.intern(&s.value);

                for scope in self.local_stack.iter().rev() {
                    if let Some(local) = scope.get(&sym) {
                        return self.alloc_expr(HirExprKind::Local(*local), span);
                    }
                }

                if let Some(defid) = self.lookup_in_current_module(sym) {
                    return self.alloc_expr(HirExprKind::Global(defid), span);
                }

                self.krate
                    .diagnostics
                    .push(format!("Unknown symbol {}", s.value));
                self.alloc_expr(HirExprKind::Error, span)
            }
            ExprKind::FunctionCall { callee, parameters } => match *callee {
                Expr {
                    kind:
                        ExprKind::MemberAccess {
                            base,
                            member,
                            operator,
                        },
                    ..
                } if operator.kind == TokenKind::Dot => {
                    let base_id = self.lower_expr(*base);
                    let method_sym = self.krate.interner.intern(&member.value);
                    let args = parameters.into_iter().map(|a| self.lower_expr(a)).collect();
                    self.alloc_expr(
                        HirExprKind::MethodCall {
                            base: base_id,
                            method: method_sym,
                            args,
                        },
                        span,
                    )
                }
                _ => {
                    let callee_id = self.lower_expr(*callee);
                    let args = parameters.into_iter().map(|a| self.lower_expr(a)).collect();
                    self.alloc_expr(
                        HirExprKind::Call {
                            callee: callee_id,
                            args,
                        },
                        span,
                    )
                }
            },
            ExprKind::StructInstantiation {
                name,
                fields: expr_fields,
            } => {
                let def_sym = self.krate.interner.intern(&name.value);
                if let Some(defid) = self.lookup_in_current_module(def_sym) {
                    let mut fields = ThinVec::with_capacity(expr_fields.len());
                    for (ident, val) in expr_fields.into_iter() {
                        let fsym = self.krate.interner.intern(&ident.value);
                        let v = self.lower_expr(val);
                        fields.push((fsym, v));
                    }
                    self.alloc_expr(HirExprKind::StructInit { def: defid, fields }, span)
                } else {
                    self.krate
                        .diagnostics
                        .push(format!("Unknown struct name: {}", name.value));
                    self.alloc_expr(HirExprKind::Error, span)
                }
            }
            ExprKind::MemberAccess {
                base,
                member,
                operator,
            } => {
                if operator.kind == TokenKind::ColonColon {
                    let base_id = self.lower_expr(*base);
                    let member_sym = self.krate.interner.intern(&member.value);

                    if let HirExprKind::Global(defid) = &self.krate.exprs[base_id.0 as usize].kind {
                        let defid = *defid;
                        let struct_mod = self.krate.items[defid.0 as usize].kind.module();

                        if let Some(methods) = self.krate.modules[struct_mod.0 as usize]
                            .struct_methods
                            .get(&defid)
                            && let Some(meta) = methods.get(&member_sym)
                        {
                            if meta.visibility == Visibility::Private {
                                let allowed = self.current_struct == Some(defid);

                                if !allowed {
                                    self.krate.diagnostics.push(format!(
                                        "Cannot access private associated item `{}`",
                                        member.value
                                    ));
                                }
                            }
                            return self.alloc_expr(HirExprKind::Global(meta.def), span);
                        }

                        // Also could be a module, but we don't have good module-as-value yet in HIR.
                        // Assuming simple associated item access for now.
                    }

                    self.krate
                        .diagnostics
                        .push(format!("Cannot resolve associated item `{}`", member.value));
                    self.alloc_expr(HirExprKind::Error, span)
                } else {
                    let base_id = self.lower_expr(*base);
                    let field_sym = self.krate.interner.intern(&member.value);
                    self.alloc_expr(
                        HirExprKind::Field {
                            base: base_id,
                            field: field_sym,
                        },
                        span,
                    )
                }
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_id = self.lower_expr(*left);
                let right_id = self.lower_expr(*right);
                let op = operator.kind.into();
                self.alloc_expr(
                    HirExprKind::Binary {
                        left: left_id,
                        op,
                        right: right_id,
                    },
                    span,
                )
            }
            ExprKind::Block(b) => {
                let stmts = self.lower_body(b.stmts);
                self.alloc_expr(HirExprKind::Block { stmts }, span)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(*condition);

                let then_stmts = self.lower_body(then_branch.stmts);
                let then_span = if then_stmts.is_empty() {
                    span
                } else {
                    let first = self.krate.stmt(then_stmts[0]).span;
                    let last = self
                        .krate
                        .stmt(*then_stmts.last().expect("non-empty then_stmts"))
                        .span;
                    Span::new(first.start(), last.end())
                };
                let then_block =
                    self.alloc_expr(HirExprKind::Block { stmts: then_stmts }, then_span);

                let else_block = else_branch.map(|e| self.lower_expr(*e));

                self.alloc_expr(
                    HirExprKind::If {
                        cond,
                        then_branch: then_block,
                        else_branch: else_block,
                    },
                    span,
                )
            }
            ExprKind::While { condition, body } => {
                let cond = self.lower_expr(*condition);

                let then_stmts = self.lower_body(body.stmts);
                let body_span = if then_stmts.is_empty() {
                    span
                } else {
                    let first = self.krate.stmt(then_stmts[0]).span;
                    let last = self
                        .krate
                        .stmt(*then_stmts.last().expect("non-empty body stmts"))
                        .span;
                    Span::new(first.start(), last.end())
                };
                let then_block =
                    self.alloc_expr(HirExprKind::Block { stmts: then_stmts }, body_span);

                let break_expr = self.alloc_expr(HirExprKind::Break { value: None }, span);
                let break_stmt = self.alloc_stmt(HirStmtKind::Semi(break_expr), span);
                let else_block = self.alloc_expr(
                    HirExprKind::Block {
                        stmts: thin_vec![break_stmt],
                    },
                    span,
                );

                let if_expr = self.alloc_expr(
                    HirExprKind::If {
                        cond,
                        then_branch: then_block,
                        else_branch: Some(else_block),
                    },
                    span,
                );
                let if_stmt = self.alloc_stmt(HirStmtKind::Semi(if_expr), span);
                let loop_body = self.alloc_body(Body {
                    stmts: thin_vec![if_stmt],
                });

                self.alloc_expr(
                    HirExprKind::Loop {
                        body: loop_body,
                        source: LoopSource::While,
                    },
                    span,
                )
            }
            ExprKind::Loop(l) => {
                let stmts = self.lower_body(l.stmts);
                let body = self.alloc_body(Body { stmts });
                self.alloc_expr(
                    HirExprKind::Loop {
                        body,
                        source: LoopSource::Loop,
                    },
                    span,
                )
            }
            ExprKind::Break(b) => {
                let value = b.map(|e| self.lower_expr(*e));
                self.alloc_expr(HirExprKind::Break { value }, span)
            }
            ExprKind::Return(r) => {
                let value = r.map(|e| self.lower_expr(*e));
                self.alloc_expr(HirExprKind::Return { value }, span)
            }
            _ => todo!("Lowering of {:?} not implemented", expr.kind),
        }
    }

    fn lower_stmt(&mut self, stmt: Stmt) -> StmtId {
        debug_assert!(self.current_owner.is_some());
        let span = stmt.span;
        match stmt.kind {
            StmtKind::Expr(expr) => {
                let exprid = self.lower_expr(expr);
                self.alloc_stmt(HirStmtKind::Expr(exprid), span)
            }
            StmtKind::Semi(expr) => {
                let exprid = self.lower_expr(expr);
                self.alloc_stmt(HirStmtKind::Semi(exprid), span)
            }
            StmtKind::Let {
                name,
                ty,
                value,
                mutability: _,
            } => {
                let name_sym = self.krate.interner.intern(&name.value);
                let ty_id = if let TypeKind::Infer = ty.kind {
                    None
                } else {
                    Some(self.lower_type(ty))
                };
                let init_expr = value.map(|e| self.lower_expr(e));

                let local = self.alloc_local();

                if let Some(scope) = self.local_stack.last_mut() {
                    scope.insert(name_sym, local);
                }

                let var = HirStmtKind::Let {
                    name: name_sym,
                    ty: ty_id,
                    init: init_expr,
                    local,
                };
                self.alloc_stmt(var, span)
            }
        }
    }

    fn lower_type(&mut self, ty: Type) -> TypeId {
        match ty.kind {
            TypeKind::Symbol(s) => {
                let name = s.name.value;

                if BUILTIN_TYPES.contains(&name.as_ref()) {
                    return self.alloc_type(HirType::Builtin(name.into()));
                }

                let sym = self.krate.interner.intern(&name);
                if let Some(defid) = self.lookup_in_current_module(sym) {
                    return self.alloc_type(HirType::Adt(defid));
                }

                self.krate
                    .diagnostics
                    .push(format!("Unknown type: {}", name));
                self.alloc_type(HirType::Error)
            }
            TypeKind::Pointer(p) => {
                let inner = *p.underlying;
                let tid = self.lower_type(inner);
                self.alloc_type(HirType::Pointer(tid, p.mutability))
            }
            _ => todo!("Lowering of {:?} not implemented", ty.kind),
        }
    }

    fn lower_body(&mut self, body: ThinVec<Stmt>) -> ThinVec<StmtId> {
        self.local_stack.push(FxHashMap::default());
        let stmts = body
            .into_iter()
            .map(|s| self.lower_stmt(s))
            .collect::<ThinVec<_>>();
        self.local_stack.pop();
        stmts
    }
}
