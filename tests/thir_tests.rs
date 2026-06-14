use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use oxic::{
    ast::validate::validate_ast,
    context::{Ctx, with_ctx, with_ctx_mut},
    errors::ErrorLevel,
    hir::AstLoweringContext,
    lexer::tokenize,
    parser::parse,
    resolve::{Resolver, build_module_tree},
    thir::{Expr, ExprKind, ThirCrate, lower_thir, scope::build_scope_trees},
    typeck::{Ty, typeck_crate},
};
use thin_vec::ThinVec;

static THIR_CALL_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn compile_to_thir(source: &str) -> ThirCrate {
    with_ctx_mut(|ctx| {
        *ctx = Ctx::new();
    });

    let temp_dir = PathBuf::from(".oxi/tests");
    let call_id = THIR_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let test_dir = temp_dir.join(format!("thir-{call_id}"));

    if let Err(e) = fs::create_dir_all(&test_dir) {
        panic!("Failed to create test directory: {e}");
    }

    let file_path = test_dir.join("main.oxi");
    if let Err(e) = fs::write(&file_path, source) {
        panic!("Failed to write source file: {e}");
    }

    let file_paths = vec![file_path];
    let mut asts = ThinVec::new();
    for file_path in &file_paths {
        let source = fs::read_to_string(file_path).expect("Failed to read source file");

        let (tokens, module_id) = tokenize(source, file_path).expect("Tokenization failed");
        let ast = parse(tokens, file_path).expect("Parsing failed");
        validate_ast(&ast, module_id);
        asts.push(ast);
    }

    with_ctx_mut(|ctx| {
        Resolver::assign_node_ids(ctx, &mut asts);
    });

    let module_tree =
        build_module_tree(&asts, &file_paths, "main").expect("Module tree building failed");

    with_ctx(|ctx| {
        if ctx.errors.has_errors_above_level(ErrorLevel::Warning) {
            ctx.errors.print_errors(ErrorLevel::Warning);
            panic!("Errors during pipeline before resolution");
        }
    });

    let resolver = with_ctx_mut(|ctx| {
        let mut resolver = Resolver::new(&asts, &module_tree, ctx);
        resolver.resolve();
        resolver.into_resolver_outputs()
    });

    let mut hir_crate = with_ctx_mut(|ctx| {
        let mut lowering_ctx = AstLoweringContext::new(ctx, &asts, &module_tree, &resolver);
        lowering_ctx.lower_crate()
    });

    let typeck = with_ctx_mut(|ctx| typeck_crate(ctx, &mut hir_crate, &resolver));
    typeck.assert_no_errors();

    let scope_trees = build_scope_trees(&hir_crate);
    lower_thir(&hir_crate, &typeck, &scope_trees)
}

fn find_exprs<F>(thir: &ThirCrate, filter: F) -> Vec<Expr>
where
    F: Fn(&Expr) -> bool,
{
    let mut results = Vec::new();
    for body in thir.bodies.values() {
        for expr in &body.exprs {
            if filter(expr) {
                results.push(expr.clone());
            }
        }
    }
    results
}

#[test]
fn test_autoref_type() {
    let source = r#"
        struct Foo {
            val: i32,
            pub fn get_val(self: &Self) i32 {
                return self.val;
            }
        }
        pub fn main() i32 {
            let foo = Foo { val: 7 };
            return foo.get_val();
        }
    "#;
    let thir = compile_to_thir(source);

    let borrow_exprs = find_exprs(&thir, |e| matches!(e.kind, ExprKind::Borrow { .. }));
    assert!(
        !borrow_exprs.is_empty(),
        "Should find at least one Borrow expression"
    );

    for borrow_expr in borrow_exprs {
        match &borrow_expr.ty {
            Ty::Ptr(inner, _) => match inner.as_ref() {
                Ty::Adt(_) => {}
                other => panic!("Expected Borrow inner type to be Adt, found {:?}", other),
            },
            other => panic!("Expected Borrow type to be Ptr, found {:?}", other),
        }
    }
}

#[test]
fn test_autoderef_type() {
    let source = r#"
        struct Foo {
            val: i32,
        }
        pub fn main(foo: &Foo) i32 {
            return foo.val;
        }
    "#;
    let thir = compile_to_thir(source);

    let deref_exprs = find_exprs(&thir, |e| matches!(e.kind, ExprKind::Deref { .. }));
    assert!(
        !deref_exprs.is_empty(),
        "Should find at least one Deref expression"
    );

    for deref_expr in deref_exprs {
        match &deref_expr.ty {
            Ty::Adt(_) => {}
            other => panic!("Expected Deref type to be Adt, found {:?}", other),
        }
    }
}
