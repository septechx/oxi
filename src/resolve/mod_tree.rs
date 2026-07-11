use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use thin_vec::ThinVec;

use crate::{
    ast::validate::validate_ast,
    ast::{Ast, Item, ItemKind},
    context::Ctx,
    diag_params,
    errors::builders,
    hashmap::FxHashMap,
    lexer::tokenize,
    parser::parse,
    resolve::{Resolver, diag},
};

#[derive(Debug)]
pub struct ModuleTree {
    pub nodes: Vec<ModuleNode>,
}

#[derive(Debug)]
pub struct ModuleNode {
    pub ast_idx: Option<usize>,
    // TODO: Replace with symbol
    pub name: String,
    pub qualified_name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub inline_body: Option<ThinVec<Item>>,
}

pub fn build_module_tree(
    ctx: &mut Ctx,
    asts: &mut ThinVec<Ast>,
    file_paths: &mut Vec<PathBuf>,
) -> Result<ModuleTree> {
    if file_paths.is_empty() {
        return Ok(ModuleTree { nodes: Vec::new() });
    }

    let mut file_index: FxHashMap<PathBuf, usize> = file_paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.canonicalize().unwrap_or_else(|_| p.to_owned()), i))
        .collect();

    let root_idx = 0;
    let root_name = file_paths[root_idx]
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("root")
        .to_owned();

    let mut tree = ModuleTree { nodes: Vec::new() };
    tree.nodes.push(ModuleNode {
        ast_idx: Some(root_idx),
        name: root_name.clone(),
        qualified_name: root_name,
        parent: None,
        children: Vec::new(),
        inline_body: None,
    });

    let mut claimed_files: FxHashMap<usize, usize> = FxHashMap::default();
    claimed_files.insert(root_idx, 0);

    let root_items = asts[root_idx].items.clone();
    let root_path = file_paths[root_idx].clone();
    process_ast_items(
        ctx,
        asts,
        file_paths,
        &mut file_index,
        &mut tree,
        0,
        &mut claimed_files,
        root_items,
        &root_path,
    );

    Ok(tree)
}

#[allow(clippy::too_many_arguments)]
fn process_ast_items(
    ctx: &mut Ctx,
    asts: &mut ThinVec<Ast>,
    file_paths: &mut Vec<PathBuf>,
    file_index: &mut FxHashMap<PathBuf, usize>,
    tree: &mut ModuleTree,
    parent_idx: usize,
    claimed_files: &mut FxHashMap<usize, usize>,
    items: ThinVec<Item>,
    declaring_path: &Path,
) {
    for item in &items {
        let ItemKind::Module { name, body } = &item.kind else {
            continue;
        };

        let mod_name = ctx.interner.lookup(name.value).to_string();
        let parent_qualified = &tree.nodes[parent_idx].qualified_name;
        let qualified = if parent_qualified.is_empty() {
            mod_name.clone()
        } else {
            format!("{}::{}", parent_qualified, mod_name)
        };

        let node_idx = match body {
            None => {
                let declaring_dir = declaring_path.parent().unwrap_or_else(|| Path::new("."));

                let candidate1 = declaring_dir.join(format!("{}.oxi", mod_name));
                let candidate2 = declaring_dir.join(&mod_name).join("mod.oxi");

                let target_idx = if let Some(&idx) = file_index
                    .get(&canonicalize_or(&candidate1))
                    .or_else(|| file_index.get(&canonicalize_or(&candidate2)))
                {
                    idx
                } else {
                    let found = if candidate1.exists() {
                        &candidate1
                    } else if candidate2.exists() {
                        &candidate2
                    } else {
                        builders::emit(
                            ctx,
                            diag::ModuleFileNotFound,
                            diag_params! {
                                name = mod_name,
                                file = declaring_path.display(),
                                candidate1 = candidate1.display(),
                                candidate2 = candidate2.display(),
                            },
                        );
                        return;
                    };

                    let source = match fs::read_to_string(found) {
                        Ok(s) => s,
                        Err(e) => {
                            builders::emit(
                                ctx,
                                diag::ModuleFileReadError,
                                diag_params! {
                                    file = found.display(),
                                    error = e.to_string(),
                                },
                            );
                            return;
                        }
                    };

                    let Ok((tokens, module_id)) = tokenize(ctx, source, found) else {
                        builders::emit(
                            ctx,
                            diag::ModuleTokenizeError,
                            diag_params! { file = found.display() },
                        );
                        return;
                    };
                    if ctx.errors.has_errors() {
                        return;
                    }

                    let Ok(mut ast) = parse(ctx, tokens, found) else {
                        builders::emit(
                            ctx,
                            diag::ModuleParseError,
                            diag_params! { file = found.display() },
                        );
                        return;
                    };
                    if ctx.errors.has_errors() {
                        return;
                    }

                    validate_ast(&ast, module_id);
                    if ctx.errors.has_errors() {
                        return;
                    }

                    Resolver::assign_node_ids(ctx, &mut ast);

                    let idx = asts.len();
                    let canonical = found.canonicalize().unwrap_or_else(|_| found.to_owned());
                    file_index.insert(canonical, idx);
                    asts.push(ast);
                    file_paths.push(found.to_path_buf());

                    idx
                };

                if claimed_files.contains_key(&target_idx) {
                    builders::emit(
                        ctx,
                        diag::DuplicateModuleDeclaration,
                        diag_params! { file = file_paths[target_idx].display() },
                    );
                    return;
                }
                claimed_files.insert(target_idx, tree.nodes.len());

                tree.nodes.push(ModuleNode {
                    ast_idx: Some(target_idx),
                    name: mod_name,
                    qualified_name: qualified,
                    parent: Some(parent_idx),
                    children: Vec::new(),
                    inline_body: None,
                });

                let child_idx = tree.nodes.len() - 1;
                let cloned_items = asts[target_idx].items.clone();
                let child_path = file_paths[target_idx].clone();
                process_ast_items(
                    ctx,
                    asts,
                    file_paths,
                    file_index,
                    tree,
                    child_idx,
                    claimed_files,
                    cloned_items,
                    &child_path,
                );

                child_idx
            }
            Some(inline_items) => {
                let inline_declaring_path = declaring_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&mod_name)
                    .join("mod.oxi");

                tree.nodes.push(ModuleNode {
                    ast_idx: None,
                    name: mod_name,
                    qualified_name: qualified,
                    parent: Some(parent_idx),
                    children: Vec::new(),
                    inline_body: Some(inline_items.clone()),
                });

                let child_idx = tree.nodes.len() - 1;
                process_ast_items(
                    ctx,
                    asts,
                    file_paths,
                    file_index,
                    tree,
                    child_idx,
                    claimed_files,
                    inline_items.clone(),
                    &inline_declaring_path,
                );

                child_idx
            }
        };

        tree.nodes[parent_idx].children.push(node_idx);
    }
}

fn canonicalize_or(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_owned())
}
