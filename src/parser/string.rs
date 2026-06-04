use crate::context::with_ctx_mut;
use crate::errors::builders;
use crate::hir::ModuleId;
use crate::interner::Symbol;
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
                        let enable_printing = ctx.enable_printing;
                        ctx.errors.add(
                            builders::warning_at(
                                format!("Unknown escape sequence \\{c}"),
                                module_id,
                                error_span,
                                ctx,
                            ),
                            enable_printing,
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
