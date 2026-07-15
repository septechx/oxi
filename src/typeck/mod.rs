mod env;
mod fold;
mod infctx;
mod passes;
mod unify;

mod types;
pub use types::*;

pub(super) use infctx::TyVarId;

use oxic_diag::include_diagnostics;
use thin_vec::ThinVec;

use crate::ast::Mutability;
use crate::context::Ctx;
use crate::hashmap::FxHashMap;
use crate::hir::{self, Crate, DefId, HirId, ModuleId};
use crate::interner::Symbol;
use crate::resolve::ResolverOutputs;

include_diagnostics!("diagnostics.toml");

pub fn typeck_crate(
    ctx: &mut Ctx,
    hir_crate: &mut Crate,
    resolver: &ResolverOutputs,
) -> TypeckOutputs {
    let mut typeck = Typeck::new(ctx, hir_crate, resolver);
    typeck.run();
    typeck.into_outputs()
}

#[derive(Debug, Clone)]
pub enum Adjustment {
    AutoRef(Mutability),
    AutoDeref,
}

struct Typeck<'ctx, 'hir, 'res> {
    ctx: &'ctx mut Ctx,
    krate: &'hir mut Crate,
    resolver: &'res ResolverOutputs,

    /// maps (typed node hir id) -> (ty)
    node_types: FxHashMap<HirId, Ty>,
    /// maps (member access expr hir id) -> (res chosen)
    member_res: FxHashMap<HirId, MemberRes>,
    coherence: CoherenceTable,
    /// maps (struct def id) -> (maps (method name) -> (method def id))
    inherent_methods: FxHashMap<DefId, FxHashMap<Symbol, DefId>>,
    /// maps (struct def id) -> (maps (method name) -> [(interface def id, method def id)])
    interface_methods: FxHashMap<DefId, FxHashMap<Symbol, Vec<(DefId, DefId)>>>,
    /// maps (item def id) -> (scheme)
    item_schemes: FxHashMap<DefId, Scheme>,
    /// maps (def id) -> (module id)
    def_to_module: FxHashMap<DefId, ModuleId>,
    /// maps (expr hir id) -> (adjustments)
    adjustments: FxHashMap<HirId, Vec<Adjustment>>,
    /// maps (generic param hir id) -> (type variable id)
    hir_id_to_ty_var: FxHashMap<HirId, TyVarId>,
}

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    fn new(ctx: &'ctx mut Ctx, krate: &'hir mut Crate, resolver: &'res ResolverOutputs) -> Self {
        let def_to_module = build_def_to_module(resolver);
        Self {
            ctx,
            krate,
            resolver,
            node_types: FxHashMap::default(),
            member_res: FxHashMap::default(),
            coherence: CoherenceTable::default(),
            inherent_methods: FxHashMap::default(),
            interface_methods: FxHashMap::default(),
            item_schemes: FxHashMap::default(),
            def_to_module,
            adjustments: FxHashMap::default(),
            hir_id_to_ty_var: FxHashMap::default(),
        }
    }

    fn run(&mut self) {
        self.collect_signatures();
        self.check_coherence();
        self.build_method_tables();
        self.check_bodies();
        self.rewrite_member_access();
    }

    fn into_outputs(self) -> TypeckOutputs {
        TypeckOutputs {
            node_types: self.node_types,
            member_res: self.member_res,
            coherence: self.coherence,
            inherent_methods: self.inherent_methods,
            interface_methods: self.interface_methods,
            item_schemes: self.item_schemes,
            adjustments: self.adjustments,
            hir_id_to_ty_var: self.hir_id_to_ty_var,
        }
    }
}

