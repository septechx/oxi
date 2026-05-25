use std::{
    ffi::OsString,
    path::{self, Component},
};
use thin_vec::ThinVec;

use crate::{
    ast::{Ast, ImportTree, ImportTreeKind, ItemKind, Path, Visibility},
    context::with_ctx,
    hir::{DefId, ExportEntry, HirItemKind, lower::LoweringContext},
    interner::Symbol,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// Successfully resolved and applied to the module
    Resolved,
    /// Failed permanently
    Failed,
    /// Temporary failure (might succeed in later pass)
    Pending,
}

pub struct PendingImport<'a> {
    pub module_idx: usize,
    pub import_item: &'a ImportTree,
    pub visibility: Visibility,
}

impl LoweringContext {
    pub fn resolve_all_imports(&mut self, asts: &[Ast]) {
        // PASS 2: Resolve imports (iteratively until fixpoint)
        let mut pending: ThinVec<PendingImport> = ThinVec::new();
        for (mid, ast) in asts.iter().enumerate() {
            for item in ast.items.iter() {
                if let ItemKind::Import(im) = &item.kind {
                    pending.push(PendingImport {
                        module_idx: mid,
                        import_item: im,
                        visibility: item.visibility,
                    });
                }
            }
        }

        // attempt to resolve until no further progress
        let mut progress = true;
        while progress && !pending.is_empty() {
            progress = false;

            // iterate with index so we can remove resolved entries in-place
            let mut i = 0usize;
            while i < pending.len() {
                let pi = &pending[i];
                // try resolve; if resolved we remove from pending and set progress = true
                match self.try_resolve_import(pi.module_idx, pi.import_item, pi.visibility) {
                    ResolutionStatus::Resolved => {
                        pending.swap_remove(i);
                        progress = true;
                        continue;
                    }
                    ResolutionStatus::Failed => {
                        pending.swap_remove(i);
                        progress = true;
                        continue;
                    }
                    ResolutionStatus::Pending => {
                        // cannot resolve yet: keep item for next pass
                        i += 1;
                        continue;
                    }
                }
            }
        }

        // anything left unresolved -> emit diagnostics
        if !pending.is_empty() {
            for pi in pending {
                let segments: ThinVec<String> = pi
                    .import_item
                    .prefix
                    .segments
                    .iter()
                    .map(|ident| with_ctx(|ctx| ctx.interner.lookup(ident.value).to_string()))
                    .collect();
                let path = segments.join("::");
                self.krate.diagnostics.push(format!(
                    "Could not resolve import `{}` in module `{}`",
                    path, self.krate.modules[pi.module_idx].name
                ));
            }
        }
    }

