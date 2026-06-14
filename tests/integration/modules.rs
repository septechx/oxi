use crate::common::with;

#[test]
fn mod_declaration_file_based() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod foo;

            fn main() i32 {
                return foo::bar();
            }
            "#,
        )
        .add_source(
            "foo.oxi",
            r#"
            pub fn bar() i32 {
                return 42;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn mod_directory_convention() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod bar;

            fn main() i32 {
                return bar::dir_conv();
            }
            "#,
        )
        .add_source(
            "bar/mod.oxi",
            r#"
            pub fn dir_conv() i32 {
                return 5;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn mod_inline() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod math {
                pub fn add(a: i32, b: i32) i32 {
                    return a + b;
                }
            }

            fn main() i32 {
                return math::add(2, 3);
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn mod_nested_file_based() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod outer;

            fn main() i32 {
                return outer::inner::deep();
            }
            "#,
        )
        .add_source(
            "outer.oxi",
            r#"
            pub mod inner;

            pub fn deep() i32 {
                return inner::deep();
            }
            "#,
        )
        .add_source(
            "inner.oxi",
            r#"
            pub fn deep() i32 {
                return 99;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn mod_nested_with_inline_child() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod outer;

            fn main() i32 {
                return outer::inner::value();
            }
            "#,
        )
        .add_source(
            "outer.oxi",
            r#"
            pub mod inner {
                pub fn value() i32 {
                    return 7;
                }
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn mod_unmatched_declaration_fails() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod nonexistent;

            fn main() i32 {
                return 0;
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn mod_crate_path_root() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod foo;

            fn main() i32 {
                return crate::foo::bar();
            }
            "#,
        )
        .add_source(
            "foo.oxi",
            r#"
            pub fn bar() i32 {
                return 10;
            }
            "#,
        )
        .succeeds(true);
    })
}