fn build_def_to_module(resolver: &ResolverOutputs) -> FxHashMap<DefId, ModuleId> {
    let mut map: FxHashMap<DefId, ModuleId> = FxHashMap::default();
    for (i, module) in resolver.modules.iter().enumerate() {
        for res in module.resolutions.values() {
            map.insert(res.best_binding().def_id, ModuleId(i as u32));
        }
        for methods in module.struct_methods.values() {
            for binding in methods.values() {
                map.insert(binding.def_id, ModuleId(i as u32));
            }
        }
        for &impl_def_id in &module.impls {
            map.insert(impl_def_id, ModuleId(i as u32));
        }
        for &method_def_id in &module.methods {
            map.insert(method_def_id, ModuleId(i as u32));
        }
    }
    map
}

#[derive(Debug)]
pub struct TypeckOutputs {
    /// maps (typed node hir id) -> (ty)
    pub node_types: FxHashMap<HirId, Ty>,
    /// maps (member access expr hir id) -> (res chosen)
    pub member_res: FxHashMap<HirId, MemberRes>,
    pub coherence: CoherenceTable,
    /// maps (struct def id) -> (maps (method name) -> (method def id))
    pub inherent_methods: FxHashMap<DefId, FxHashMap<Symbol, DefId>>,
    /// maps (struct def id) -> (maps (method name) -> [(interface def id, method def id)])
    pub interface_methods: FxHashMap<DefId, FxHashMap<Symbol, Vec<(DefId, DefId)>>>,
    /// maps (item def id) -> (scheme)
    pub item_schemes: FxHashMap<DefId, Scheme>,
    /// maps (expr hir id) -> (adjustments)
    pub adjustments: FxHashMap<HirId, Vec<Adjustment>>,
    /// maps (generic param hir id) -> (type variable id)
    pub hir_id_to_ty_var: FxHashMap<HirId, TyVarId>,
}

impl TypeckOutputs {
    /// Assert there are no errors, if errors were correctly reported this won't be reached
    pub fn assert_no_errors(&self) {
        for ty in self.node_types.values() {
            assert!(!matches!(ty, Ty::Error));
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MemberRes {
    Field { index: usize },
    Method { def_id: DefId, kind: MethodKind },
}

#[derive(Debug, Clone, Copy)]
pub enum MethodKind {
    Inherent,
    Interface { iface: DefId, impl_def: DefId },
}

#[derive(Debug, Default)]
pub struct CoherenceTable {
    /// maps (interface def id, struct def id) -> [impl def id]
    pub impls: FxHashMap<(DefId, DefId), Vec<DefId>>,
    /// maps (interface def id) -> (maps (method name) -> (mdethod def id))
    pub interface_methods: FxHashMap<DefId, FxHashMap<Symbol, DefId>>,
    /// maps (method def id) -> (owning interface def id)
    pub method_to_interface: FxHashMap<DefId, DefId>,
    /// maps (struct def id) -> (maps (field name) -> (HIR type, index))
    pub struct_fields: FxHashMap<DefId, FxHashMap<Symbol, (hir::Ty, usize)>>,
    /// maps (def id) -> (generic param info)
    pub generic_params: FxHashMap<DefId, GenericParamInfo>,
    /// maps (impl def id) -> resolved interface generic args (for duplicate detection)
    pub impl_resolved_generic_args: FxHashMap<DefId, Option<ThinVec<Ty>>>,
}

#[derive(Debug, Default)]
pub struct GenericParamInfo {
    pub hir_ids: Vec<HirId>,
    pub defaults: ThinVec<Option<hir::Ty>>,
}

impl CoherenceTable {
    pub fn has_conflicting_impl(&self, existing: &[DefId], new_args: &Option<ThinVec<Ty>>) -> bool {
        existing.iter().any(|&existing_def_id| {
            self.impl_resolved_generic_args
                .get(&existing_def_id)
                .is_some_and(|existing_args| existing_args == new_args)
        })
    }

    pub(super) fn register_interface(&mut self, interface: DefId, methods: Vec<(Symbol, DefId)>) {
        // or_default() will always be called
        let entry = self.interface_methods.entry(interface).or_default();
        for (name, method) in methods {
            entry.insert(name, method);
            self.method_to_interface.insert(method, interface);
        }
    }
}
