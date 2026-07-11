use oxic_diag::include_diagnostics;

use crate::context::Ctx;
use crate::errors::builders;
use crate::lexer::token::{TokenKind, TokenStream};

include_diagnostics!("diagnostics.toml");

pub fn verify_tokens(ctx: &mut Ctx, tokens: &TokenStream) {
    for token in &tokens.0 {
        if let TokenKind::Illegal = &token.kind {
            let c = token.value.chars().next().unwrap_or('\0');
            builders::emit_at(
                ctx,
                token.span,
                token.module_id,
                diag::IllegalToken,
                crate::diag_params! { token = c },
            );
        }
    }
}
