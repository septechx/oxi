mod common;

use common::with;
use oxic::errors::ErrorLevel;

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
                let a = []Foo{Foo { val: 21 }};
                a.ptr@.val + a.len
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn type_error_on_binary_tail_in_void_fn() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() void {
                loop {
                    break 12;
                } + true
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn no_type_error_on_literal_tail_matching_return() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                42
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn can_compile_nested_block_with_implicit_returns() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() void {
                {
                    {
                        {}
                    }
                }
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn can_compile_simple_nested_block() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() void {
                {
                    {
                        {};
                    };
                };
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
        .succeeds(false);
    })
}

#[test]
fn can_compile_program_with_shebang() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            #!/usr/bin/env oxic
            pub fn main() void {}
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn can_compile_empty_program() {
    with(|ctx| {
        ctx.add_source("main.oxi", "").succeeds(true);
    })
}

#[test]
fn slice_literals() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() void {
                let s = []u8{1, 2, 3};
            }
        "#,
        )
        .succeeds(true);
    })
}

#[test]
fn test_attribute_works() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
                #[test]
                pub fn main() usize {
                    return 42;
                }
                "#,
        )
        .succeeds(true)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn attribute_with_arguments() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
                #[foo(bar, baz)]
                pub fn main() usize {
                    return 10;
                }
                "#,
        )
        .succeeds(true)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn multiple_attributes() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
                #[test]
                #[foo]
                pub fn main() usize {
                    return 5;
                }
                "#,
        )
        .succeeds(true)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn main_fn_declaration() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
        pub fn main() usize {
            return 0;
        }
    "#,
        )
        .succeeds(true);
    })
}

#[test]
fn variable_declaration() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
        pub fn main() usize {
            let a = 2;
            return a;
        }
    "#,
        )
        .succeeds(true);
    })
}

#[test]
fn multiple_variables_and_addition() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
        pub fn main() usize {
            let a = 1;
            let b = 2;
            let c = a + b;
            return c;
        }
    "#,
        )
        .succeeds(true);
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
fn string_literals_and_slice_operations() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
        pub fn main() usize {
            let s = "Hello world!";
            let ptr = s.ptr;
            let len = s.len;
            return len;
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
fn main_function_return_void() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() void {}
        "#,
        )
        .succeeds(true);
    })
}

#[test]
fn integer_literals() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let a: i32 = 10;
                let b: i32 = -5;
                return a + b;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn boolean_literals() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                let a: bool = true;
                return a;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn char_literals() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() u8 {
                let a: u8 = 'a';
                return a;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn string_literals() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() usize {
                let s: []u8 = "hello";
                return s.len;
            }
            "#,
        )
        .succeeds(true);
    });
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
fn float_literals() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let a: f64 = 1.5;
                let b: f64 = 2.5;
                return 1;
            }
            "#,
        )
        .succeeds(true);
    });
}

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

