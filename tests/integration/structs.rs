use oxic::errors::ErrorLevel;

use crate::common::with;

#[test]
fn slice_member_access_with_extra() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: usize,
            }

            fn main() usize {
                let a = [Foo { val: 21 }];
                a.ptr@.val + a.len
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn duplicate_struct_property_fails() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                a: i32,
                a: i32,
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn struct_declaration_and_initialization() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
        struct Foo {
            a: i32,
        }

        pub fn main() i32 {
            let foo = Foo {
                a: 1,
            };

            return foo.a;
        }
    "#,
        )
        .succeeds(true);
    })
}

#[test]
fn struct_with_methods() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                a: i32,

                pub fn bar(a: i32, b: i32) i32 {
                    return a - b;
                }
            }

            pub fn main() i32 {
                let foo = Foo {
                    a: 1,
                };

                return foo.a - Foo::bar(2, 1);
            }
        "#,
        )
        .succeeds(true);
    })
}

#[test]
fn struct_shorthand_initialization() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                x: i32,
                y: i32,
            }

            pub fn main() i32 {
                let x = 10;
                let foo = Foo {
                    x,
                    y: 20,
                };

                return foo.x + foo.y;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn pointer_to_struct_unknown_field() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,
            }

            pub fn main() i32 {
                let foo = Foo { val: 3 };
                let p = &foo;
                return p.nonexistent;
            }
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}
