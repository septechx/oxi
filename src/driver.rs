use std::path::PathBuf;

use anyhow::Result;
use thin_vec::thin_vec;

use crate::ast::Ast;
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

pub fn frontend_stage(
    source_path: &PathBuf,
    source: String,
    check_for_errors: impl Fn() -> Result<()>,
) -> Result<Ast> {
    let (tokens, module_id) = with_ctx_mut(|ctx| tokenize(ctx, source, source_path))?;
    check_for_errors()?;

    let mut ast = with_ctx_mut(|ctx| parse(ctx, tokens, source_path))?;
    check_for_errors()?;

    validate_ast(&ast, module_id);
    check_for_errors()?;

    with_ctx_mut(|ctx| {
        Resolver::assign_node_ids(ctx, &mut ast);
    });

    Ok(ast)
}

pub fn compile_source(
    root_path: PathBuf,
    root_source: String,
    check_for_errors: impl Fn() -> Result<()>,
) -> Result<()> {
    let root_ast = frontend_stage(&root_path, root_source, &check_for_errors)?;

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

    let mut hir_crate = with_ctx_mut(|ctx| {
        let mut lowering_ctx = AstLoweringContext::new(ctx, &asts, &module_tree, &resolver);
        lowering_ctx.lower_crate()
    });
    check_for_errors()?;

    let typeck = with_ctx_mut(|ctx| typeck_crate(ctx, &mut hir_crate, &resolver));
    check_for_errors()?;
    typeck.assert_no_errors();

    let scope_trees = build_scope_trees(&hir_crate);
    let _thir_crate = lower_thir(&hir_crate, &typeck, &scope_trees);

    Ok(())
}
