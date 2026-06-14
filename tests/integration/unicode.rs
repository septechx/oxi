use crate::common::with;

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