    fn try_resolve_import(
        &mut self,
        mid: usize,
        im: &ImportTree,
        vis: Visibility,
    ) -> ResolutionStatus {
        match &im.kind {
            ImportTreeKind::Simple(rename_opt) => {
                let segments = &im.prefix.segments;
                if segments.is_empty() {
                    self.krate.diagnostics.push(format!(
                        "Empty import in module {}",
                        self.krate.modules[mid].name
                    ));
                    return ResolutionStatus::Failed;
                }

                let local_sym = match rename_opt {
                    Some(ident) => ident.value,
                    None => segments.last().expect("segments isn't empty").value,
                };

                if self.krate.modules[mid].imports.contains_key(&local_sym)
                    || self.krate.modules[mid].exports.contains_key(&local_sym)
                {
                    self.krate.diagnostics.push(format!(
                        "Import name collision: `{}` in module `{}`",
                        self.krate.interner.lookup(local_sym),
                        self.krate.modules[mid].name
                    ));
                    return ResolutionStatus::Failed;
                }

                if segments.len() == 1 {
                    let name = &segments[0].value;
                    let mut found: Option<(usize, ExportEntry)> = None;
                    for (i, m) in self.krate.modules.iter().enumerate() {
                        if let Some(entry) = m.exports.get(name) {
                            found = Some((i, entry.clone()));
                            break;
                        }
                    }

                    if let Some((def_mod_idx, export_entry)) = found {
                        if def_mod_idx != mid && export_entry.visibility == Visibility::Private {
                            self.krate.diagnostics.push(format!(
                                "Cannot import `{}` as it is not marked as public in module `{}`",
                                name, self.krate.modules[def_mod_idx].name
                            ));
                            return ResolutionStatus::Failed;
                        }

                        self.krate.modules[mid]
                            .imports
                            .insert(local_sym, export_entry.def);

                        if vis == Visibility::Public {
                            if def_mod_idx != mid && export_entry.visibility == Visibility::Private
                            {
                                self.krate.diagnostics.push(format!(
                                    "Cannot re-export `{}` from `{}` because original is private",
                                    name, self.krate.modules[def_mod_idx].name
                                ));
                            } else {
                                self.krate.modules[mid].exports.insert(
                                    local_sym,
                                    ExportEntry {
                                        def: export_entry.def,
                                        visibility: Visibility::Public,
                                    },
                                );
                            }
                        }

                        ResolutionStatus::Resolved
                    } else {
                        ResolutionStatus::Pending
                    }
                } else {
                    let module_name = &segments[0].value;
                    let symbol_name = &segments[segments.len() - 1].value;

                    // find the module index if it exists
                    let target_idx_opt =
                        self.krate.modules.iter().position(|m| {
                            m.name.as_str() == self.krate.interner.lookup(*module_name)
                        });
                    if let Some(tmid) = target_idx_opt {
                        let maybe_export = self.krate.modules[tmid]
                            .exports
                            .get(symbol_name)
                            .map(|export| (export.def, export.visibility));
                        if let Some((def, visibility)) = maybe_export {
                            if tmid != mid && visibility == Visibility::Private {
                                self.krate.diagnostics.push(format!(
                                    "Cannot import `{}` from module `{}` as it is not marked as public",
                                    self.krate.interner.lookup(*symbol_name), self.krate.interner.lookup(*module_name)
                                ));
                                return ResolutionStatus::Failed;
                            }

                            self.krate.modules[mid].imports.insert(local_sym, def);

                            if vis == Visibility::Public {
                                self.krate.modules[mid].exports.insert(
                                    local_sym,
                                    ExportEntry {
                                        def,
                                        visibility: Visibility::Public,
                                    },
                                );
                            }

                            ResolutionStatus::Resolved
                        } else {
                            ResolutionStatus::Pending
                        }
                    } else {
                        self.krate
                            .diagnostics
                            .push(format!("Module {} not found for import", module_name));
                        ResolutionStatus::Failed
                    }
                }
            }
            _ => {
                self.krate
                    .diagnostics
                    .push("Unsupported import tree (only simple imports supported)".to_string());
                ResolutionStatus::Failed
            }
        }
    }

    pub fn lookup_in_current_module(&self, sym: Symbol) -> Option<DefId> {
        let modid = self.current_module?;
        let module = &self.krate.modules[modid.0 as usize];

        // Check local items before imports
        if let Some(export_entry) = module.exports.get(&sym) {
            return Some(export_entry.def);
        }

        if let Some(defid) = module.imports.get(&sym) {
            return Some(*defid);
        }

        None
    }

