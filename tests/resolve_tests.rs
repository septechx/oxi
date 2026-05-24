use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use oxic::{
    ast::Visibility,
    context::{with_ctx, with_ctx_mut},
    hir::interner::Symbol,
    hir::{IntTy, PrimTy},
    resolve::{DefKind, Res, Resolver, ResolverOutputs, build_module_tree},
};
use thin_vec::ThinVec;

static RESOLVE_CALL_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn resolve_outputs(files: &[(&str, &str)]) -> ResolverOutputs {
    with_ctx_mut(|ctx| {
        *ctx = oxic::context::Ctx::new();
    });

    let temp_dir = PathBuf::from(".oxi/tests");
    let call_id = RESOLVE_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let test_dir = temp_dir.join(format!("resolve-{call_id}"));

    if let Err(e) = fs::create_dir_all(&test_dir) {
        panic!("Failed to create test directory: {e}");
    }

    let mut file_paths = Vec::new();
    for (filename, content) in files {
        let file_path = test_dir.join(filename);
        if let Some(parent) = file_path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            panic!("Failed to create directory: {e}");
        }
        if let Err(e) = fs::write(&file_path, content) {
            panic!("Failed to write source file {filename}: {e}");
        }
        file_paths.push(file_path);
    }

    let mut asts = ThinVec::new();
    for file_path in &file_paths {
        let source = fs::read_to_string(file_path).expect("Failed to read source file");

        let (tokens, _module_id) =
            oxic::lexer::tokenize(source, file_path).expect("Tokenization failed");
        let ast = oxic::parser::parse(tokens, file_path).expect("Parsing failed");
        oxic::ast::validate::validate_ast(&ast, oxic::hir::ModuleId(0));
        asts.push(ast);
    }

    let module_tree = build_module_tree(&asts, &file_paths).expect("Module tree building failed");

    with_ctx(|ctx| {
        if ctx
            .errors
            .has_errors_above_level(oxic::errors::ErrorLevel::Warning)
        {
            ctx.errors.print_errors(oxic::errors::ErrorLevel::Warning);
            panic!("Errors during pipeline before resolution");
        }
    });

    Resolver::assign_node_ids(&mut asts);
    with_ctx_mut(|ctx| {
        let mut resolver = Resolver::new(&asts, &module_tree, ctx);
        resolver.resolve();
        resolver.into_resolver_outputs()
    })
}

fn intern(s: &str) -> Symbol {
    with_ctx_mut(|ctx| ctx.interner.intern(s))
}

// ---------------------------------------------------------------------------
// defs tests
// ---------------------------------------------------------------------------

#[test]
fn empty_program_has_no_defs() {
    let outputs = resolve_outputs(&[("main.oxi", "")]);
    assert!(outputs.defs.is_empty(), "no items → no defs");
    assert!(outputs.def_map.is_empty());
    assert_eq!(outputs.modules.len(), 1);
    assert!(outputs.modules[0].resolutions.is_empty());
}

#[test]
fn function_def_is_recorded() {
    let outputs = resolve_outputs(&[("main.oxi", "pub fn foo() void {}")]);
    assert_eq!(outputs.defs.len(), 1);
    assert_eq!(outputs.defs[0].kind, DefKind::Function);
    assert_eq!(outputs.defs[0].visibility, Visibility::Public);
    with_ctx(|ctx| {
        assert_eq!(ctx.interner.lookup(outputs.defs[0].name), "foo");
    });
}

#[test]
fn private_function_def_has_private_visibility() {
    let outputs = resolve_outputs(&[("main.oxi", "fn foo() void {}")]);
    assert_eq!(outputs.defs.len(), 1);
    assert_eq!(outputs.defs[0].kind, DefKind::Function);
    assert_eq!(outputs.defs[0].visibility, Visibility::Private);
}

#[test]
fn struct_def_is_recorded() {
    let outputs = resolve_outputs(&[("main.oxi", "pub struct Foo { x: i32, }")]);
    assert_eq!(outputs.defs.len(), 1);
    assert_eq!(outputs.defs[0].kind, DefKind::Struct);
    with_ctx(|ctx| {
        assert_eq!(ctx.interner.lookup(outputs.defs[0].name), "Foo");
    });
}

#[test]
fn const_def_is_recorded() {
    let outputs = resolve_outputs(&[("main.oxi", "const X: i32 = 42;")]);
    assert_eq!(outputs.defs.len(), 1);
    assert_eq!(outputs.defs[0].kind, DefKind::Const);
    with_ctx(|ctx| {
        assert_eq!(ctx.interner.lookup(outputs.defs[0].name), "X");
    });
}

