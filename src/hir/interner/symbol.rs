macro_rules! declare_symbols {
    (@step $idx:expr; $name:ident, $($rest:ident),*) => {
        pub const $name: Symbol = $idx;
        declare_symbols!(@step $idx + 1u32; $($rest),*);
    };
    (@step $idx:expr; $name:ident) => {
        pub const $name: Symbol = $idx;
    };

    ($($name:ident),* $(,)?) => {
        pub(crate) const SYM_PREFILL: &[&str] = &[$(stringify!($name)),*];

        #[allow(non_upper_case_globals)]
        pub mod sym {
            use crate::hir::interner::Symbol;
            declare_symbols!(@step 0u32; $($name),*);
        }
    };
}

declare_symbols! {
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
    f16, f32, f64, f128,
    bool, void,
}
