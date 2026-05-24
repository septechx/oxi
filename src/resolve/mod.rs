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
pub struct ResolverOutputs {
    /// maps (path node id) -> (res)
    pub res_map: NodeMap<Res>,
    /// maps (def node id) -> (def id)
    pub def_map: NodeMap<DefId>,
    /// Arena\[DefId] -> Def
    pub defs: ThinVec<Def>,
    pub modules: PerModule<ModuleData>,
}

#[derive(Debug)]
pub struct Resolver<'a> {
    asts: &'a ThinVec<Ast>,
    module_tree: &'a ModuleTree,
    module_idx: usize,
    interner: &'a mut Interner,

    // Early res
    pending_imports: ThinVec<PendingImport>,
    modules: PerModule<ModuleData>,
    def_map: NodeMap<DefId>,
    /// Arena\[DefId] -> Def
    defs: ThinVec<Def>,

    // Late res
    /// maps (path node id) -> (res)
    res_map: NodeMap<Res>,
}

impl<'a> Resolver<'a> {
    pub fn new(
        asts: &'a ThinVec<Ast>,
        module_tree: &'a ModuleTree,
        interner: &'a mut Interner,
    ) -> Self {
        let node_count = module_tree.nodes.len();
        let mut modules: PerModule<ModuleData> = PerModule::new(node_count);

        for (i, node) in module_tree.nodes.iter().enumerate() {
            modules[i].parent = node.parent;
            modules[i].qualified_name = node.qualified_name.clone();
            for &child in &node.children {
                modules[i].children.push(child);
            }
        }

        Self {
            asts,
            interner,
            module_idx: 0,
            module_tree,
            pending_imports: ThinVec::new(),
            modules,
            def_map: NodeMap::default(),
            defs: ThinVec::new(),
            res_map: NodeMap::default(),
        }
    }

    pub fn resolve(&mut self) {
        self.collect_definitions();
        self.build_graph();
        self.resolve_imports();
        self.late_resolve();
    }

    pub fn into_resolver_outputs(self) -> ResolverOutputs {
        ResolverOutputs {
            res_map: self.res_map,
            def_map: self.def_map,
            defs: self.defs,
            modules: self.modules,
        }
    }

    fn current_module(&self) -> &ModuleData {
        &self.modules[self.module_idx]
    }

    fn current_module_mut(&mut self) -> &mut ModuleData {
        &mut self.modules[self.module_idx]
    }

    fn def_id_for_node(&self, node_id: NodeId) -> Option<DefId> {
        self.def_map.get(&node_id).copied()
    }
}
