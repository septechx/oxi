pub mod widgets;

use std::fmt::{self, Display, Formatter};

use colored::Colorize;

use crate::errors::widgets::Widget;
use crate::hashmap::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorLevel {
    Warning,
    Error,
    Fatal,
}

impl Display for ErrorLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorLevel::Warning => write!(f, "{}", "warning".yellow().bold()),
            ErrorLevel::Error => write!(f, "{}", "error".red().bold()),
            ErrorLevel::Fatal => write!(f, "{}", "fatal".red().bold()),
        }
    }
}

#[derive(Debug)]
pub struct CompilationError {
    level: ErrorLevel,
    code: Option<Box<str>>,
    message: Box<str>,
    widgets: Vec<Box<dyn for<'a> Widget<Formatter<'a>>>>,
}

impl CompilationError {
    pub fn new(level: ErrorLevel, code: Option<Box<str>>, message: impl Into<Box<str>>) -> Self {
        Self {
            level,
            code,
            message: message.into(),
            widgets: Vec::new(),
        }
    }

    pub fn add_widget<W>(mut self, widget: W) -> Self
    where
        W: for<'a> Widget<Formatter<'a>> + 'static,
    {
        self.widgets.push(Box::new(widget));
        self
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    fn display_with_context(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(code) = &self.code {
            writeln!(f, "{}[{}]: {}", self.level, code, self.message.bold())?;
        } else {
            writeln!(f, "{}: {}", self.level, self.message.bold())?;
        }

        for widget in &self.widgets {
            widget.render(f)?;
            writeln!(f)?;
        }

        Ok(())
    }
}

impl Display for CompilationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.display_with_context(f)
    }
}

#[derive(Debug)]
pub struct ErrorCollector {
    errors: Vec<CompilationError>,
    max_errors: usize,
    should_panic_on_fatal: bool,
}

impl ErrorCollector {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            max_errors: 100,
            should_panic_on_fatal: true,
        }
    }

    pub fn with_max_errors(mut self, max_errors: usize) -> Self {
        self.max_errors = max_errors;
        self
    }

    pub fn with_panic_on_fatal(mut self, should_panic: bool) -> Self {
        self.should_panic_on_fatal = should_panic;
        self
    }

    pub fn add(&mut self, error: CompilationError, enable_printing: bool) {
        if error.level == ErrorLevel::Fatal && self.should_panic_on_fatal {
            if enable_printing {
                eprintln!("{}", error);
            }
            std::process::exit(1);
        }

        self.errors.push(error);

        if self.errors.len() >= self.max_errors {
            if enable_printing {
                let max_error = builders::fatal1(
                    None,
                    format!(
                        "Too many errors ({}), stopping compilation",
                        self.max_errors
                    ),
                );
                eprintln!("{}", max_error);
            }
            std::process::exit(1);
        }
    }

    pub fn has_errors(&self) -> bool {
        self.errors.iter().any(|e| e.level >= ErrorLevel::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.errors.iter().any(|e| e.level == ErrorLevel::Warning)
    }

    pub fn get_errors(&self, min_level: ErrorLevel) -> Vec<&CompilationError> {
        self.errors
            .iter()
            .filter(|e| e.level >= min_level)
            .collect()
    }

    pub fn get_all_errors(&self) -> &[CompilationError] {
        &self.errors
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }

    pub fn print_all(&self) {
        for error in &self.errors {
            eprint!("{}", error);
        }
    }

    pub fn print_errors(&self, min_level: ErrorLevel) {
        for error in self.get_errors(min_level) {
            eprint!("{}", error);
        }
    }

    pub fn error_counts(&self) -> FxHashMap<ErrorLevel, usize> {
        let mut counts = FxHashMap::default();
        for error in &self.errors {
            *counts.entry(error.level).or_insert(0) += 1;
        }
        counts
    }

    pub fn can_continue(&self) -> bool {
        !self.errors.iter().any(|e| e.level == ErrorLevel::Fatal)
    }

    pub fn has_errors_above_level(&self, min_level: ErrorLevel) -> bool {
        self.errors.iter().any(|e| e.level >= min_level)
    }

    pub fn find_code(&self, code: &str) -> Vec<&CompilationError> {
        self.errors
            .iter()
            .filter(|e| e.code() == Some(code))
            .collect()
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.errors.iter().any(|e| e.code() == Some(code))
    }
}

impl Default for ErrorCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub trait DiagEntry {
    fn code(&self) -> &'static str;
    fn level(&self) -> ErrorLevel;
    fn message(&self) -> &'static str;
}

pub fn format_diag(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut s = template.to_string();
    for (key, val) in args {
        let palceholder = format!("{{{key}}}");
        s = s.replace(&palceholder, &val.to_string());
    }
    s
}

pub mod builders {
    use crate::context::Ctx;
    use crate::hir::ModuleId;
    use crate::span::Span;

    use super::widgets::*;
    use super::*;

    pub fn warning1(code: Option<Box<str>>, message: impl Into<String>) -> CompilationError {
        CompilationError::new(ErrorLevel::Warning, code, message.into())
    }

    pub fn error1(code: Option<Box<str>>, message: impl Into<String>) -> CompilationError {
        CompilationError::new(ErrorLevel::Error, code, message.into())
    }

    pub fn fatal1(code: Option<Box<str>>, message: impl Into<String>) -> CompilationError {
        CompilationError::new(ErrorLevel::Fatal, code, message.into())
    }

    pub fn warning_at1(
        code: Option<Box<str>>,
        message: impl Into<String>,
        module_id: ModuleId,
        span: Span,
        ctx: &Ctx,
    ) -> CompilationError {
        let loc_widget =
            LocationWidget::new_with_ctx(span, module_id, ctx).expect("failed to create error");
        let code_widget = CodeWidget::new_with_ctx(span, module_id, HighlightType::Warning, ctx)
            .expect("failed to create error");
        warning1(code, message.into())
            .add_widget(loc_widget)
            .add_widget(code_widget)
    }

