use crate::errors::builders;
use crate::lexer::token::{TokenKind, TokenStream};

pub fn verify_tokens(tokens: &TokenStream) {
    for token in &tokens.0 {
        if let TokenKind::Illegal = &token.kind {
            let c = token.value.chars().next().unwrap_or('\0');
            crate::with_ctx_mut(|ctx| {
                let enable_printing = ctx.enable_printing;
                ctx.errors.add(
                    builders::error_at(
                        None,
                        format!("Illegal token: {c}"),
                        token.module_id,
                        token.span,
                        ctx,
                    ),
                    enable_printing,
                );
            });
        }
    }
}
