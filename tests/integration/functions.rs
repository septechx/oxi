use crate::common::with;

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
fn extern_fn_declaration() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            pub extern fn foo() void;

            pub fn main() void {}
            "#,
        )
        .succeeds(true);
    })
}
