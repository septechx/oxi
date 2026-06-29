use oxic::errors::ErrorLevel;

use crate::common::with;

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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    });
}

#[test]
fn let_mut_variable() {
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