    pub fn resolve_path(&mut self, path: &Path) -> Option<DefId> {
        if path.segments.is_empty() {
            return None;
        }

        if path.is_single() {
            let sym = path.segments[0].value;
            return self.lookup_in_current_module(sym);
        }

        let seg_count = path.segments.len();

        // First, check if the first segment is a struct in current module
        // and try to resolve methods from remaining segments
        if let Some(curr) = self.current_module {
            let first_sym = path.segments[0].value;
            if let Some(export_entry) = self.krate.modules[curr.0 as usize].exports.get(&first_sym)
            {
                let def = export_entry.def;
                if let Some(item) = self.krate.items.get(def.0 as usize)
                    && matches!(item.kind, HirItemKind::Struct(_))
                    && seg_count >= 2
                {
                    let method_name = self.krate.interner.lookup(path.segments[1].value);
                    let method_name = &method_name.to_string();
                    if let Some(method_def) =
                        self.try_resolve_struct_method(curr.0 as usize, def, method_name)
                    {
                        return Some(method_def);
                    }
                }
            }
        }

        // Try longest possible module prefixes.
        // For path segments s0, s1, ..., s(n-1) (n >= 2) attempt prefixes:
        //  p = n-1: module_name = join(s0..s(p-1))  (length p), symbol = s[p]
        //  p = n-2, ..., 1
        for p in (1..seg_count).rev() {
            let module_name = path.segments[..p]
                .iter()
                .map(|id| self.krate.interner.lookup(id.value))
                .collect::<Vec<_>>()
                .join("::");

            if let Some(target_idx) = self
                .krate
                .modules
                .iter()
                .position(|m| m.name.as_str() == module_name)
            {
                let sym = path.segments[p].value;

                if let Some(export_entry) = self.krate.modules[target_idx].exports.get(&sym) {
                    let curr = self.current_module?;
                    if target_idx != curr.0 as usize
                        && export_entry.visibility == Visibility::Private
                    {
                        return None;
                    }

                    let def = export_entry.def;

                    // Check if this is a struct and try to resolve methods from remaining segments
                    if let Some(item) = self.krate.items.get(def.0 as usize)
                        && matches!(item.kind, HirItemKind::Struct(_))
                        && p + 1 < seg_count
                    {
                        let method_name = self.krate.interner.lookup(path.segments[p + 1].value);
                        let method_name = &method_name.to_string();
                        if let Some(method_def) =
                            self.try_resolve_struct_method(target_idx, def, method_name)
                        {
                            return Some(method_def);
                        }
                        return None;
                    }

                    return Some(def);
                }
            }
        }

        None
    }

    fn try_resolve_struct_method(
        &mut self,
        module_idx: usize,
        struct_def: DefId,
        method_name: &str,
    ) -> Option<DefId> {
        let item = self.krate.item(struct_def);
        if !matches!(item.kind, HirItemKind::Struct(_)) {
            return None;
        }

        let module = &self.krate.modules[module_idx];
        let method_sym = self.krate.interner.intern(method_name);

        let method_meta = module.struct_methods.get(&struct_def)?.get(&method_sym)?;

        let curr = self.current_module?.0 as usize;
        if module_idx != curr && method_meta.visibility == Visibility::Private {
            return None;
        }

        Some(method_meta.def)
    }
}

