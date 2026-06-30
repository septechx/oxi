use crate::context::with_ctx_mut;
use crate::diag_params;
use crate::errors::builders;
use crate::hir::ModuleId;
use crate::interner::Symbol;
use crate::parser::diag;
use crate::span::Span;

pub fn process_string(str: &str, span: Span, module_id: ModuleId) -> Symbol {
    let mut builder = String::new();

    let mut escaped = false;
    for (i, c) in str.chars().enumerate() {
        if escaped {
            match c {
                'n' => builder.push('\n'),
                'r' => builder.push('\r'),
                't' => builder.push('\t'),
                '0' => builder.push('\0'),
                '\\' => builder.push('\\'),
                _ => {
                    let error_span =
                        Span::new(span.start() + i as u32, span.start() + (i + 2) as u32);
                    crate::with_ctx_mut(|ctx| {
                        builders::emit_at(
                            ctx,
                            error_span,
                            module_id,
                            diag::UnknownEscapeSequence,
                            diag_params! { c = c },
                        );
                    });
                }
            }

            escaped = false;
            continue;
        }

        if c == '\\' {
            escaped = true;
            continue;
        }

        builder.push(c);
    }

    with_ctx_mut(|ctx| ctx.interner.intern(builder))
}
