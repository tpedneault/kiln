//! Text plumbing shared by the parsers: line-ending normalisation and
//! byte offsets to line and column.

use kiln_model::Span;

/// UTF-8, with `\r\n` turned into `\n` (FORMAT.md, "The project"). `None`
/// for bytes that aren't UTF-8. A lone `\r` is left alone.
pub fn normalize(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    Some(if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text.to_owned()
    })
}

/// Whether a file's own line ending is `\r\n`, judged by its first line
/// ending, so a writer can put it back (FORMAT.md: never changed on
/// write).
pub fn uses_crlf(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .position(|&b| b == b'\n')
        .is_some_and(|i| i > 0 && bytes[i - 1] == b'\r')
}

/// Turns `\n` back into `\r\n` when `crlf`.
pub fn denormalize(text: &str, crlf: bool) -> String {
    if crlf {
        text.replace('\n', "\r\n")
    } else {
        text.to_owned()
    }
}

/// Byte offsets to 1-based line and character column, for one text.
pub struct LineIndex<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex { text, starts }
    }

    /// The span of `start..end`. Offsets past the end, or inside a
    /// character, are clamped rather than trusted.
    pub fn span(&self, start: usize, end: usize) -> Span {
        let start = self.floor(start.min(self.text.len()));
        let end = self.floor(end.clamp(start, self.text.len()));
        let line = self.starts.partition_point(|&s| s <= start);
        let line_start = self.starts[line - 1];
        let column = self.text[line_start..start].chars().count() + 1;
        Span {
            start,
            end,
            line: line as u32,
            column: column as u32,
        }
    }

    fn floor(&self, mut i: usize) -> usize {
        while !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_is_normalised_and_detected() {
        assert_eq!(normalize(b"a\r\nb\r\n").unwrap(), "a\nb\n");
        assert_eq!(normalize(b"\xef\xbb\xbfa").unwrap(), "a");
        assert!(normalize(b"\xff").is_none());
        assert!(uses_crlf(b"a\r\nb\n"));
        assert!(!uses_crlf(b"a\nb\r\n"));
        assert!(!uses_crlf(b"no newline"));
        assert_eq!(denormalize("a\nb\n", true), "a\r\nb\r\n");
    }

    #[test]
    fn spans_have_lines_and_character_columns() {
        let text = "ab\nçd\n";
        let index = LineIndex::new(text);
        let s = index.span(0, 1);
        assert_eq!((s.line, s.column), (1, 1));
        let s = index.span(text.find('d').unwrap(), text.len());
        assert_eq!((s.line, s.column), (2, 2));
        let s = index.span(999, 1000);
        assert_eq!((s.line, s.column, s.start), (3, 1, text.len()));
    }
}
