#[macro_export]
macro_rules! logln {
    ($fmt:expr $(, $($arg:tt)*)?) => {
        if $crate::CTX.with(|ctx| ctx.borrow().enable_printing) {
            println!($fmt $(, $($arg)*)?);
        }
    };
}

#[macro_export]
macro_rules! log {
    ($fmt:expr $(, $($arg:tt)*)?) => {
        if $crate::CTX.with(|ctx| ctx.borrow().enable_printing) {
            print!($fmt $(, $($arg)*)?);
        }
    };
}

#[macro_export]
macro_rules! elogln {
    ($fmt:expr $(, $($arg:tt)*)?) => {
        if $crate::CTX.with(|ctx| ctx.borrow().enable_printing) {
            eprintln!($fmt $(, $($arg)*)?);
        }
    };
}

#[macro_export]
macro_rules! elog {
    ($fmt:expr $(, $($arg:tt)*)?) => {
        if $crate::CTX.with(|ctx| ctx.borrow().enable_printing) {
            eprint!($fmt $(, $($arg)*)?);
        }
    };
}

#[macro_export]
macro_rules! newtype_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub u32);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

#[macro_export]
macro_rules! newtype_ids {
    ($($names:ident),*) => {
        $($crate::newtype_id!($names);)*
    };
}
