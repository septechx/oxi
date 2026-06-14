use crate::common::with;

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
