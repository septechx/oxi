use std::ops::{Index, IndexMut};

use thin_vec::{ThinVec, thin_vec};

use crate::ast::{Ast, ImportTree, NodeId, NodeMap, Visibility};
use crate::hashmap::FxHashMap;
use crate::hir::interner::{Interner, Symbol};
use crate::hir::{DefId, ModuleId};

mod early;
mod late;

#[derive(Debug, Clone, Copy)]
pub enum DefKind {
    Function,
    Struct,
    Interface,
    Const,
}

#[derive(Debug, Clone, Copy)]
pub struct Def {
    name: Symbol,
    kind: DefKind,
    visibility: Visibility,
}

#[derive(Debug, Clone, Copy)]
pub enum Res {
    Def(DefId),
    Local(NodeId),
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
    resolutions: FxHashMap<Symbol, NameResolution>,
}

#[derive(Debug)]
pub struct Resolver<'a> {
    asts: &'a ThinVec<Ast>,
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
    pub fn new(asts: &'a ThinVec<Ast>, interner: &'a mut Interner) -> Self {
        let len = asts.len();
        Self {
            asts,
            interner,
            module_idx: 0,
            pending_imports: ThinVec::new(),
            modules: PerModule::new(len),
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

    fn current_module(&self) -> &ModuleData {
        &self.modules[self.module_idx]
    }

    fn current_module_mut(&mut self) -> &mut ModuleData {
        &mut self.modules[self.module_idx]
    }

    pub fn def_id_for_node(&self, node_id: NodeId) -> Option<DefId> {
        self.def_map.get(&node_id).copied()
    }

    pub fn get_def(&self, def_id: DefId) -> &Def {
        &self.defs[def_id.0 as usize]
    }

    #[allow(dead_code)]
    pub fn dump(&self) {
        dbg!(&self.defs);
        dbg!(&self.def_map);
        dbg!(&self.modules);
        dbg!(&self.pending_imports);
        dbg!(&self.interner);
    }
}
