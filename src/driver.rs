use std::path::PathBuf;

use anyhow::Result;
use thin_vec::ThinVec;

use crate::ast::validate::validate_ast;
use crate::context::with_ctx_mut;
use crate::diag;
use crate::diag_params;
use crate::errors::builders;
use crate::hir::AstLoweringContext;
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::resolve::Resolver;
use crate::resolve::build_module_tree;
use crate::thir::lower_thir;
use crate::thir::scope::build_scope_trees;
use crate::typeck::typeck_crate;

pub fn compile_sources(
    sources: Vec<(PathBuf, String)>,
    entrypoint: &str,
    check_for_errors: impl Fn() -> Result<()>,
) -> Result<()> {
    let mut asts = ThinVec::with_capacity(sources.len());
    for (file_path, source_text) in &sources {
        let (tokens, module_id) = tokenize(source_text.clone(), file_path)?;
        check_for_errors()?;

        let ast = parse(tokens, file_path)?;
        check_for_errors()?;

        validate_ast(&ast, module_id);
        check_for_errors()?;

        asts.push(ast);
    }

    with_ctx_mut(|ctx| {
        Resolver::assign_node_ids(ctx, &mut asts);
    });

    let paths: Vec<PathBuf> = sources.iter().map(|(p, _)| p.clone()).collect();

    let module_tree = match build_module_tree(&asts, &paths, entrypoint) {
        Ok(tree) => tree,
        Err(e) => {
            with_ctx_mut(|ctx| {
                builders::emit(
                    ctx,
                    diag::FailedToBuildModuleTree,
                    diag_params! { error = e },
                );
            });
            check_for_errors()?;
            anyhow::bail!("failed to build module tree");
        }
    };
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
