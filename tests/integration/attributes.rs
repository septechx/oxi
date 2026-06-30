use crate::common::with;

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
        .succeeds(true);
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
        .succeeds(true);
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
        .succeeds(true);
    })
}
