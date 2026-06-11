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
