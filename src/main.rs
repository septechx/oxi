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
pub mod macros;
pub mod parser;
pub mod resolve;
pub mod span;
pub mod thir;
pub mod typeck;
pub mod utils;

use std::cell::RefCell;
use std::fs;
use std::io::IsTerminal;

use anyhow::Result;
use clap::Parser;
use oxic_diag::include_diagnostics;

use crate::cli::{Cli, ColorChoice};
use crate::context::{Ctx, with_ctx_mut};
use crate::driver::compile_sources;
use crate::driver::frontend_stage;
use crate::errors::builders;

include_diagnostics!("diagnostics.toml");

pub static DEFAULT_ROOT: &str = "..";

// TODO: Make this not global
thread_local! {
    pub static CTX: RefCell<Ctx> = RefCell::new(Ctx::new());
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.quiet {
        CTX.with(|ctx| {
            ctx.borrow_mut().enable_printing = false;
        });
    }

    build_files(cli)?;

    CTX.with(|ctx| {
        ctx.borrow().errors.print_all();
    });

    Ok(())
}

fn check_for_errors() -> Result<()> {
    CTX.with(|ctx| {
        let e = &ctx.borrow().errors;
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
            unreachable!()
        })
    };

    let sources = vec![(cli.input.clone(), source_text)];

    if cli.print_ast {
        let use_color = match cli.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
            }
        };
        colored::control::set_override(use_color);

        let asts = frontend_stage(&sources, check_for_errors)?;

        for ast in asts {
            logln!("{}", ast.display(use_color)?);
        }

        return Ok(());
    }

    compile_sources(sources, "main", check_for_errors)
}