#[test]
fn method_call_autoborrow_self_ref() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
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
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn pipe_call_autoborrow_first_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,

                pub fn new(val: i32) Self {
                    return Self { val };
                }

                pub fn get_val(self: &Self) i32 {
                    return self.val;
                }
            }

            pub fn main() i32 {
                return Foo::new(3) |> Foo::get_val;
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn pipe_call_explicit_ref_first_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,

                pub fn new(val: i32) Self {
                    return Self { val };
                }

                pub fn get_val(self: &Self) i32 {
                    return self.val;
                }
            }

            pub fn main() i32 {
                return &Foo::new(3) |> Foo::get_val;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn function_call_no_autoborrow_first_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            fn id(x: &i32, y: i32) i32 {
                return y;
            }

            pub fn main() i32 {
                let x: i32 = 5;
                return id(x, 6);
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn function_call_explicit_ref_first_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            fn id(x: &i32, y: i32) i32 {
                return y;
            }

            pub fn main() i32 {
                return id(&5, 6);
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn method_call_value_receiver_unchanged() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,

                pub fn get(self: Self) i32 {
                    return self.val;
                }
            }

            pub fn main() i32 {
                let foo = Foo { val: 9 };
                return foo.get();
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn method_call_no_autoborrow_explicit_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,

                pub fn inspect(self: &Self, x: &i32) i32 {
                    return self.val + x@;
                }
            }

            pub fn main() i32 {
                let foo = Foo { val: 3 };
                let arg: i32 = 5;
                return foo.inspect(arg);
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn method_call_explicit_ref_arg() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct Foo {
                val: i32,

                pub fn inspect(self: &Self, x: &i32) i32 {
                    return self.val + x@;
                }
            }

            pub fn main() i32 {
                let foo = Foo { val: 3 };
                return foo.inspect(&5);
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn valid_cast_expr() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let x = 1 as i32;
                return x;
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
        .succeeds(false);
    })
}

#[test]
fn pointer_to_primitive_field_access() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn foo(x: &i32) i32 {
                return x.bar;
            }

            pub fn main() i32 {
                return foo(&5);
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn invalid_cast_operand_type_error() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                return (1 + true) as i32;
            }
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn greek_letter_variable_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let α = 42;
                let β = α + 1;
                return β;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn greek_letter_function_identifier() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn π() i32 {
                return 314;
            }

            fn main() i32 {
                return π();
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn cyrillic_letter_variable_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let переменная = 1;
                let значение = переменная + 2;
                return значение;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn mixed_ascii_and_unicode_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let my_α = 1;
                let π_value = 2;
                return my_α + π_value;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn emoji_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let 😀 = 42;
                return 😀;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn multiple_emoji_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let 🚀 = 10;
                let 🌟 = 20;
                return 🚀 + 🌟;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn struct_emoji_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            struct 🚀 {
                ❤️: i32,
            }
            fn main() i32 {
                let 🤠 = 🚀 { ❤️: 12 };
                let 🌟 = 20;
                return 🤠.❤️ + 🌟;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn mixed_emoji_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let aa🚀b = 10;
                let 🌟d = 20;
                return aa🚀b + 🌟d;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn greek_letter_struct_identifiers() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            struct Δύο {
                α: i32,
                β: i32,
            }

            fn main() i32 {
                let d = Δύο { α: 1, β: 2 };
                return d.α + d.β;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn greek_letter_function_parameter() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn add(α: i32, β: i32) i32 {
                return α + β;
            }

            fn main() i32 {
                return add(10, 20);
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn tail_expr_at_end_of_block_succeeds() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let x = 1;
                let y = 2;
                x + y
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn tail_expr_in_nested_block_succeeds() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let x = {
                    let y = 1;
                    y + 2
                };
                x
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn tail_expr_not_at_tail_of_block_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                1 + 2
                let x = 3;
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_not_at_tail_of_nested_block_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                {
                    1 + 2
                    let x = 3;
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_not_at_tail_of_if_block_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                if true {
                    1 + 2
                    let x = 3;
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_not_at_tail_of_while_body_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                while true {
                    1 + 2
                    let x = 3;
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_not_at_tail_of_loop_body_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                loop {
                    1 + 2
                    let x = 3;
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_at_tail_of_while_body_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                while true {
                    1 + 2
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn tail_expr_at_tail_of_loop_body_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                loop {
                    1 + 2
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn bare_inline_block_as_loop_statement_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                loop {
                    {
                        1 + 2
                    }
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn bare_if_as_loop_statement_fails() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                loop {
                    if true {
                        1 + 2
                    }
                }
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn block_tail_inside_loop_not_as_statement_succeeds() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                loop {
                    let x = { 1 + 2 };
                    break x;
                }
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_type_matches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let a: i32 = 42;
                return a;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_type_mismatches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn foo() u8 {
                return 42;
            }

            fn main() void {
                let a: i32 = foo();
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn let_explicit_type_matches_fn_return() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn bar() i32 {
                return 10;
            }

            fn main() i32 {
                let a: i32 = bar();
                return a;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_type_mismatches_fn_return() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn bar() u8 {
                return 10;
            }

            fn main() void {
                let a: i32 = bar();
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn let_inferred_type_from_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let a = 42;
                return a;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn const_explicit_type_matches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            const X: i32 = 42;

            fn main() i32 {
                return X;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn const_explicit_type_mismatches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn foo() u8 {
                return 42;
            }

            const X: i32 = foo();

            fn main() void {}
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn const_explicit_type_matches_fn_return() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn bar() i32 {
                return 10;
            }

            const X: i32 = bar();

            fn main() i32 {
                return X;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn const_explicit_type_mismatches_fn_return() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn bar() u8 {
                return 10;
            }

            const X: i32 = bar();

            fn main() void {}
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn let_explicit_bool_type_matches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                let b: bool = true;
                return b;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_bool_type_mismatches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn foo() i32 {
                return 42;
            }

            fn main() void {
                let b: bool = foo();
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn let_explicit_float_type_matches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() f64 {
                let x: f64 = 3.14;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_u8_type_matches_char() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() u8 {
                let c: u8 = 'A';
                return c;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_slice_type_matches_string() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() usize {
                let s: []u8 = "hello";
                return s.len;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn let_explicit_slice_type_mismatches_init() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                let s: []i32 = "hello";
            }
            "#,
        )
        .succeeds(false);
    });
}

#[test]
fn impl_basic_success() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32,
            }

            struct S {
                v: i32,
            }

            impl I for S {
                fn f(self: &Self) i32 {
                    return self.v;
                }
            }

            pub fn main() i32 {
                let s = S { v: 42 };
                return s.f();
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn impl_self_return_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface Maker {
                fn new(v: i32) Self,
            }

            struct S {
                v: i32,
            }

            impl Maker for S {
                fn new(v: i32) Self {
                    return S { v };
                }
            }

            pub fn main() i32 {
                let s = S::new(10);
                return s.v;
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn impl_missing_interface_method() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn a(self: &Self) void,
                fn b(self: &Self) void,
            }

            struct S {}

            impl I for S {
                fn a(self: &Self) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn impl_signature_mismatch() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32,
            }

            struct S {}

            impl I for S {
                fn f(self: &Self) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn impl_conflicting_duplicate() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32,
            }

            struct S {}

            impl I for S {
                fn f(self: &Self) i32 { return 0; }
            }

            impl I for S {
                fn f(self: &Self) i32 { return 1; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn impl_wrong_self_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32,
            }

            impl I for i32 {
                fn f(self: &Self) i32 { return 0; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn impl_wrong_interface() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            struct S {
                v: i32,
            }

            impl i32 for S {
                fn f(self: &Self) i32 { return 0; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn impl_multiple_methods_call_all() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn a(self: &Self) i32,
                fn b(self: &Self) i32,
                fn c(self: &Self) i32,
            }

            struct S {
                x: i32,
                y: i32,
                z: i32,
            }

            impl I for S {
                fn a(self: &Self) i32 { return self.x; }
                fn b(self: &Self) i32 { return self.y; }
                fn c(self: &Self) i32 { return self.z; }
            }

            pub fn main() i32 {
                let s = S { x: 1, y: 2, z: 3 };
                return s.a() + s.b() + s.c();
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn impl_cross_module() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            mod ifaces;
            mod structs;

            import ifaces::I;
            import structs::S;

            impl I for S {
                fn f(self: &Self) i32 { return self.v; }
                fn new(v: i32) Self { return S { v }; }
            }

            pub fn main() i32 {
                let s = S::new(5);
                return s.f();
            }
            "#,
        )
        .add_source(
            "ifaces.oxi",
            r#"
            pub interface I {
                fn f(self: &Self) i32,
                fn new(v: i32) Self,
            }
            "#,
        )
        .add_source(
            "structs.oxi",
            r#"
            pub struct S {
                v: i32,
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn impl_two_interfaces_two_structs() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface A {
                fn a(self: &Self) i32,
            }

            interface B {
                fn b(self: &Self) i32,
            }

            struct X {
                v: i32,
            }

            struct Y {
                v: i32,
            }

            impl A for X {
                fn a(self: &Self) i32 { return self.v; }
            }

            impl B for Y {
                fn b(self: &Self) i32 { return self.v; }
            }

            pub fn main() i32 {
                let x = X { v: 10 };
                let y = Y { v: 20 };
                return x.a() + y.b();
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn interface_empty_impl_empty() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface Empty {}

            struct S {}

            impl Empty for S {}

            pub fn main() void {}
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn impl_method_with_multiple_params() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn process(self: &Self, a: i32, b: i32) i32,
            }

            struct S {}

            impl I for S {
                fn process(self: &Self, a: i32, b: i32) i32 {
                    return a + b;
                }
            }

            pub fn main() i32 {
                let s = S {};
                return s.process(10, 20);
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn interface_used_as_param_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn bar(self: &I) void,
            }

            struct S {}

            impl I for S {
                fn bar(self: &I) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn interface_used_as_return_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn make() Self,
            }

            struct S {}

            impl I for S {
                fn make() I { return S {}; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn interface_used_as_param_type_in_free_fn() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &Self) void,
            }

            fn bar(x: I) void {}

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn interface_used_as_struct_field_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &Self) void,
            }

            struct S {
                x: I,
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}

#[test]
fn interface_used_in_ptr_param_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &I) void,
            }

            struct S {}

            impl I for S {
                fn foo(self: &I) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false);
    })
}
