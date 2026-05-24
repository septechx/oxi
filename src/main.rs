#![deny(clippy::unwrap_used)]

pub mod ast;
pub mod backend;
pub mod bindings;
pub mod cli;
pub mod codegen;
pub mod context;
pub mod errors;
pub mod hashmap;
pub mod hir;
pub mod lexer;
pub mod macros;
pub mod parser;
pub mod resolve;
pub mod span;
pub mod utils;

use std::cell::RefCell;
use std::fs;
use std::io::IsTerminal;

use anyhow::Result;
use clap::Parser;
use thin_vec::ThinVec;

use crate::ast::validate::validate_ast;
use crate::cli::Cli;
use crate::context::{Ctx, with_ctx_mut};
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::resolve::{Resolver, build_module_tree};

pub static DEFAULT_ROOT: &str = "..";

// TODO: Make this not global
thread_local! {
    pub static CTX: RefCell<Ctx> = RefCell::new(Ctx::new());
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.quiet {
        CTX.with(|ctx| {
            ctx.borrow_mut().enable_printing = true;
        });
    }

    build_file(cli)?;

    CTX.with(|ctx| {
        ctx.borrow().errors.print_all();
    });

    Ok(())
}

fn check_for_errors() {
    CTX.with(|ctx| {
        let e = &ctx.borrow().errors;
        if e.has_errors() {
            e.print_all();
            std::process::exit(1);
        }
    });
}

fn build_file(cli: Cli) -> Result<()> {
    let mut asts = ThinVec::with_capacity(cli.input.len());
    for file_path in &cli.input {
        let source_text = match fs::read_to_string(file_path) {
            Err(err) => fatal!(format!(
                "Source file `{}` not found: {}",
                file_path.display(),
                err
            )),
            Ok(source_text) => source_text,
        };

        let (tokens, module_id) = tokenize(source_text, file_path)?;
        check_for_errors();

        let ast = parse(tokens, file_path)?;
        check_for_errors();

        if cli.print_ast {
            let use_color = match cli.color {
                cli::ColorChoice::Always => true,
                cli::ColorChoice::Never => false,
                cli::ColorChoice::Auto => {
                    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
                }
            };
            colored::control::set_override(use_color);
            logln!("{}", ast.display(use_color)?);
        }

        validate_ast(&ast, module_id);
        check_for_errors();

        asts.push(ast);
    }

    if cli.print_ast {
        return Ok(());
    }

    let file_paths: Vec<_> = cli.input.clone();
    let module_tree = match build_module_tree(&asts, &file_paths) {
        Ok(tree) => tree,
        Err(e) => fatal!(e.to_string()),
    };
    check_for_errors();

    Resolver::assign_node_ids(&mut asts);
    let resolver_outputs = with_ctx_mut(|ctx| {
        let mut resolver = Resolver::new(&asts, &module_tree, &mut ctx.interner);
        resolver.resolve();
        resolver.into_resolver_outputs()
    });

    dbg!(resolver_outputs);

    println!("Module resolution completed successfully.");

    Ok(())
}
