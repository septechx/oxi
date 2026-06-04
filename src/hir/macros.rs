macro_rules! impl_id {
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

macro_rules! impl_ids {
    ($($names:ident),*) => {
        $(impl_id!($names);)*
    };
}
