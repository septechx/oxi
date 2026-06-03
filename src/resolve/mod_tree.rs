use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use thin_vec::ThinVec;

use crate::{
    ast::{Ast, Item, ItemKind},
    context::with_ctx,
    errors::builders,
    hashmap::FxHashMap,
    hir::ModuleId,
    span::Span,
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

pub fn build_module_tree(asts: &[Ast], file_paths: &[PathBuf]) -> Result<ModuleTree> {
    let file_index: FxHashMap<PathBuf, usize> = file_paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.canonicalize().unwrap_or_else(|_| p.to_owned()), i))
        .collect();

    let root_name = file_paths[0]
        .file_stem()
        .expect("file stem")
        .to_string_lossy()
        .to_string();

    let mut tree = ModuleTree { nodes: Vec::new() };
    tree.nodes.push(ModuleNode {
        ast_idx: Some(0),
        name: root_name.clone(),
        qualified_name: root_name,
        parent: None,
        children: Vec::new(),
        inline_body: None,
    });

    let mut claimed_files: FxHashMap<usize, usize> = FxHashMap::default();
    claimed_files.insert(0, 0);

    process_ast_items(
        asts,
        file_paths,
        &file_index,
        &mut tree,
        0,
        &mut claimed_files,
        &asts[0].items,
        &file_paths[0],
    )?;

    for (ast_idx, _) in file_paths.iter().enumerate() {
        if !claimed_files.contains_key(&ast_idx) {
            crate::with_ctx_mut(|ctx| {
                let enable_printing = ctx.enable_printing;
                ctx.errors.add(
                    builders::warning_at(
                        format!(
                            "Provided file `{}` is not referenced by any `mod` declaration",
                            file_paths[ast_idx].display()
                        ),
                        ModuleId(ast_idx as u32),
                        Span::new(0, 0),
                        ctx,
                    ),
                    enable_printing,
                );
            });
        }
    }

    Ok(tree)
}

#[allow(clippy::too_many_arguments)]
fn process_ast_items(
    asts: &[Ast],
    file_paths: &[PathBuf],
    file_index: &FxHashMap<PathBuf, usize>,
    tree: &mut ModuleTree,
    parent_idx: usize,
    claimed_files: &mut FxHashMap<usize, usize>,
    items: &[Item],
    declaring_path: &Path,
) -> Result<()> {
    for item in items {
        let ItemKind::Module { name, body } = &item.kind else {
            continue;
        };

        let mod_name = with_ctx(|ctx| ctx.interner.lookup(name.value).to_string());
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

                let target_idx = file_index
                    .get(&canonicalize_or(&candidate1))
                    .or_else(|| file_index.get(&canonicalize_or(&candidate2)))
                    .copied()
                    .ok_or_else(|| {
                        anyhow!(
                            "`mod {mod_name}` declared in `{}` but no provided file matches \
                             (tried `{}` and `{}`)",
                            declaring_path.display(),
                            candidate1.display(),
                            candidate2.display()
                        )
                    })?;

                if claimed_files.contains_key(&target_idx) {
                    return Err(anyhow!(
                        "File `{}` is referenced by multiple `mod` declarations",
                        file_paths[target_idx].display()
                    ));
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
                process_ast_items(
                    asts,
                    file_paths,
                    file_index,
                    tree,
                    child_idx,
                    claimed_files,
                    &asts[target_idx].items,
                    &file_paths[target_idx],
                )?;

                child_idx
            }
            Some(items) => {
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
                    inline_body: Some(items.clone()),
                });

                let child_idx = tree.nodes.len() - 1;
                process_ast_items(
                    asts,
                    file_paths,
                    file_index,
                    tree,
                    child_idx,
                    claimed_files,
                    items,
                    &inline_declaring_path,
                )?;

                child_idx
            }
        };

        tree.nodes[parent_idx].children.push(node_idx);
    }

    Ok(())
}

fn canonicalize_or(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_owned())
}
