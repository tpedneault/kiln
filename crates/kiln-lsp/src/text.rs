//! Open documents and LSP positions. Kiln parses normalised text (no
//! byte-order mark, `\n` line ends); editors count in UTF-16 code units
//! per line of the text they hold. Line numbers agree between the two,
//! and so do columns, because a `\r` only ever ends a line; the one
//! difference is a byte-order mark, which the editor may count on the
//! first line. Every conversion goes through here.

use lsp_types::{Position, Range};

/// A document as the server holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    /// Normalised.
    pub text: String,
    /// The editor's copy starts with a byte-order mark.
    pub bom: bool,
    /// The editor's copy ends lines with `\r\n` (so inserted text should).
    pub crlf: bool,
}

impl Doc {
    pub fn new(raw: &str) -> Doc {
        let bom = raw.starts_with('\u{feff}');
        let crlf = kiln_parse::uses_crlf(raw.as_bytes());
        let text = kiln_parse::normalize(raw.as_bytes()).unwrap_or_default();
        Doc { text, bom, crlf }
    }

    /// The byte offset of an editor position, clamped to its line.
    pub fn offset(&self, pos: Position) -> usize {
        let character = if pos.line == 0 && self.bom {
            pos.character.saturating_sub(1)
        } else {
            pos.character
        };
        offset(&self.text, pos.line, character)
    }

    /// The editor position of a byte offset.
    pub fn position(&self, offset: usize) -> Position {
        let mut p = position(&self.text, offset);
        if p.line == 0 && self.bom {
            p.character += 1;
        }
        p
    }

    pub fn range(&self, start: usize, end: usize) -> Range {
        Range::new(self.position(start), self.position(end))
    }

    /// Text to insert, with the editor's line ends.
    pub fn outgoing(&self, text: &str) -> String {
        kiln_parse::denormalize(text, self.crlf)
    }

    /// The range a finding at `line`:`column` (1-based, in characters)
    /// underlines: the quoted string or the word there, or one character.
    pub fn finding_range(&self, line: u32, column: u32) -> Range {
        let start = line_start(&self.text, line.saturating_sub(1));
        let line_text = line_text(&self.text, start);
        let from = line_text
            .char_indices()
            .nth(column.saturating_sub(1) as usize)
            .map(|(i, _)| i)
            .unwrap_or(line_text.len());
        let rest = &line_text[from..];
        if let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) {
            // A quoted value: the whole string, quotes included.
            if let Some(close) = rest[1..].find(quote) {
                return self.range(start + from, start + from + close + 2);
            }
        }
        let word = line_text[from..]
            .char_indices()
            .find(|(_, c)| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')))
            .map(|(i, _)| i)
            .unwrap_or(line_text.len() - from);
        let word = if word == 0 {
            line_text[from..].chars().next().map_or(0, char::len_utf8)
        } else {
            word
        };
        self.range(start + from, start + from + word)
    }
}

fn line_start(text: &str, line: u32) -> usize {
    if line == 0 {
        return 0;
    }
    text.match_indices('\n')
        .nth(line as usize - 1)
        .map(|(i, _)| i + 1)
        .unwrap_or(text.len())
}

/// The line starting at `start`, without its line end.
fn line_text(text: &str, start: usize) -> &str {
    let rest = &text[start..];
    let end = rest.find('\n').unwrap_or(rest.len());
    rest[..end].trim_end_matches('\r')
}

/// The byte offset of `line`:`character` (UTF-16) in `text`, clamped to
/// the line's end; past the last line, the end of the text.
pub fn offset(text: &str, line: u32, character: u32) -> usize {
    let start = line_start(text, line);
    let line_text = line_text(text, start);
    let mut units = 0;
    for (i, c) in line_text.char_indices() {
        if units >= character {
            return start + i;
        }
        units += c.len_utf16() as u32;
    }
    start + line_text.len()
}

/// The UTF-16 position of a byte offset in `text` (raw or normalised).
pub fn position(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let character = text[line_start..offset]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    Position::new(line, character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_columns_and_line_ends() {
        let doc = Doc::new("ab\r\nx😀y\r\n");
        assert_eq!(doc.text, "ab\nx😀y\n");
        assert!(doc.crlf && !doc.bom);
        let y = doc.text.find('y').unwrap();
        assert_eq!(doc.position(y), Position::new(1, 3));
        assert_eq!(doc.offset(Position::new(1, 3)), y);
        assert_eq!(doc.offset(Position::new(0, 99)), 2, "clamped to the line");
        assert_eq!(doc.offset(Position::new(9, 0)), doc.text.len());
        assert_eq!(doc.outgoing("a\nb"), "a\r\nb");
    }

    #[test]
    fn a_byte_order_mark_shifts_the_first_line_only() {
        let doc = Doc::new("\u{feff}ab\ncd");
        assert_eq!(doc.text, "ab\ncd");
        assert_eq!(doc.position(1), Position::new(0, 2));
        assert_eq!(doc.offset(Position::new(0, 2)), 1);
        assert_eq!(doc.position(4), Position::new(1, 1));
    }

    #[test]
    fn findings_underline_their_word() {
        let doc = Doc::new("+++\nparent = \"REQ-099\"\n");
        let r = doc.finding_range(2, 11);
        assert_eq!(
            (r.start, r.end),
            (Position::new(1, 10), Position::new(1, 17))
        );
        let r = doc.finding_range(1, 1);
        assert_eq!((r.start, r.end), (Position::new(0, 0), Position::new(0, 1)));
        let r = doc.finding_range(9, 1);
        assert_eq!(r.start, r.end, "past the end is empty, not a panic");
    }
}
