use oxic::errors::ErrorLevel;

use crate::common::with;

#[test]
fn impl_basic_success() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32;
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
                fn new(v: i32) Self;
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
                fn a(self: &Self) void;
                fn b(self: &Self) void;
            }

            struct S {}

            impl I for S {
                fn a(self: &Self) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn impl_signature_mismatch() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32;
            }

            struct S {}

            impl I for S {
                fn f(self: &Self) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn impl_conflicting_duplicate() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32;
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn impl_wrong_self_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn f(self: &Self) i32;
            }

            impl I for i32 {
                fn f(self: &Self) i32 { return 0; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
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
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn impl_multiple_methods_call_all() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn a(self: &Self) i32;
                fn b(self: &Self) i32;
                fn c(self: &Self) i32;
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
                fn f(self: &Self) i32;
                fn new(v: i32) Self;
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
                fn a(self: &Self) i32;
            }

            interface B {
                fn b(self: &Self) i32;
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
                fn process(self: &Self, a: i32, b: i32) i32;
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
                fn bar(self: &I) void;
            }

            struct S {}

            impl I for S {
                fn bar(self: &I) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn interface_used_as_return_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn make() Self;
            }

            struct S {}

            impl I for S {
                fn make() I { return S {}; }
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn interface_used_as_param_type_in_free_fn() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &Self) void;
            }

            fn bar(x: I) void {}

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn interface_used_as_struct_field_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &Self) void;
            }

            struct S {
                x: I,
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}

#[test]
fn interface_used_in_ptr_param_type() {
    with(|ctx| {
        ctx.add_source(
            "main.oxi",
            r#"
            interface I {
                fn foo(self: &I) void;
            }

            struct S {}

            impl I for S {
                fn foo(self: &I) void {}
            }

            pub fn main() void {}
            "#,
        )
        .succeeds(false)
        .fail_on_level(ErrorLevel::Error);
    })
}