#[test]
fn multiple_items_are_recorded_in_order() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        pub struct Foo { x: i32, }
        pub fn bar() void {}
        const BAZ: i32 = 99;
        "#,
    )]);
    assert_eq!(outputs.defs.len(), 3);
    assert_eq!(outputs.defs[0].kind, DefKind::Struct);
    assert_eq!(outputs.defs[1].kind, DefKind::Function);
    assert_eq!(outputs.defs[2].kind, DefKind::Const);
}

#[test]
fn def_map_has_entry_per_item() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        pub fn a() void {}
        pub struct B { x: i32, }
        const C: i32 = 1;
        "#,
    )]);
    // def_map size matches defs size
    assert_eq!(outputs.def_map.len(), outputs.defs.len());
    assert_eq!(outputs.def_map.len(), 3);
}

// ---------------------------------------------------------------------------
// Module resolutions
// ---------------------------------------------------------------------------

#[test]
fn module_resolutions_contain_top_level_items() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        pub fn foo() void {}
        pub struct Bar { x: i32, }
        "#,
    )]);
    let main_mod = &outputs.modules[0];
    let foo_sym = intern("foo");
    let bar_sym = intern("Bar");
    assert!(main_mod.resolutions.contains_key(&foo_sym));
    assert!(main_mod.resolutions.contains_key(&bar_sym));
    assert_eq!(
        main_mod.resolutions[&foo_sym].best_binding().def_id,
        DefId(0)
    );
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1)
    );
}

// ---------------------------------------------------------------------------
// res_map tests
// ---------------------------------------------------------------------------

use oxic::hir::DefId;

#[test]
fn function_call_path_resolves_to_def() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        fn bar() void {}
        fn main() void { bar(); }
        "#,
    )]);
    // bar → DefId(0) (it's the first def)
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(0))));
    assert!(
        found,
        "res_map should contain an entry resolving bar to DefId(0)"
    );
}

#[test]
fn primitive_type_i32_resolves_to_prim_ty() {
    let outputs = resolve_outputs(&[("main.oxi", "fn main() i32 { return 0; }")]);
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::PrimTy(PrimTy::Int(IntTy::I32))));
    assert!(
        found,
        "res_map should contain a PrimTy(I32) entry for the `i32` type annotation"
    );
}

#[test]
fn struct_instantiation_path_resolves_to_def() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        struct Foo { x: i32, }
        fn main() void { let f = Foo { x: 1 }; }
        "#,
    )]);
    // The struct Foo should be in defs, and some path should resolve to it
    assert!(!outputs.defs.is_empty(), "there should be at least one def");
    let foo_def_id = DefId(0);
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(id) if *id == foo_def_id));
    assert!(
        found,
        "res_map should contain Foo → DefId(0); defs={}, res_map={}",
        outputs.defs.len(),
        outputs.res_map.len()
    );
}

#[test]
fn local_variable_resolves_to_local() {
    let outputs = resolve_outputs(&[("main.oxi", "fn main() i32 { let x = 42; return x; }")]);
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Local(_)));
    assert!(
        found,
        "res_map should contain a Local entry for the variable `x`"
    );
}

// ---------------------------------------------------------------------------
// Multi-module structure
// ---------------------------------------------------------------------------

#[test]
fn module_structure_file_based_child() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    assert_eq!(outputs.modules.len(), 2);
    assert_eq!(outputs.modules[0].children, vec![1usize]);
    assert_eq!(outputs.modules[1].parent, Some(0));
    assert_eq!(outputs.modules[1].qualified_name, "main::foo");
}

#[test]
fn module_structure_inline_child() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        mod math {
            pub fn add() void {}
        }
        fn main() void {}
        "#,
    )]);
    assert_eq!(outputs.modules.len(), 2);
    assert_eq!(outputs.modules[0].children, vec![1usize]);
    assert_eq!(outputs.modules[1].parent, Some(0));
    assert_eq!(outputs.modules[1].qualified_name, "main::math");
}

#[test]
fn nested_modules_structure() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod outer; fn main() void {}"),
        ("outer.oxi", "pub mod inner;"),
        ("inner.oxi", "pub fn deep() void {}"),
    ]);
    assert_eq!(outputs.modules.len(), 3);
    assert_eq!(outputs.modules[0].children, vec![1usize]);
    assert_eq!(outputs.modules[1].children, vec![2usize]);
    assert_eq!(outputs.modules[1].qualified_name, "main::outer");
    assert_eq!(outputs.modules[2].qualified_name, "main::outer::inner");
    assert_eq!(outputs.modules[2].parent, Some(1));
}

