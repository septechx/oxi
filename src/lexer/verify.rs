use oxic_diag::include_diagnostics;

use crate::errors::builders;
use crate::lexer::token::{TokenKind, TokenStream};

include_diagnostics!("diagnostics.toml");

pub fn verify_tokens(tokens: &TokenStream) {
    for token in &tokens.0 {
        if let TokenKind::Illegal = &token.kind {
            let c = token.value.chars().next().unwrap_or('\0');
            crate::with_ctx_mut(|ctx| {
                builders::emit_at(
                    ctx,
                    token.span,
                    token.module_id,
                    diag::IllegalToken,
                    crate::diag_params! { token = c },
                );
            });
        }
    }
}
