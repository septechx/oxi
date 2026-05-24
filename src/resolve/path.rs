use crate::ast::Ident;
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
            let name = seg.value.as_ref();
            match name {
                "crate" => current = 0,
                "super" => {
                    current = self.module_tree.nodes[current]
                        .parent
                        .ok_or(PathError::NoParentForSuper { span: seg.span })?;
                }
                "self" => {}
                _ => {
                    current = self.module_tree.nodes[current]
                        .children
                        .iter()
                        .find(|&&child| self.module_tree.nodes[child].name == name)
                        .copied()
                        .ok_or(PathError::ModuleNotFound {
                            name: name.to_string(),
                            span: seg.span,
                        })?;
                }
            }
        }

        Ok(current)
    }
}
