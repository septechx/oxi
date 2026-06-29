/// Creates a FxHashMap from the given field names.
/// ```rust,ignore
/// struct_fields!(a, b, c)
/// ```
#[macro_export]
macro_rules! struct_fields {
    ( $( $name:ident ),* $(,)? ) => {{
        let names = [ $( stringify!($name) ),* ];
        names.iter()
             .enumerate()
             .map(|(i, &s)| (s.into(), i as u32))
             .collect::<$crate::hashmap::FxHashMap<Box<str>, u32>>()
    }};
}

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
macro_rules! emit_at {
    ($builder:expr, $span:expr, $module_id:expr, $msg:expr, $info:expr, $highlight:expr) => {
        (|| -> anyhow::Result<()> {
            let span = $span;
            let module_id = $module_id;
            let msg = $msg;
            let loc_widget = $crate::errors::widgets::LocationWidget::new(span, module_id)?;
            let code_widget =
                $crate::errors::widgets::CodeWidget::new(span, module_id, $highlight)?;
            let builder = $builder(None, msg)
                .add_widget(loc_widget)
                .add_widget(code_widget);
            let builder = if let Some(info) = $info {
                let info_widget = $crate::errors::widgets::InfoWidget::new(span, module_id, info)?;
                builder.add_widget(info_widget)
            } else {
                builder
            };

            $crate::CTX.with_borrow_mut(|ctx| {
                let enable_printing = ctx.enable_printing;
                ctx.errors.add(builder, enable_printing);
            });

            Ok(())
        })()
    };
}

#[macro_export]
macro_rules! error_at {
    ($span:expr, $module_id:expr, $msg:expr $(,)?) => {
        $crate::emit_at!(
            $crate::errors::builders::error1,
            $span,
            $module_id,
            $msg,
            None::<Box<str>>,
            $crate::errors::widgets::HighlightType::Error
        )
        .expect("failed to emit error")
    };
    ($token:expr, $msg:expr $(,)?) => {{
        let token = $token;
        $crate::error_at!(token.span, token.module_id, $msg)
    }};
}

#[macro_export]
macro_rules! warning_at {
    ($span:expr, $module_id:expr, $msg:expr $(,)?) => {
        $crate::emit_at!(
            $crate::errors::builders::warning1,
            $span,
            $module_id,
            $msg,
            None::<Box<str>>,
            $crate::errors::widgets::HighlightType::Warning
        )
        .expect("failed to emit warning")
    };
    ($token:expr, $msg:expr $(,)?) => {{
        let token = $token;
        $crate::warning_at!(token.span, token.module_id, $msg)
    }};
}

#[macro_export]
macro_rules! fatal_at {
    ($span:expr, $module_id:expr, $msg:expr $(,)?) => {{
        $crate::emit_at!(
            $crate::errors::builders::fatal1,
            $span,
            $module_id,
            $msg,
            None::<Box<str>>,
            $crate::errors::widgets::HighlightType::Error
        )
        .expect("failed to create error");
        unreachable!()
    }};
    ($token:expr, $msg:expr $(,)?) => {{
        let token = $token;
        $crate::fatal_at!(token.span, token.module_id, $msg)
    }};
}

#[macro_export]
macro_rules! error_at_with_info {
    ($span:expr, $module_id:expr, $msg:expr, $info:expr $(,)?) => {
        $crate::emit_at!(
            $crate::errors::builders::error1,
            $span,
            $module_id,
            $msg,
            Some($info),
            $crate::errors::widgets::HighlightType::Error
        )
    };
    ($token:expr, $msg:expr, $info:expr $(,)?) => {{
        let token = $token;
        $crate::error_at_with_info!(token.span, token.module_id, $msg, $info)
    }};
}

#[macro_export]
macro_rules! warning_at_with_info {
    ($span:expr, $module_id:expr, $msg:expr, $info:expr $(,)?) => {
        $crate::emit_at!(
            $crate::errors::builders::warning1,
            $span,
            $module_id,
            $msg,
            Some($info),
            $crate::errors::widgets::HighlightType::Warning
        )
    };
    ($token:expr, $msg:expr, $info:expr $(,)?) => {{
        let token = $token;
        $crate::warning_at_with_info!(token.span, token.module_id, $msg, $info)
    }};
}

#[macro_export]
macro_rules! fatal_at_with_info {
    ($span:expr, $module_id:expr, $msg:expr, $info:expr $(,)?) => {{
        $crate::emit_at!(
            $crate::errors::builders::fatal1,
            $span,
            $module_id,
            $msg,
            Some($info),
            $crate::errors::widgets::HighlightType::Error
        )
        .expect("failed to create error");
        unreachable!()
    }};
    ($token:expr, $msg:expr, $info:expr $(,)?) => {{
        let token = $token;
        $crate::fatal_at_with_info!(token.span, token.module_id, $msg, $info)
    }};
}

#[macro_export]
macro_rules! error {
    ($msg:expr $(,)?) => {
        $crate::CTX.with_borrow_mut(|ctx| {
            let enable_printing = ctx.enable_printing;
            ctx.errors.add(
                $crate::errors::builders::error1(None, $msg),
                enable_printing,
            );
        })
    };
}

#[macro_export]
macro_rules! warning {
    ($msg:expr $(,)?) => {
        $crate::CTX.with_borrow_mut(|ctx| {
            let enable_printing = ctx.enable_printing;
            ctx.errors
                .add($crate::errors::builders::warning1($msg), enable_printing);
        })
    };
}

#[macro_export]
macro_rules! fatal {
    ($msg:expr $(,)?) => {{
        $crate::CTX.with_borrow_mut(|ctx| {
            let enable_printing = ctx.enable_printing;
            ctx.errors.add(
                $crate::errors::builders::fatal1(None, $msg),
                enable_printing,
            );
        });
        unreachable!()
    }};
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