#[test]
fn child_module_contains_its_own_item_resolutions() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let foo_mod = &outputs.modules[1];
    let bar_sym = intern("bar");
    assert!(foo_mod.resolutions.contains_key(&bar_sym));
    assert_eq!(
        foo_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1)
    );
}

// ---------------------------------------------------------------------------
// Import resolution
// ---------------------------------------------------------------------------

#[test]
fn import_adds_entry_to_module_resolutions() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::bar; fn main() void { bar(); }",
        ),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "imported `bar` should appear in main module resolutions"
    );
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1)
    );
    // The call to `bar()` should resolve to bar's DefId(1)
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(found, "res_map should have `bar` → DefId(1) from the call");
}

#[test]
fn import_rename_uses_new_name() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::bar as baz; fn main() void { baz(); }",
        ),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let baz_sym = intern("baz");
    let bar_sym = intern("bar");
    assert!(
        main_mod.resolutions.contains_key(&baz_sym),
        "renamed import should create resolution for `baz`"
    );
    assert!(
        !main_mod.resolutions.contains_key(&bar_sym),
        "original name `bar` should NOT be in main module"
    );
    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().def_id,
        DefId(1)
    );
}

// ---------------------------------------------------------------------------
// Path resolution with `crate::` prefix
// ---------------------------------------------------------------------------

#[test]
fn crate_path_resolves_correctly() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; fn main() i32 { return crate::foo::bar(); }",
        ),
        ("foo.oxi", "pub fn bar() i32 { return 1; }"),
    ]);
    assert_eq!(outputs.defs.len(), 2);
    // bar → DefId(1)
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(found, "res_map should contain crate::foo::bar → DefId(1)");
}

#[test]
fn crate_path_with_inline_module() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        mod math {
            pub fn add(a: i32, b: i32) i32 { return a + b; }
        }
        fn main() i32 { return crate::math::add(1, 2); }
        "#,
    )]);
    // Tree traversal: main items first (fn main → DefId(0)),
    // then inline module items (fn add → DefId(1)).
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(found, "crate::math::add should resolve to DefId(1)");
}

// ---------------------------------------------------------------------------
// super path in child module
// ---------------------------------------------------------------------------

#[test]
fn super_path_in_child_module() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            r#"
            mod foo;
            fn top_level() i32 { return 0; }
            fn main() i32 { return foo::call_super(); }
            "#,
        ),
        (
            "foo.oxi",
            r#"
            fn call_super() i32 { return super::top_level(); }
            "#,
        ),
    ]);
    // top_level → DefId(0), main → DefId(1)
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(0))));
    assert!(found, "res_map should contain super::top_level → DefId(0)");
}

// ---------------------------------------------------------------------------
// self path
// ---------------------------------------------------------------------------

#[test]
fn self_path_resolves_within_module() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        mod inner {
            pub fn helper() i32 { return 1; }
            pub fn caller() i32 { return self::helper(); }
        }
        fn main() i32 { return inner::caller(); }
        "#,
    )]);
    // Tree traversal: main items first (fn main → DefId(0)),
    // then inner items (fn helper → DefId(1), fn caller → DefId(2)).
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(found, "self::helper should resolve to DefId(1)");
}

// ---------------------------------------------------------------------------
// Visibility: private items cannot be imported
// ---------------------------------------------------------------------------

#[test]
fn private_item_not_importable() {
    use oxic::errors::ErrorLevel;

    let _outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; import foo::bar; fn main() void {}"),
        ("foo.oxi", "fn bar() void {}"),
    ]);
    with_ctx(|ctx| {
        let has_import_error = ctx.errors.has_errors_above_level(ErrorLevel::Warning);
        assert!(
            has_import_error,
            "Should emit an error when trying to import a private item"
        );
    });
}

