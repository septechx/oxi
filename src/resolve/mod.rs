use std::ops::{Index, IndexMut};

use thin_vec::{ThinVec, thin_vec};

use crate::ast::{Ast, ImportTree, NodeId, NodeMap, Visibility};
use crate::hashmap::FxHashMap;
use crate::hir::interner::{Interner, Symbol};
use crate::hir::{DefId, ModuleId, PrimTy};
use crate::resolve::mod_tree::ModuleTree;

pub use mod_tree::build_module_tree;

mod early;
mod late;
mod mod_tree;

#[derive(Debug, Clone, Copy)]
pub enum DefKind {
    Function,
    Struct,
    Interface,
    Const,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Def {
    name: Symbol,
    kind: DefKind,
    visibility: Visibility,
}

#[derive(Debug, Clone, Copy)]
pub enum Res {
    /// Module-level def
    Def(DefId),
    /// Local definition in function body
    Local(NodeId),
    /// Primitive type, like `i32`
    PrimTy(PrimTy),
    /// Self param in struct or impl
    SelfTyAlias { alias_to: DefId },
    /// Error in name resolution
    Err,
}

#[derive(Debug)]
pub struct PendingImport {
    pub module: ModuleId,
    pub import_item: ImportTree,
    pub visibility: Visibility,
}

#[derive(Debug)]
pub struct PerModule<T: Clone + Default>(ThinVec<T>);

impl<T: Clone + Default> PerModule<T> {
    pub fn new(len: usize) -> Self {
        Self(thin_vec![Default::default(); len])
    }
}

impl<T: Clone + Default> Index<usize> for PerModule<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T: Clone + Default> IndexMut<usize> for PerModule<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

// Non glob imports can shadow glob imports, so this cannot be an enum
#[derive(Debug, Clone, Copy)]
pub struct NameResolution {
    /// Name coming from a local definition or single import. e.g.
    /// ```ignore
    /// import some_module::SomeStruct;
    /// // or
    /// struct MyStruct {}
    /// ````
    non_glob_import: Option<DefId>,
    /// Name coming from a glob import. e.g.
    /// ```ignore
    /// import my_module::*;
    /// ````
    glob_import: Option<DefId>,
}

impl NameResolution {
    pub fn best_binding(&self) -> DefId {
        self.non_glob_import
            .or(self.glob_import)
            .expect("If a resolution exists, it must be a non glob import or a glob import")
    }

    pub fn non_glob_import(res: DefId) -> Self {
        Self {
            non_glob_import: Some(res),
            glob_import: None,
        }
    }

    pub fn glob_import(res: DefId) -> Self {
        Self {
            non_glob_import: None,
            glob_import: Some(res),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModuleData {
    pub resolutions: FxHashMap<Symbol, NameResolution>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub qualified_name: String,
}

#[derive(Debug)]
pub struct Resolver<'a> {
    pub asts: &'a ThinVec<Ast>,
    module_idx: usize,
    interner: &'a mut Interner,

    // Module tree state
    module_tree: Option<ModuleTree>,
    /// Maps ast index -> first tree node index for that ast
    ast_to_module: FxHashMap<usize, usize>,

    // Early res
    pending_imports: ThinVec<PendingImport>,
    modules: PerModule<ModuleData>,
    def_map: NodeMap<DefId>,
    /// Arena\[DefId] -> Def
    pub defs: ThinVec<Def>,

    // Late res
    /// maps (path node id) -> (res)
    pub res_map: NodeMap<Res>,
}

impl<'a> Resolver<'a> {
    pub fn new(asts: &'a ThinVec<Ast>, interner: &'a mut Interner) -> Self {
        Self {
            asts,
            interner,
            module_idx: 0,
            module_tree: None,
            ast_to_module: FxHashMap::default(),
            pending_imports: ThinVec::new(),
            modules: PerModule::new(asts.len()),
            def_map: NodeMap::default(),
            defs: ThinVec::new(),
            res_map: NodeMap::default(),
        }
    }

    pub fn build_module_tree(&mut self, tree: ModuleTree) {
        let node_count = tree.nodes.len();
        self.modules = PerModule::new(node_count);

        for (i, node) in tree.nodes.iter().enumerate() {
            let parent = node.parent;
            let qualified = node.qualified_name.clone();
            self.modules[i].parent = parent;
            self.modules[i].qualified_name = qualified;
        }

        for (i, node) in tree.nodes.iter().enumerate() {
            for &child in &node.children {
                self.modules[i].children.push(child);
            }
            if let Some(ast_idx) = node.ast_idx {
                self.ast_to_module.entry(ast_idx).or_insert(i);
            }
        }

        self.module_tree = Some(tree);
    }

    pub fn resolve(&mut self) {
        self.collect_definitions();
        self.build_graph();
        self.resolve_imports();
        self.late_resolve();
    }

    pub fn current_module(&self) -> &ModuleData {
        &self.modules[self.module_idx]
    }

    pub fn current_module_mut(&mut self) -> &mut ModuleData {
        &mut self.modules[self.module_idx]
    }

    pub fn def_id_for_node(&self, node_id: NodeId) -> Option<DefId> {
        self.def_map.get(&node_id).copied()
    }

    pub fn get_def(&self, def_id: DefId) -> &Def {
        &self.defs[def_id.0 as usize]
    }
}