/// Convert a path like `a/b.oxi` into `a::b`.
pub fn path_to_mod<P: AsRef<path::Path>>(p: P) -> String {
    let path = p.as_ref();

    let mut normals: Vec<OsString> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(os.to_os_string()),
            _ => None,
        })
        .collect();

    if normals.is_empty() {
        return String::new();
    }

    let last = normals.pop().expect("normals isn't empty");
    let last_stem = path::Path::new(&last)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut parts: Vec<String> = normals
        .into_iter()
        .map(|os| os.to_string_lossy().into_owned())
        .collect();

    if !last_stem.is_empty() {
        parts.push(last_stem);
    }

    parts.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Ident;
    use crate::hashmap::FxHashMap;
    use crate::hir::{HirItem, MethodMeta, ModuleId, ModuleInfo, TypeId};
    use crate::span::Span;

    #[test]
    fn simple() {
        assert_eq!(path_to_mod("a/b.oxi"), "a::b");
        assert_eq!(path_to_mod("a/b.test.oxi"), "a::b.test");
        assert_eq!(path_to_mod("single.oxi"), "single");
        assert_eq!(path_to_mod("single"), "single");
        assert_eq!(path_to_mod("/usr/local/pkg.oxi"), "usr::local::pkg");
    }

    struct TestCtx {
        ctx: LoweringContext,
    }

    impl TestCtx {
        fn new() -> Self {
            TestCtx {
                ctx: LoweringContext::new(),
            }
        }

        fn intern(&mut self, name: &str) -> Symbol {
            self.ctx.krate.interner.intern(name)
        }

        fn add_module(&mut self, name: &str) -> usize {
            let idx = self.ctx.krate.modules.len();
            self.ctx.krate.modules.push(ModuleInfo {
                name: name.to_string(),
                exports: FxHashMap::default(),
                items: ThinVec::new(),
                imports: FxHashMap::default(),
                struct_methods: FxHashMap::default(),
                struct_fields: FxHashMap::default(),
                struct_impls: FxHashMap::default(),
                interface_impls: FxHashMap::default(),
            });
            idx
        }

        fn add_fn_to_module(
            &mut self,
            mod_idx: usize,
            name: &str,
            visibility: Visibility,
        ) -> DefId {
            let def_id = DefId(self.ctx.krate.items.len() as u32);
            let sym = self.intern(name);

            self.ctx.krate.items.push(HirItem {
                defid: def_id,
                kind: HirItemKind::Function(crate::hir::Function {
                    name: sym,
                    params: ThinVec::new(),
                    ret: TypeId(0),
                    body: None,
                    module: ModuleId(mod_idx as u32),
                    associated: None,
                }),
                span: Span::new(0, 0),
            });

            self.ctx.krate.modules[mod_idx].exports.insert(
                sym,
                ExportEntry {
                    def: def_id,
                    visibility,
                },
            );

            def_id
        }

        fn add_struct_to_module(
            &mut self,
            mod_idx: usize,
            name: &str,
            visibility: Visibility,
        ) -> DefId {
            let def_id = DefId(self.ctx.krate.items.len() as u32);
            let sym = self.intern(name);

            self.ctx.krate.items.push(HirItem {
                defid: def_id,
                kind: HirItemKind::Struct(crate::hir::Struct {
                    name: sym,
                    fields: ThinVec::new(),
                    module: ModuleId(mod_idx as u32),
                }),
                span: Span::new(0, 0),
            });

            self.ctx.krate.modules[mod_idx].exports.insert(
                sym,
                ExportEntry {
                    def: def_id,
                    visibility,
                },
            );

            def_id
        }

        fn add_method_to_struct(
            &mut self,
            mod_idx: usize,
            struct_def: DefId,
            method_name: &str,
            visibility: Visibility,
        ) -> DefId {
            let method_def = DefId(self.ctx.krate.items.len() as u32);
            let method_sym = self.intern(method_name);

            self.ctx.krate.items.push(HirItem {
                defid: method_def,
                kind: HirItemKind::Function(crate::hir::Function {
                    name: method_sym,
                    params: ThinVec::new(),
                    ret: TypeId(0),
                    body: None,
                    module: ModuleId(mod_idx as u32),
                    associated: Some(struct_def),
                }),
                span: Span::new(0, 0),
            });

            self.ctx.krate.modules[mod_idx]
                .struct_methods
                .entry(struct_def)
                .or_default()
                .insert(
                    method_sym,
                    MethodMeta {
                        def: method_def,
                        visibility,
                    },
                );

            method_def
        }

        fn set_current_module(&mut self, idx: usize) {
            self.ctx.current_module = Some(ModuleId(idx as u32));
        }

        fn path(&mut self, segments: &[&str]) -> Path {
            Path {
                span: Span::new(0, 0),
                segments: segments
                    .iter()
                    .map(|s| Ident {
                        value: self.ctx.krate.interner.intern(s),
                        span: Span::new(0, 0),
                    })
                    .collect(),
            }
        }

        fn resolve(&mut self, segments: &[&str]) -> Option<DefId> {
            let path = self.path(segments);
            self.ctx.resolve_path(&path)
        }
    }

    #[test]
    fn resolve_single_segment_local_fn() {
        let mut t = TestCtx::new();
        let mod_idx = t.add_module("foo");
        let fn_def = t.add_fn_to_module(mod_idx, "bar", Visibility::Public);
        t.set_current_module(mod_idx);

        let result = t.resolve(&["bar"]);
        assert_eq!(result, Some(fn_def));
    }

    #[test]
    fn resolve_two_segment_module_fn() {
        let mut t = TestCtx::new();
        let mod_a = t.add_module("mod_a");
        let mod_b = t.add_module("mod_b");
        let fn_def = t.add_fn_to_module(mod_a, "func", Visibility::Public);
        t.set_current_module(mod_b);

        let result = t.resolve(&["mod_a", "func"]);
        assert_eq!(result, Some(fn_def));
    }

    #[test]
    fn resolve_three_segment_nested_fn() {
        let mut t = TestCtx::new();
        let mod_a = t.add_module("a");
        let _mod_b = t.add_module("a::b");
        let mod_c = t.add_module("a::b::c");
        let fn_def = t.add_fn_to_module(mod_c, "deep_fn", Visibility::Public);
        t.set_current_module(mod_a);

        let result = t.resolve(&["a", "b", "c", "deep_fn"]);
        assert_eq!(result, Some(fn_def));
    }

    #[test]
    fn resolve_struct_not_method() {
        let mut t = TestCtx::new();
        let mod_idx = t.add_module("foo");
        let struct_def = t.add_struct_to_module(mod_idx, "MyStruct", Visibility::Public);
        t.set_current_module(mod_idx);

        let result = t.resolve(&["MyStruct"]);
        assert_eq!(result, Some(struct_def));
    }

    #[test]
    fn resolve_struct_method_same_module() {
        let mut t = TestCtx::new();
        let mod_idx = t.add_module("foo");
        let struct_def = t.add_struct_to_module(mod_idx, "MyStruct", Visibility::Public);
        let method_def =
            t.add_method_to_struct(mod_idx, struct_def, "my_method", Visibility::Public);
        t.set_current_module(mod_idx);

        let result = t.resolve(&["MyStruct", "my_method"]);
        assert_eq!(result, Some(method_def));
    }

    #[test]
    fn resolve_struct_method_other_module() {
        let mut t = TestCtx::new();
        let mod_a = t.add_module("mod_a");
        let mod_b = t.add_module("mod_b");
        let struct_def = t.add_struct_to_module(mod_a, "MyStruct", Visibility::Public);
        let method_def = t.add_method_to_struct(mod_a, struct_def, "my_method", Visibility::Public);
        t.set_current_module(mod_b);

        let result = t.resolve(&["mod_a", "MyStruct", "my_method"]);
        assert_eq!(result, Some(method_def));
    }

    #[test]
    fn resolve_fails_private_from_other_module() {
        let mut t = TestCtx::new();
        let mod_a = t.add_module("mod_a");
        let mod_b = t.add_module("mod_b");
        t.add_fn_to_module(mod_a, "private_func", Visibility::Private);
        t.set_current_module(mod_b);

        let result = t.resolve(&["mod_a", "private_func"]);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_fails_nonexistent() {
        let mut t = TestCtx::new();
        let mod_idx = t.add_module("foo");
        t.set_current_module(mod_idx);

        let result = t.resolve(&["nonexistent"]);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_fails_nonexistent_in_module() {
        let mut t = TestCtx::new();
        let _mod_a = t.add_module("mod_a");
        let mod_b = t.add_module("mod_b");
        t.set_current_module(mod_b);

        let result = t.resolve(&["mod_a", "nonexistent"]);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_fails_private_method_from_other_module() {
        let mut t = TestCtx::new();
        let mod_a = t.add_module("mod_a");
        let mod_b = t.add_module("mod_b");
        let struct_def = t.add_struct_to_module(mod_a, "MyStruct", Visibility::Public);
        t.add_method_to_struct(mod_a, struct_def, "private_method", Visibility::Private);
        t.set_current_module(mod_b);

        let result = t.resolve(&["mod_a", "MyStruct", "private_method"]);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_fails_empty_path() {
        let mut t = TestCtx::new();
        let mod_idx = t.add_module("foo");
        t.set_current_module(mod_idx);

        let result = t.resolve(&[]);
        assert_eq!(result, None);
    }
}