// ---------------------------------------------------------------------------
// Non-existent module declaration
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Module tree building failed")]
fn non_existent_module_fails() {
    let _outputs = resolve_outputs(&[("main.oxi", "mod nonexistent; fn main() void {}")]);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn only_module_declarations_no_defs() {
    let outputs = resolve_outputs(&[("main.oxi", "mod foo; fn main() void {}"), ("foo.oxi", "")]);
    let main_sym = intern("main");
    assert!(outputs.modules[0].resolutions.contains_key(&main_sym));
    assert!(outputs.modules[1].resolutions.is_empty());
}

#[test]
fn resolutions_use_non_glob_import_for_local_defs() {
    let outputs = resolve_outputs(&[("main.oxi", "pub fn foo() void {}")]);
    let foo_sym = intern("foo");
    let res = &outputs.modules[0].resolutions[&foo_sym];
    assert!(res.non_glob_import.is_some());
    assert!(res.glob_import.is_none());
}

#[test]
fn struct_name_does_not_resolve_to_prim_ty() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        struct Foo { x: i32, }
        fn main() void { let f = Foo { x: 1 }; }
        "#,
    )]);
    let found_def = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(0))));
    assert!(
        found_def,
        "Foo struct instantiation should resolve to Def(DefId(0))"
    );
}

// ---------------------------------------------------------------------------
// Re-export (pub import) tests
// ---------------------------------------------------------------------------

#[test]
fn pub_import_creates_public_binding() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; pub import foo::bar; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let binding = main_mod.resolutions[&bar_sym].best_binding();
    assert_eq!(binding.visibility, Visibility::Public);
    assert_eq!(binding.def_id, DefId(1));
}

#[test]
fn private_import_creates_private_binding() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; import foo::bar; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let binding = main_mod.resolutions[&bar_sym].best_binding();
    assert_eq!(binding.visibility, Visibility::Private);
    assert_eq!(binding.def_id, DefId(1));
}

#[test]
fn re_exported_name_resolves_from_downstream_module() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod a; import a::bar; fn main() void { bar(); }",
        ),
        ("a.oxi", "mod b; pub import b::bar;"),
        ("b.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let binding = main_mod.resolutions[&bar_sym].best_binding();
    assert_eq!(binding.def_id, DefId(1));
    // The call `bar()` should resolve in res_map
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(found, "bar() call should resolve to DefId(1)");
}

#[test]
fn private_import_not_visible_to_downstream() {
    use oxic::errors::ErrorLevel;

    let _outputs = resolve_outputs(&[
        ("main.oxi", "mod a; import a::bar; fn main() void {}"),
        ("a.oxi", "mod b; import b::bar;"),
        ("b.oxi", "pub fn bar() void {}"),
    ]);
    with_ctx(|ctx| {
        let has_error = ctx.errors.has_errors_above_level(ErrorLevel::Warning);
        assert!(
            has_error,
            "Should error when importing through a private import"
        );
    });
}

#[test]
fn re_export_with_rename() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; pub import foo::bar as baz; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let baz_sym = intern("baz");
    let bar_sym = intern("bar");
    let binding = main_mod.resolutions[&baz_sym].best_binding();
    assert_eq!(binding.def_id, DefId(1));
    assert_eq!(binding.visibility, Visibility::Public);
    assert!(
        !main_mod.resolutions.contains_key(&bar_sym),
        "original name `bar` should NOT be in main module"
    );
}

#[test]
fn cannot_re_export_private_item() {
    use oxic::errors::ErrorLevel;

    let _outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; pub import foo::bar; fn main() void {}",
        ),
        ("foo.oxi", "fn bar() void {}"),
    ]);
    with_ctx(|ctx| {
        let has_error = ctx.errors.has_errors_above_level(ErrorLevel::Warning);
        assert!(
            has_error,
            "Should error when trying to pub import a private item"
        );
    });
}

#[test]
fn re_export_chain() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod a; pub import a::bar; fn main() void {}"),
        ("a.oxi", "mod b; pub import b::bar;"),
        ("b.oxi", "mod c; pub import c::bar;"),
        ("c.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let binding = main_mod.resolutions[&bar_sym].best_binding();
    assert_eq!(binding.def_id, DefId(1));
    assert_eq!(binding.visibility, Visibility::Public);
}

// ---------------------------------------------------------------------------
// Glob import tests
// ---------------------------------------------------------------------------

#[test]
fn glob_import_brings_in_public_items() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; import foo::*; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let baz_sym = intern("baz");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "glob-imported `bar` should appear in main module"
    );
    assert!(
        main_mod.resolutions.contains_key(&baz_sym),
        "glob-imported `baz` should appear in main module"
    );
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1),
    );
    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().def_id,
        DefId(2),
    );
}

#[test]
fn glob_import_skips_private_items() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; import foo::*; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {} fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let baz_sym = intern("baz");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "public `bar` should be glob-imported"
    );
    assert!(
        !main_mod.resolutions.contains_key(&baz_sym),
        "private `baz` should NOT be glob-imported"
    );
}

