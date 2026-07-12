#![deny(clippy::unwrap_used)]

pub mod ast;
pub mod backend;
pub mod bindings;
pub mod cli;
pub mod context;
pub mod driver;
pub mod errors;
pub mod hashmap;
pub mod hir;
pub mod interner;
pub mod lexer;
pub mod newtype;
pub mod parser;
pub mod resolve;
pub mod span;
pub mod thir;
pub mod typeck;

use std::fs;
use std::io::IsTerminal;

use anyhow::Result;
use clap::Parser;
use oxic_diag::include_diagnostics;

use crate::cli::{Cli, ColorChoice};
use crate::context::{with_ctx, with_ctx_mut};
use crate::driver::compile_source;
use crate::errors::builders;

include_diagnostics!("diagnostics.toml");

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.quiet {
        with_ctx_mut(|ctx| ctx.enable_printing = false);
    }

    build_files(cli)?;

    with_ctx(|ctx| ctx.errors.print_all());

    Ok(())
}

fn check_for_errors() -> Result<()> {
    with_ctx(|ctx| {
        let e = &ctx.errors;
        if e.has_errors() {
            e.print_all();
            std::process::exit(1);
        }
        Ok(())
    })
}

fn build_files(cli: Cli) -> Result<()> {
    let Ok(source_text) = fs::read_to_string(&cli.input) else {
        with_ctx_mut(|ctx| {
            builders::emit(
                ctx,
                diag::SourceFileNotFound,
                diag_params! { file = cli.input.display() },
            );
        });
        std::process::exit(1);
    };

    let use_color = match cli.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err(),
    };
    colored::control::set_override(use_color);

    compile_source(cli.input, source_text, check_for_errors)
}
