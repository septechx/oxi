use oxic::errors::ErrorLevel;

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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
                let s = [1, 2, 3];
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
                let s: [u8] = "hello";
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    });
}

#[test]
fn bare_if_with_empty_body_as_loop_statement() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() isize {
                loop {
                    if true {}
                }
                12
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn if_as_statement_with_break_body_in_loop_as_expression() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() isize {
                loop {
                    if true {
                        break 12;
                    }
                }
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn block_with_break_as_let_value_in_loop() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() isize {
                loop {
                    let x = {
                        break 1;
                        12
                    };
                }
            }
            "#,
        )
        .succeeds(true);
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
fn simple_assignment() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x = 5;
                x = 10;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn compound_assignment() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x = 5;
                x += 10;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn compound_subtraction() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x = 10;
                x -= 3;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn compound_division() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x = 12;
                x /= 3;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn compound_remainder() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x = 10;
                x %= 3;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn modulo_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                10 % 3
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn if_else_expression() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                if false { 1 } else { 2 }
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn equality_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                1 == 1
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn not_equal_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                1 != 2
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn less_than_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                1 < 2
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn greater_than_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                2 > 1
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn less_or_equal_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                1 <= 1
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn greater_or_equal_operator() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() bool {
                2 >= 2
            }
            "#,
        )
        .succeeds(true);
    });
}

#[ignore = "TODO: Implement `..<` operator"]
#[test]
fn range_exclusive() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                let r = 0..<5;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[ignore = "TODO: Implement `..=` operator"]
#[test]
fn range_inclusive() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() void {
                let r = 0..=5;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn bitwise_operators() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() i32 {
                let a: i32 = 12;
                let b: i32 = 10;
                let and = a & b;
                let or = a | b;
                let xor = a ^ b;
                let shl = a << 2;
                let shr = a >> 2;
                return and;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn bitwise_compound_assignments() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let mut x: i32 = 12;
                x &= 10;
                x |= 3;
                x ^= 5;
                x <<= 1;
                x >>= 1;
                return x;
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn if_as_statement_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                if true {
                    let x = 1;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn if_else_as_statement_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                if true {
                    let x = 1;
                } else {
                    let x = 2;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn while_as_statement_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                while false {
                    let x = 1;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn loop_as_statement_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                loop {
                    break;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn block_expr_as_statement_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                {
                    let x = 1;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn multiple_block_exprs_as_statements_not_in_tail() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                if true {
                    let x = 1;
                }
                while false {
                    let y = 2;
                }
                loop {
                    break;
                }
                42
            }
            "#,
        )
        .succeeds(true);
    });
}

#[ignore = "TODO: Implement `?` operator"]
#[test]
fn postfix_question() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            fn main() Option<i32> {
                let x = Some(5);
                x?
            }
            "#,
        )
        .succeeds(true);
    });
}

#[test]
fn multiple_vars_negate() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() i32 {
                let x = 0;
                let y = x + 1;
                let z = -y;
                z
            }
            "#,
        )
        .succeeds(true);
    })
}

#[test]
fn not_expr() {
    with(|t| {
        t.add_source(
            "main.oxi",
            r#"
            pub fn main() bool {
                let x = true;
                let y = !x;
                y
            }
            "#,
        )
        .succeeds(true);
    })
}