#[test]
fn glob_import_from_inline_module() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        mod math {
            pub fn add(a: i32, b: i32) i32 { return a + b; }
            pub fn sub(a: i32, b: i32) i32 { return a - b; }
        }
        import math::*;
        fn main() void {}
        "#,
    )]);
    let main_mod = &outputs.modules[0];
    let add_sym = intern("add");
    let sub_sym = intern("sub");
    assert!(
        main_mod.resolutions.contains_key(&add_sym),
        "glob-imported `add` should appear in main module"
    );
    assert!(
        main_mod.resolutions.contains_key(&sub_sym),
        "glob-imported `sub` should appear in main module"
    );
    assert_eq!(
        main_mod.resolutions[&add_sym].best_binding().def_id,
        DefId(1),
    );
    assert_eq!(
        main_mod.resolutions[&sub_sym].best_binding().def_id,
        DefId(2),
    );
}

#[test]
fn glob_imported_item_resolves_in_body() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::*; fn main() void { bar(); }",
        ),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    assert!(
        found,
        "res_map should contain `bar` → DefId(1) from the call"
    );
}

#[test]
fn non_glob_import_shadows_glob_import() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; mod bar; import foo::*; import bar::baz; fn main() void { baz(); other(); }",
        ),
        ("foo.oxi", "pub fn baz() void {} pub fn other() void {}"),
        ("bar.oxi", "pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let baz_sym = intern("baz");
    let other_sym = intern("other");

    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().def_id,
        DefId(3),
        "non-glob import (bar::baz) should shadow glob import (foo::baz)"
    );

    assert_eq!(
        main_mod.resolutions[&other_sym].best_binding().def_id,
        DefId(2),
        "glob-imported `other` should resolve to foo::other"
    );

    let found = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(3))));
    assert!(found, "baz() call should resolve to bar::baz (DefId(3))");
}

#[test]
fn glob_import_uses_glob_slot() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod foo; import foo::*; fn main() void {}"),
        ("foo.oxi", "pub fn bar() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let bar_res = &main_mod.resolutions[&bar_sym];
    assert!(
        bar_res.non_glob_import.is_none(),
        "glob-imported item should not have a non_glob_import slot"
    );
    assert!(
        bar_res.glob_import.is_some(),
        "glob-imported item should use the glob_import slot"
    );
}

// ---------------------------------------------------------------------------
// Nested import tests
// ---------------------------------------------------------------------------

#[test]
fn nested_import_resolves_all_items() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::{bar, baz}; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let baz_sym = intern("baz");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "nested-imported `bar` should appear in main module"
    );
    assert!(
        main_mod.resolutions.contains_key(&baz_sym),
        "nested-imported `baz` should appear in main module"
    );
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1),
    );
    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().def_id,
        DefId(2),
    );
}

#[test]
fn nested_import_with_rename() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::{bar, baz as qux}; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let qux_sym = intern("qux");
    let baz_sym = intern("baz");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "`bar` should be imported from nested import"
    );
    assert!(
        main_mod.resolutions.contains_key(&qux_sym),
        "renamed `baz as qux` should create resolution for `qux`"
    );
    assert!(
        !main_mod.resolutions.contains_key(&baz_sym),
        "original name `baz` should NOT be in main module"
    );
    assert_eq!(
        main_mod.resolutions[&qux_sym].best_binding().def_id,
        DefId(2),
    );
}

#[test]
fn nested_import_from_inline_module() {
    let outputs = resolve_outputs(&[(
        "main.oxi",
        r#"
        mod math {
            pub fn add(a: i32, b: i32) i32 { return a + b; }
            pub fn sub(a: i32, b: i32) i32 { return a - b; }
        }
        import math::{add, sub};
        fn main() void {}
        "#,
    )]);
    let main_mod = &outputs.modules[0];
    let add_sym = intern("add");
    let sub_sym = intern("sub");
    assert!(
        main_mod.resolutions.contains_key(&add_sym),
        "nested-imported `add` from inline module should resolve"
    );
    assert!(
        main_mod.resolutions.contains_key(&sub_sym),
        "nested-imported `sub` from inline module should resolve"
    );
    // Tree traversal: root (fn main → DefId(0)), then inline module (fn add → DefId(1), fn sub → DefId(2))
    assert_eq!(
        main_mod.resolutions[&add_sym].best_binding().def_id,
        DefId(1),
    );
    assert_eq!(
        main_mod.resolutions[&sub_sym].best_binding().def_id,
        DefId(2),
    );
}

