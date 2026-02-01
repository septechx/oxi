use thin_vec::ThinVec;

use crate::{
    ast::{Ast, ImportTree, ImportTreeKind, ItemKind, Visibility},
    hir::{DefId, ExportEntry, interner::Symbol, lower::LoweringContext},
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
                    .map(|ident| ident.value.to_string())
                    .collect();
                let path = segments.join("::");
                self.krate.diagnostics.push(format!(
                    "Could not resolve import `{}` in module `{}`",
                    path, self.krate.modules[pi.module_idx].name
                ));
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

                let desired_local_name = match rename_opt {
                    Some(ident) => ident.value.as_ref(),
                    None => segments
                        .last()
                        .expect("segments isn't empty")
                        .value
                        .as_ref(),
                };
                let local_sym = self.krate.interner.intern(desired_local_name);

                if self.krate.modules[mid].imports.contains_key(&local_sym)
                    || self.krate.modules[mid].exports.contains_key(&local_sym)
                {
                    self.krate.diagnostics.push(format!(
                        "Import name collision: `{}` in module `{}`",
                        desired_local_name, self.krate.modules[mid].name
                    ));
                    return ResolutionStatus::Failed;
                }

                if segments.len() == 1 {
                    let name = &segments[0].value;
                    let sym = self.krate.interner.intern(name);
                    let mut found: Option<(usize, ExportEntry)> = None;
                    for (i, m) in self.krate.modules.iter().enumerate() {
                        if let Some(entry) = m.exports.get(&sym) {
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
                    let target_idx_opt = self
                        .krate
                        .modules
                        .iter()
                        .position(|m| m.name.as_str() == module_name.as_ref());
                    if let Some(tmid) = target_idx_opt {
                        let sym = self.krate.interner.intern(symbol_name);
                        let maybe_export = self.krate.modules[tmid]
                            .exports
                            .get(&sym)
                            .map(|export| (export.def, export.visibility));
                        if let Some((def, visibility)) = maybe_export {
                            if tmid != mid && visibility == Visibility::Private {
                                self.krate.diagnostics.push(format!(
                                    "Cannot import `{}` from module `{}` as it is not marked as public",
                                    symbol_name, module_name
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
}
