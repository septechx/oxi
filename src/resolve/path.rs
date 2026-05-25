use crate::ast::Ident;
use crate::interner::sym;
use crate::resolve::Resolver;
use crate::span::Span;

#[derive(Debug, Clone)]
pub(super) enum PathError {
    NoParentForSuper { span: Span },
    ModuleNotFound { name: String, span: Span },
}

impl<'a, 'ctx> Resolver<'a, 'ctx> {
    pub(super) fn resolve_module_path(
        &self,
        from_node: usize,
        segments: &[Ident],
    ) -> Result<usize, PathError> {
        let mut current = from_node;

        for seg in segments.iter() {
            match seg.value {
                sym::crate_ => current = 0,
                sym::super_ => {
                    current = self.module_tree.nodes[current]
                        .parent
                        .ok_or(PathError::NoParentForSuper { span: seg.span })?;
                }
                sym::self_ => {}
                _ => {
                    current = self.module_tree.nodes[current]
                        .children
                        .iter()
                        .find(|&&child| {
                            self.module_tree.nodes[child].name
                                == self.ctx.interner.lookup(seg.value)
                        })
                        .copied()
                        .ok_or(PathError::ModuleNotFound {
                            name: self.ctx.interner.lookup(seg.value).to_string(),
                            span: seg.span,
                        })?;
                }
            }
        }

        Ok(current)
    }
}
