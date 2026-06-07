use std::collections::hash_map::Entry;

use crate::errors::builders;
use crate::hashmap::FxHashMap;
use crate::hir::{DefId, DefKind, HirId, ItemKind, MaybeOwner, Node};
use crate::interner::Symbol;
use crate::resolve::Res;
use crate::typeck::Typeck;
use crate::typeck::infctx::InferCtx;
use crate::typeck::types::Ty;
use crate::typeck::unify::unify;

// TODO: Get ModuleId from span for diagnostics

impl<'ctx, 'hir, 'res> Typeck<'ctx, 'hir, 'res> {
    pub(super) fn check_coherence(&mut self) {
        let mut icx = InferCtx::default();
        icx.push_level();

        for (i, owner) in self.krate.owners.iter().enumerate() {
            let def_id = DefId(i as u32);
            let MaybeOwner::Owner(info) = owner else {
                continue;
            };
            let Node::Item(item) = &info.nodes.nodes[0].node else {
                continue;
            };
            let ItemKind::Impl {
                self_ty,
                interface_ty,
                items,
            } = &item.kind
            else {
                continue;
            };

            let Some(struct_def_id) = self.resolve_struct(self_ty.res) else {
                self.ctx.errors.add(
                    builders::error(format!(
                        "Expected path to struct in impl, found `{}`",
                        self_ty.display(self.ctx)
                    )),
                    self.ctx.enable_printing,
                );
                continue;
            };
            let Some(interface_def_id) = self.resolve_interface(interface_ty.res) else {
                self.ctx.errors.add(
                    builders::error(format!(
                        "Expected path to interface in impl, found `{}`",
                        interface_ty.display(self.ctx)
                    )),
                    self.ctx.enable_printing,
                );
                continue;
            };

            // 1. Overlap check
            let key = (interface_def_id, struct_def_id);

            if let Entry::Vacant(e) = self.coherence.impls.entry(key) {
                e.insert(def_id);
            } else {
                self.ctx.errors.add(
                    builders::error(format!(
                        "Conflicting implementations of `{}` for `{}`",
                        self.ctx.interner.lookup(
                            self.resolver.defs[interface_def_id.0 as usize]
                                .name
                                .expect("interface has name"),
                        ),
                        self.ctx.interner.lookup(
                            self.resolver.defs[struct_def_id.0 as usize]
                                .name
                                .expect("struct has name"),
                        )
                    )),
                    self.ctx.enable_printing,
                );
            }

            // 2. Signatures check
            let Some(interface_methods) = self.coherence.interface_methods.get(&interface_def_id)
            else {
                continue;
            };

            let mut impl_methods: FxHashMap<Symbol, DefId> = FxHashMap::default();
            for item in items {
                impl_methods.insert(
                    self.resolver.defs[item.0 as usize]
                        .name
                        .expect("item has name"),
                    *item,
                );
            }

            for (name, interface_method) in interface_methods.iter() {
                let Some(impl_method) = impl_methods.get(name) else {
                    self.ctx.errors.add(
                        builders::error(format!(
                            "Missing implementation of interface method `{}`",
                            self.ctx.interner.lookup(*name)
                        )),
                        self.ctx.enable_printing,
                    );
                    continue;
                };
                let iface_sig = self.item_schemes.get(interface_method);
                let impl_sig = self.item_schemes.get(impl_method);
                if let (Some(iface_sig), Some(impl_sig)) = (iface_sig, impl_sig) {
                    let iface_sig_sub =
                        substitute_self(&iface_sig.body, interface_def_id, struct_def_id);

                    if unify(&mut icx, &iface_sig_sub, &impl_sig.body).is_err() {
                        self.ctx.errors.add(
                            builders::error(format!(
                                "Signature mismatch in method `{}`",
                                self.ctx.interner.lookup(*name)
                            )),
                            self.ctx.enable_printing,
                        );
                    }
                }
            }
        }
    }

    fn resolve_struct(&self, res: Res<HirId>) -> Option<DefId> {
        let Res::Def(def_id) = res else {
            return None;
        };
        match self.resolver.defs[def_id.0 as usize].kind {
            DefKind::Struct => Some(def_id),
            _ => None,
        }
    }

    fn resolve_interface(&self, res: Res<HirId>) -> Option<DefId> {
        let Res::Def(def_id) = res else {
            return None;
        };
        match self.resolver.defs[def_id.0 as usize].kind {
            DefKind::Interface => Some(def_id),
            _ => None,
        }
    }
}

fn substitute_self(ty: &Ty, from: DefId, to: DefId) -> Ty {
    match ty {
        Ty::Var(_) | Ty::Prim(_) | Ty::Interface(_) | Ty::Never | Ty::Error => ty.clone(),
        Ty::Adt(d) if *d == from => Ty::Adt(to),
        Ty::Adt(d) => Ty::Adt(*d),
        Ty::Ptr(inner, m) => Ty::Ptr(substitute_self(inner, from, to).into_box(), *m),
        Ty::Slice(inner) => Ty::Slice(substitute_self(inner, from, to).into_box()),
        Ty::Array(inner, n) => Ty::Array(substitute_self(inner, from, to).into_box(), *n),
        Ty::Fn { params, ret } => Ty::Fn {
            params: params
                .iter()
                .map(|ty| substitute_self(ty, from, to))
                .collect(),
            ret: substitute_self(ret, from, to).into_box(),
        },
        Ty::Tuple(elements) => Ty::Tuple(
            elements
                .iter()
                .map(|ty| substitute_self(ty, from, to))
                .collect(),
        ),
    }
}
