use std::path::PathBuf;

use crate::hir::ModuleId;
use crate::span::Span;
use fxhash::FxHashMap;

#[derive(Debug, Clone)]
pub struct SourceMap {
    content: Box<str>,
    path: PathBuf,
    line_starts: Vec<u32>,
}

impl SourceMap {
    pub fn new(content: String, path: PathBuf) -> Self {
        let mut line_starts = vec![0u32];
        for (i, c) in content.char_indices() {
            if c == '\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            content: content.into_boxed_str(),
            path,
            line_starts,
        }
    }

    pub fn line_column(&self, byte_offset: u32) -> (usize, usize) {
        let offset = byte_offset as usize;
        if offset > self.content.len() {
            return (self.line_starts.len(), 0);
        }
        if offset == self.content.len() {
            let last_line_index = self.line_starts.len();
            let last_line_start = *self.line_starts.last().expect("has line start") as usize;
            let column = 1 + offset - last_line_start;
            return (last_line_index, column);
        }

        let result = self.line_starts.binary_search(&(offset as u32));
        let line_idx = match result {
            Ok(i) => i,
            Err(i) => i - 1,
        };

        let line_start = self.line_starts[line_idx] as usize;
        let byte_col = offset.saturating_sub(line_start);
        let line_content = &self.content[line_start..];
        let char_col = line_content
            .get(..byte_col)
            .map(|s| s.chars().count())
            .unwrap_or(byte_col);
        (line_idx + 1, char_col + 1)
    }

    pub fn get_line(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line - 1] as usize;
        let end = if line < self.line_starts.len() {
            self.line_starts[line] as usize
        } else {
            self.content.len()
        };
        let line_content = self.content.get(start..end)?;
        if let Some(stripped) = line_content.strip_suffix('\n') {
            Some(stripped)
        } else {
            Some(line_content)
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn span_to_source_location(&self, span: &Span) -> (PathBuf, usize, usize, usize) {
        let (line, column) = self.line_column(span.start());
        let char_len = self.byte_range_to_char_count(span.start(), span.len());
        (self.path.clone(), line, column, char_len)
    }

    pub fn span_end_location(&self, span: &Span) -> (usize, usize) {
        self.line_column(span.start() + span.len())
    }

    fn byte_range_to_char_count(&self, byte_offset: u32, byte_len: u32) -> usize {
        if byte_len == 0 {
            return 0;
        }
        let start = byte_offset as usize;
        let end = (start + byte_len as usize).min(self.content.len());
        self.content
            .get(start..end)
            .map(|s| s.chars().count())
            .unwrap_or(byte_len as usize)
    }

    pub fn get_lines(&self, start_line: usize, end_line: usize) -> Vec<(usize, &str)> {
        (start_line..=end_line)
            .filter_map(|line| Some((line, self.get_line(line)?)))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceMapManager {
    source_maps: FxHashMap<ModuleId, SourceMap>,
    next_id: ModuleId,
}

impl SourceMapManager {
    pub fn add_source(&mut self, content: String, path: PathBuf) -> ModuleId {
        let id = self.next_id;
        let source_map = SourceMap::new(content, path);
        self.source_maps.insert(id, source_map);
        self.next_id = ModuleId(self.next_id.0 + 1);
        id
    }

    pub fn next_id(&mut self) -> ModuleId {
        let id = self.next_id;
        self.next_id = ModuleId(self.next_id.0 + 1);
        id
    }

    pub fn get_source(&self, id: ModuleId) -> Option<&SourceMap> {
        self.source_maps.get(&id)
    }

    pub fn get_line_column(&self, id: ModuleId, offset: u32) -> Option<(usize, usize)> {
        self.source_maps.get(&id).map(|sm| sm.line_column(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_column_simple() {
        let content = "hello\nworld\n";
        let sm = SourceMap::new(content.to_string(), PathBuf::from("test.oxi"));

        assert_eq!(sm.line_column(0), (1, 1));
        assert_eq!(sm.line_column(5), (1, 6));
        assert_eq!(sm.line_column(6), (2, 1));
        assert_eq!(sm.line_column(11), (2, 6));
    }

    #[test]
    fn test_line_column_multiline() {
        let content = "first line\nsecond line\nthird line";
        let sm = SourceMap::new(content.to_string(), PathBuf::from("test.oxi"));

        assert_eq!(sm.line_column(0), (1, 1));
        assert_eq!(sm.line_column(11), (2, 1));
        assert_eq!(sm.line_column(23), (3, 1));
    }

    #[test]
    fn test_get_line() {
        let content = "line1\nline2\nline3";
        let sm = SourceMap::new(content.to_string(), PathBuf::from("test.oxi"));

        assert_eq!(sm.get_line(1), Some("line1"));
        assert_eq!(sm.get_line(2), Some("line2"));
        assert_eq!(sm.get_line(3), Some("line3"));
        assert_eq!(sm.get_line(4), None);
    }

    #[test]
    fn test_span_to_source_location() {
        let content = "hello world\n";
        let sm = SourceMap::new(content.to_string(), PathBuf::from("test.oxi"));
        let span = Span::new(0, 5);

        let (path, line, column, length) = sm.span_to_source_location(&span);
        assert_eq!(path.as_os_str(), "test.oxi");
        assert_eq!(line, 1);
        assert_eq!(column, 1);
        assert_eq!(length, 5);
    }

    #[test]
    fn test_span_end_location_at_eof() {
        let content = "first line\nsecond line";
        assert_eq!(content.len(), 22);
        let sm = SourceMap::new(content.to_string(), PathBuf::from("test.oxi"));
        let span = Span::new(0, 22);
        let (line, column) = sm.span_end_location(&span);
        assert_eq!(line, 2, "EOF should be on last line");
        assert_eq!(column, 12, "EOF column should be len of 'second line' + 1");
    }
}