#[test]
fn deeply_nested_import() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod a; import a::{b, c::{d, e}}; fn main() void {}",
        ),
        ("a.oxi", "pub fn b() void {} pub mod c;"),
        ("c.oxi", "pub fn d() void {} pub fn e() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let b_sym = intern("b");
    let d_sym = intern("d");
    let e_sym = intern("e");
    assert!(
        main_mod.resolutions.contains_key(&b_sym),
        "`b` from `a::b` should resolve"
    );
    assert!(
        main_mod.resolutions.contains_key(&d_sym),
        "`d` from `a::c::d` should resolve"
    );
    assert!(
        main_mod.resolutions.contains_key(&e_sym),
        "`e` from `a::c::e` should resolve"
    );
    // Def order: main.oxi (fn main → 0), a.oxi (fn b → 1, mod c), c.oxi (fn d → 2, fn e → 3)
    assert_eq!(main_mod.resolutions[&b_sym].best_binding().def_id, DefId(1));
    assert_eq!(main_mod.resolutions[&d_sym].best_binding().def_id, DefId(2));
    assert_eq!(main_mod.resolutions[&e_sym].best_binding().def_id, DefId(3));
}

#[test]
fn pub_nested_import_creates_public_bindings() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; pub import foo::{bar, baz}; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let baz_sym = intern("baz");
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().visibility,
        Visibility::Public,
        "pub nested import should give `bar` public visibility"
    );
    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().visibility,
        Visibility::Public,
        "pub nested import should give `baz` public visibility"
    );
}

#[test]
fn nested_re_export_through_module() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod a; import a::{bar, baz}; fn main() void { bar(); baz(); }",
        ),
        ("a.oxi", "pub mod b; pub import b::{bar, baz};"),
        ("b.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let main_mod = &outputs.modules[0];
    let bar_sym = intern("bar");
    let baz_sym = intern("baz");
    assert!(
        main_mod.resolutions.contains_key(&bar_sym),
        "`bar` re-exported through nested import should resolve"
    );
    assert!(
        main_mod.resolutions.contains_key(&baz_sym),
        "`baz` re-exported through nested import should resolve"
    );
    assert_eq!(
        main_mod.resolutions[&bar_sym].best_binding().def_id,
        DefId(1),
    );
    assert_eq!(
        main_mod.resolutions[&baz_sym].best_binding().def_id,
        DefId(2),
    );
    let found_bar = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    let found_baz = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(2))));
    assert!(found_bar, "bar() call should resolve to DefId(1)");
    assert!(found_baz, "baz() call should resolve to DefId(2)");
}

#[test]
fn nested_import_private_item_errors() {
    use oxic::errors::ErrorLevel;

    let _outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::{bar, baz}; fn main() void {}",
        ),
        ("foo.oxi", "pub fn bar() void {} fn baz() void {}"),
    ]);
    with_ctx(|ctx| {
        let has_error = ctx.errors.has_errors_above_level(ErrorLevel::Warning);
        assert!(
            has_error,
            "Should error when nested importing a private item"
        );
    });
}

#[test]
fn nested_import_items_resolve_in_body() {
    let outputs = resolve_outputs(&[
        (
            "main.oxi",
            "mod foo; import foo::{bar, baz}; fn main() void { bar(); baz(); }",
        ),
        ("foo.oxi", "pub fn bar() void {} pub fn baz() void {}"),
    ]);
    let found_bar = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(1))));
    let found_baz = outputs
        .res_map
        .values()
        .any(|res| matches!(res, Res::Def(DefId(2))));
    assert!(found_bar, "bar() call should resolve to DefId(1)");
    assert!(found_baz, "baz() call should resolve to DefId(2)");
}

// ---------------------------------------------------------------------------
// Module tree module count
// ---------------------------------------------------------------------------

#[test]
fn module_count_matches_module_tree() {
    let outputs = resolve_outputs(&[
        ("main.oxi", "mod a; mod b; fn main() void {}"),
        ("a.oxi", "pub fn a_fn() void {}"),
        ("b.oxi", "pub fn b_fn() void {}"),
    ]);
    assert_eq!(outputs.modules.len(), 3);
    assert_eq!(outputs.modules[0].children.len(), 2);
    assert_eq!(outputs.modules[1].qualified_name, "main::a");
    assert_eq!(outputs.modules[2].qualified_name, "main::b");
}
