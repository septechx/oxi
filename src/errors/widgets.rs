use std::fmt::{Debug, Write};
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

use crate::context::Ctx;
use crate::hir::ModuleId;
use crate::span::Span;

pub trait Widget<T: Write>: Debug {
    fn render(&self, f: &mut T) -> std::fmt::Result;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub enum CodeType {
    Add,
    Remove,
    Default,
}

#[derive(Debug, Clone)]
pub struct CodeExampleWidget {
    code: Box<str>,
    line: usize,
    code_type: CodeType,
}

impl CodeExampleWidget {
    pub fn new(code: impl Into<Box<str>>, line: usize, code_type: CodeType) -> Self {
        Self {
            line,
            code_type,
            code: code.into(),
        }
    }

    pub fn code_type(&mut self, code_type: CodeType) -> &mut Self {
        self.code_type = code_type;
        self
    }
}

impl<T: Write> Widget<T> for CodeExampleWidget {
    fn render(&self, f: &mut T) -> std::fmt::Result {
        let pad = (self.line.ilog10() + 1) as usize;

        writeln!(f, "{} {}", " ".repeat(pad), "|".purple())?;
        writeln!(
            f,
            "{} {} {}",
            self.line.to_string().purple(),
            "|".purple(),
            match self.code_type {
                CodeType::Add => self.code.green(),
                CodeType::Remove => self.code.red(),
                CodeType::Default => self.code.normal(),
            }
        )?;
        write!(f, "{} {}", " ".repeat(pad), "|".purple(),)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub enum HighlightType {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct CodeWidget {
    span_start: (usize, usize),
    span_end: (usize, usize),
    span_len: usize,
    source_lines: Vec<(usize, String)>,
    message: Option<Box<str>>,
    highlight_type: HighlightType,
}

impl CodeWidget {
    pub fn new(
        span: Span,
        module_id: ModuleId,
        highlight_type: HighlightType,
        ctx: &Ctx,
    ) -> Result<Self> {
        let sm = ctx
            .source_maps
            .get_source(module_id)
            .ok_or_else(|| anyhow::anyhow!("Source map not found for module id {module_id}"))?;
        let (_, start_line, start_col, span_len) = sm.span_to_source_location(&span);
        let (end_line, end_col) = sm.span_end_location(&span);
        let source_lines = sm
            .get_lines(start_line, end_line)
            .into_iter()
            .map(|(ln, s)| (ln, s.to_string()))
            .collect();

        Ok(Self {
            span_start: (start_line, start_col),
            span_end: (end_line, end_col),
            span_len,
            source_lines,
            message: None,
            highlight_type,
        })
    }

    pub fn from_raw(
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        span_len: usize,
        source_lines: Vec<(usize, String)>,
        highlight_type: HighlightType,
    ) -> Self {
        Self {
            span_start: (start_line, start_col),
            span_end: (end_line, end_col),
            span_len,
            source_lines,
            message: None,
            highlight_type,
        }
    }

    pub fn with_message(mut self, message: impl Into<Box<str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn highlight_type(&mut self, highlight_type: HighlightType) -> &mut Self {
        self.highlight_type = highlight_type;
        self
    }
}

impl<T: Write> Widget<T> for CodeWidget {
    fn render(&self, f: &mut T) -> std::fmt::Result {
        if self.source_lines.is_empty() {
            return Ok(());
        }

        let max_line = self.source_lines.last().map(|(ln, _)| *ln).unwrap_or(0);
        let pad = (max_line.ilog10() + 1) as usize;
        let (start_line, start_col) = self.span_start;
        let (end_line, end_col) = self.span_end;

        let colored_hl = |s: &str| -> colored::ColoredString {
            match self.highlight_type {
                HighlightType::Error => s.red().bold(),
                HighlightType::Warning => s.yellow().bold(),
                HighlightType::Info => s.blue().bold(),
            }
        };

        if start_line == end_line {
            let (_, code) = &self.source_lines[0];
            writeln!(f, "{} {}", " ".repeat(pad), "|".purple())?;
            writeln!(
                f,
                "{} {} {}",
                start_line.to_string().purple(),
                "|".purple(),
                code
            )?;

            let underline = if self.span_len > 1 {
                " ".repeat(start_col - 1) + &"^".repeat(self.span_len)
            } else {
                " ".repeat(start_col - 1) + "^"
            };
            write!(
                f,
                "{} {} {}",
                " ".repeat(pad),
                "|".purple(),
                colored_hl(&underline)
            )?;
        } else {
            let last_idx = self.source_lines.len() - 1;

            writeln!(f, "{} {}", " ".repeat(pad), "|".purple())?;

            for (i, (line_num, code)) in self.source_lines.iter().enumerate() {
                let is_first = i == 0;
                let is_last = i == last_idx;

                if is_first {
                    writeln!(
                        f,
                        "{} {} {}",
                        line_num.to_string().purple(),
                        "|".purple(),
                        code
                    )?;

                    let fill = start_col.saturating_sub(1);
                    let start_ann = format!("{pad}^", pad = "_".repeat(fill));
                    if last_idx == 0 {
                        write!(
                            f,
                            "{} {} {}",
                            " ".repeat(pad),
                            "|".purple(),
                            colored_hl(&start_ann)
                        )?;
                    } else {
                        writeln!(
                            f,
                            "{} {} {}",
                            " ".repeat(pad),
                            "|".purple(),
                            colored_hl(&start_ann)
                        )?;
                    }
                } else if is_last {
                    writeln!(
                        f,
                        "{} {} {} {}",
                        line_num.to_string().purple(),
                        "|".purple(),
                        colored_hl("|"),
                        code
                    )?;

                    let fill = end_col.saturating_sub(1);
                    let end_ann = format!("|{pad}^", pad = "_".repeat(fill));
                    if let Some(msg) = &self.message {
                        write!(
                            f,
                            "{} {} {} {}",
                            " ".repeat(pad),
                            "|".purple(),
                            colored_hl(&end_ann),
                            msg.as_ref().bold()
                        )?;
                    } else {
                        write!(
                            f,
                            "{} {} {}",
                            " ".repeat(pad),
                            "|".purple(),
                            colored_hl(&end_ann)
                        )?;
                    }
                } else {
                    writeln!(
                        f,
                        "{} {} {} {}",
                        line_num.to_string().purple(),
                        "|".purple(),
                        colored_hl("|"),
                        code
                    )?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LocationWidget {
    line: usize,
    column: usize,
    file: PathBuf,
}

impl LocationWidget {
    pub fn new(span: Span, module_id: ModuleId, ctx: &Ctx) -> Result<Self> {
        let (file, line, column, _) = ctx
            .source_maps
            .get_source(module_id)
            .map(|sm| sm.span_to_source_location(&span))
            .ok_or_else(|| anyhow::anyhow!("Source map not found for module id {module_id}"))?;

        Ok(Self { line, column, file })
    }

    pub fn from_raw(line: usize, column: usize, file: PathBuf) -> Self {
        Self { line, column, file }
    }
}

impl<T: Write> Widget<T> for LocationWidget {
    fn render(&self, f: &mut T) -> std::fmt::Result {
        write!(
            f,
            "{}{} {}:{}:{}",
            " ".repeat(self.line.ilog10() as usize + 1),
            "-->".purple(),
            self.file.display(),
            self.line,
            self.column
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InfoWidget {
    line: usize,
    content: Box<str>,
}

impl InfoWidget {
    pub fn new(
        span: Span,
        module_id: ModuleId,
        content: impl Into<Box<str>>,
        ctx: &Ctx,
    ) -> Result<Self> {
        let (_, line, ..) = ctx
            .source_maps
            .get_source(module_id)
            .map(|sm| sm.span_to_source_location(&span))
            .ok_or_else(|| anyhow::anyhow!("Source map not found for module id {module_id}"))?;

        Ok(Self {
            line,
            content: content.into(),
        })
    }

    pub fn from_raw(line: usize, content: Box<str>) -> Self {
        Self { line, content }
    }
}

impl<T: Write> Widget<T> for InfoWidget {
    fn render(&self, f: &mut T) -> std::fmt::Result {
        write!(
            f,
            "{} {} note: {}",
            " ".repeat(self.line.to_string().len()),
            "=".purple(),
            self.content
        )?;

        Ok(())
    }
}
