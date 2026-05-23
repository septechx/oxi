use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use thin_vec::ThinVec;

use crate::{
    ast::{Ast, Item, ItemKind},
    hashmap::FxHashMap,
    span::Span,
    warning_at,
};

#[derive(Debug)]
pub struct ModuleTree {
    pub nodes: Vec<ModuleNode>,
}

#[derive(Debug)]
pub struct ModuleNode {
    pub ast_idx: Option<usize>,
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

    let root_idx = file_paths
        .iter()
        .position(|p| p.file_stem().map(|s| s == "main").unwrap_or(false))
        .ok_or_else(|| anyhow!("main.oxi not found among provided files"))?;

    let mut tree = ModuleTree { nodes: Vec::new() };
    tree.nodes.push(ModuleNode {
        ast_idx: Some(root_idx),
        name: "main".into(),
        qualified_name: "main".into(),
        parent: None,
        children: Vec::new(),
        inline_body: None,
    });

    let mut claimed_files: FxHashMap<usize, usize> = FxHashMap::default();
    claimed_files.insert(root_idx, 0);

    process_ast_items(
        asts,
        file_paths,
        &file_index,
        &mut tree,
        0,
        &mut claimed_files,
        &asts[root_idx].items,
        &file_paths[root_idx],
    )?;

    for (ast_idx, _) in file_paths.iter().enumerate() {
        if !claimed_files.contains_key(&ast_idx) {
            warning_at!(
                Span::new(0, 0),
                crate::hir::ModuleId(ast_idx as u32),
                format!(
                    "Provided file `{}` is not referenced by any `mod` declaration",
                    file_paths[ast_idx].display()
                )
            );
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

        let mod_name = name.value.to_string();
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
                    declaring_path,
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
