use std::path::PathBuf;

use anyhow::Result;
use thin_vec::thin_vec;

use crate::ast::validate::validate_ast;
use crate::context::with_ctx_mut;
use crate::hir::AstLoweringContext;
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::resolve::Resolver;
use crate::resolve::build_module_tree;
use crate::thir::lower_thir;
use crate::thir::scope::build_scope_trees;
use crate::typeck::typeck_crate;

#[derive(Debug, Clone, Copy)]
pub enum UnprettyPrintable {
    Tokens,
    Ast,
    Resolver,
    Hir,
    Typeck,
    Thir,
}

// TODO: Query based instead of batch
pub fn compile_source(
    root_path: PathBuf,
    root_source: String,
    check_for_errors: impl Fn() -> Result<()>,
    unpretty: Option<UnprettyPrintable>,
) -> Result<()> {
    let (tokens, module_id) = with_ctx_mut(|ctx| tokenize(ctx, root_source, &root_path))?;
    check_for_errors()?;
    if matches!(unpretty, Some(UnprettyPrintable::Tokens)) {
        println!("{:#?}", tokens);
        return Ok(());
    }

    let mut root_ast = with_ctx_mut(|ctx| parse(ctx, tokens, &root_path))?;
    check_for_errors()?;
    validate_ast(&root_ast, module_id);
    check_for_errors()?;
    with_ctx_mut(|ctx| Resolver::assign_node_ids(ctx, &mut root_ast));
    if matches!(unpretty, Some(UnprettyPrintable::Ast)) {
        println!("{:#?}", root_ast);
        return Ok(());
    }

    let mut asts = thin_vec![root_ast];
    let mut paths = vec![root_path];

    let module_tree = with_ctx_mut(|ctx| build_module_tree(ctx, &mut asts, &mut paths))?;
    check_for_errors()?;

    let resolver = with_ctx_mut(|ctx| {
        let mut resolver = Resolver::new(&asts, &module_tree, ctx);
        resolver.resolve();
        resolver.into_resolver_outputs()
    });
    check_for_errors()?;
    if matches!(unpretty, Some(UnprettyPrintable::Resolver)) {
        println!("{:#?}", resolver);
        return Ok(());
    }

    let mut hir_crate = with_ctx_mut(|ctx| {
        let mut lowering_ctx = AstLoweringContext::new(ctx, &asts, &module_tree, &resolver);
        lowering_ctx.lower_crate()
    });
    check_for_errors()?;
    if matches!(unpretty, Some(UnprettyPrintable::Hir)) {
        println!("{:#?}", hir_crate);
        return Ok(());
    }

    let typeck = with_ctx_mut(|ctx| typeck_crate(ctx, &mut hir_crate, &resolver));
    check_for_errors()?;
    typeck.assert_no_errors();
    if matches!(unpretty, Some(UnprettyPrintable::Typeck)) {
        println!("{:#?}", typeck);
        return Ok(());
    }

    let scope_trees = build_scope_trees(&hir_crate);
    let thir_crate = lower_thir(&hir_crate, &typeck, &scope_trees);
    thir_crate.assert_no_free_vars(&typeck);
    if matches!(unpretty, Some(UnprettyPrintable::Thir)) {
        println!("{:#?}", thir_crate);
        return Ok(());
    }

    Ok(())
}