    pub fn error_at1(
        code: Option<Box<str>>,
        message: impl Into<String>,
        module_id: ModuleId,
        span: Span,
        ctx: &Ctx,
    ) -> CompilationError {
        let loc_widget =
            LocationWidget::new_with_ctx(span, module_id, ctx).expect("failed to create error");
        let code_widget = CodeWidget::new_with_ctx(span, module_id, HighlightType::Error, ctx)
            .expect("failed to create error");
        error1(code, message.into())
            .add_widget(loc_widget)
            .add_widget(code_widget)
    }

    pub fn fatal_at1(
        code: Option<Box<str>>,
        message: impl Into<String>,
        module_id: ModuleId,
        span: Span,
        ctx: &Ctx,
    ) -> CompilationError {
        let loc_widget =
            LocationWidget::new_with_ctx(span, module_id, ctx).expect("failed to create error");
        let code_widget = CodeWidget::new_with_ctx(span, module_id, HighlightType::Error, ctx)
            .expect("failed to create error");
        fatal1(code, message.into())
            .add_widget(loc_widget)
            .add_widget(code_widget)
    }

    pub fn emit(ctx: &mut Ctx, entry: impl DiagEntry, params: &[(&str, &dyn Display)]) {
        let error = prepare_diag(&entry, params);
        ctx.errors.add(error, ctx.enable_printing);
    }

    pub fn emit_at(
        ctx: &mut Ctx,
        span: Span,
        module_id: ModuleId,
        entry: impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) {
        let error = prepare_diag_at(ctx, span, module_id, &entry, params);
        ctx.errors.add(error, ctx.enable_printing);
    }

    pub fn emit_with_info(
        ctx: &mut Ctx,
        span: Span,
        module_id: ModuleId,
        info: &str,
        entry: impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) {
        let error = prepare_diag_with_info(ctx, span, module_id, info, &entry, params);
        ctx.errors.add(error, ctx.enable_printing);
    }

    pub fn emit_with_info_raw(
        ctx: &mut Ctx,
        info: &str,
        line: usize,
        entry: impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) {
        let error = prepare_diag_with_info_raw(info, line, &entry, params);
        ctx.errors.add(error, ctx.enable_printing);
    }

    pub fn emit_at_with_info(
        ctx: &mut Ctx,
        span: Span,
        module_id: ModuleId,
        info: &str,
        entry: impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) {
        let error = prepare_diag_at_with_info(ctx, span, module_id, &entry, params, info);
        ctx.errors.add(error, ctx.enable_printing);
    }

    #[macro_export]
    macro_rules! diag_params {
        ($($key:ident = $val:expr),* $(,)?) => {
            &[ $((stringify!($key), &$val as &dyn std::fmt::Display)),* ]
        };
    }

    fn prepare_diag(entry: &impl DiagEntry, params: &[(&str, &dyn Display)]) -> CompilationError {
        let template = entry.message();
        let formatted = format_diag(template, params);

        CompilationError::new(ErrorLevel::Warning, Some(entry.code().into()), formatted)
    }

    fn prepare_diag_at(
        ctx: &Ctx,
        span: Span,
        module_id: ModuleId,
        entry: &impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) -> CompilationError {
        prepare_diag(entry, params)
            .add_widget(
                LocationWidget::new_with_ctx(span, module_id, ctx).expect("failed to create error"),
            )
            .add_widget(
                CodeWidget::new_with_ctx(
                    span,
                    module_id,
                    match entry.level() {
                        ErrorLevel::Warning => HighlightType::Warning,
                        ErrorLevel::Error | ErrorLevel::Fatal => HighlightType::Error,
                    },
                    ctx,
                )
                .expect("failed to create error"),
            )
    }

    fn prepare_diag_with_info(
        ctx: &Ctx,
        span: Span,
        module_id: ModuleId,
        info: &str,
        entry: &impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) -> CompilationError {
        prepare_diag(entry, params).add_widget(
            InfoWidget::new_with_ctx(span, module_id, info, ctx).expect("failed to create error"),
        )
    }

    fn prepare_diag_with_info_raw(
        info: &str,
        line: usize,
        entry: &impl DiagEntry,
        params: &[(&str, &dyn Display)],
    ) -> CompilationError {
        prepare_diag(entry, params).add_widget(InfoWidget::from_raw(line, info.into()))
    }

    fn prepare_diag_at_with_info(
        ctx: &Ctx,
        span: Span,
        module_id: ModuleId,
        entry: &impl DiagEntry,
        params: &[(&str, &dyn Display)],
        info: &str,
    ) -> CompilationError {
        prepare_diag_at(ctx, span, module_id, entry, params).add_widget(
            InfoWidget::new_with_ctx(span, module_id, info, ctx).expect("failed to create error"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_levels() {
        assert!(ErrorLevel::Warning < ErrorLevel::Error);
        assert!(ErrorLevel::Error < ErrorLevel::Fatal);
    }

    #[test]
    fn test_error_collector() {
        let mut collector = ErrorCollector::new();

        collector.add(
            CompilationError::new(ErrorLevel::Warning, None, "Warning message".to_string()),
            true,
        );
        collector.add(
            CompilationError::new(ErrorLevel::Error, None, "Error message".to_string()),
            true,
        );

        assert_eq!(collector.get_all_errors().len(), 2);
        assert!(collector.has_errors());
        assert!(collector.has_warnings());
        assert!(collector.can_continue());
    }
}
