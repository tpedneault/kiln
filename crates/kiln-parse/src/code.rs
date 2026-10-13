//! Code references: source lines matching a configured pattern
//! (FORMAT.md, "Code references").
//!
//! Whole-text regex matching, not a language parser: a match inside a
//! string literal counts. That keeps the scanner language-agnostic and
//! fast, and the role word and colon make accidental matches rare.

use kiln_model::{CodeRef, Ref};
use regex::{Regex, RegexBuilder};

use crate::text::LineIndex;

/// Compiled patterns are capped, so a hostile `kiln.toml` can't make
/// Kiln build an enormous automaton (`regex` matching is linear time).
const PATTERN_SIZE_LIMIT: usize = 1 << 20;

/// Files over this are skipped, as are files with a NUL byte near the
/// start (binary).
pub const MAX_CODE_FILE: usize = 2 * 1024 * 1024;

/// A compiled pattern, multi-line so `^` and `$` mean line ends.
#[derive(Debug, Clone)]
pub struct CodePattern {
    regex: Regex,
}

/// Compiles `pattern`, which must have a named group `ids`.
pub fn compile_pattern(pattern: &str) -> Result<CodePattern, String> {
    let regex = RegexBuilder::new(pattern)
        .multi_line(true)
        .size_limit(PATTERN_SIZE_LIMIT)
        .build()
        .map_err(|e| {
            let message = e.to_string();
            let last = message
                .lines()
                .last()
                .unwrap_or("invalid")
                .trim()
                .to_owned();
            format!("`{pattern}` isn't a valid regex: {last}")
        })?;
    if !regex.capture_names().any(|n| n == Some("ids")) {
        return Err(format!("`{pattern}` has no named group `ids`"));
    }
    Ok(CodePattern { regex })
}

/// Whether `bytes` should be scanned: small enough and not binary.
pub fn is_scannable(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_CODE_FILE && !bytes[..bytes.len().min(8192)].contains(&0)
}

/// Every code reference in `text`. The role is the word just before the
/// colon that precedes the `ids` group; IDs are split on commas. Spans are
/// in `text` as given (code files are not normalised).
pub fn scan_code(path: &str, text: &str, patterns: &[CodePattern]) -> Vec<CodeRef> {
    let mut out = Vec::new();
    let mut lines: Option<LineIndex> = None;
    for pattern in patterns {
        for caps in pattern.regex.captures_iter(text) {
            let (Some(whole), Some(ids)) = (caps.get(0), caps.name("ids")) else {
                continue;
            };
            let lines = lines.get_or_insert_with(|| LineIndex::new(text));
            let (role, role_start) = role_before(text, whole.start(), ids.start());
            let role_span = lines.span(role_start, role_start + role.len());
            let mut offset = ids.start();
            for part in ids.as_str().split(',') {
                let trimmed = part.trim();
                let start = offset + (part.len() - part.trim_start().len());
                if !trimmed.is_empty() {
                    out.push(CodeRef {
                        path: path.to_owned(),
                        role: role.to_owned(),
                        role_span,
                        target: Ref::new(trimmed, lines.span(start, start + trimmed.len())),
                    });
                }
                offset += part.len() + 1;
            }
        }
    }
    out.sort_by_key(|r| r.target.span.start);
    out
}

/// The last word (`[a-z0-9-]`) before the colon preceding `ids_start`,
/// within the match.
fn role_before(text: &str, match_start: usize, ids_start: usize) -> (&str, usize) {
    let head = &text[match_start..ids_start];
    let head = head.trim_end();
    let head = head.strip_suffix(':').unwrap_or(head).trim_end();
    let word_start = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|i| i + 1)
        .unwrap_or(0);
    (&head[word_start..], match_start + word_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture text is assembled at run time so this file's own source
    // holds no code references for Kiln's dogfood check to resolve.
    fn line(role: &str, ids: &str) -> String {
        format!("// {role}: {ids}\n")
    }

    #[test]
    fn roles_and_ids_with_their_lines() {
        let pattern = compile_pattern(&kiln_model_default()).unwrap();
        let text = format!(
            "fn a() {{}}\n{}#[test]\n    {}",
            line("implements", "REQ-014"),
            line("verifies", "REQ-014, REQ-015")
        );
        let refs = scan_code("src/a.rs", &text, &[pattern]);
        let got: Vec<String> = refs
            .iter()
            .map(|r| format!("{} {} {}", r.role, r.target.raw, r.location()))
            .collect();
        assert_eq!(
            got,
            [
                "implements REQ-014 src/a.rs:2",
                "verifies REQ-014 src/a.rs:4",
                "verifies REQ-015 src/a.rs:4"
            ]
        );
        let second = &refs[2];
        assert_eq!(
            &text[second.target.span.start..second.target.span.end],
            "REQ-015"
        );
        assert_eq!(second.target.span.column, 27);
    }

    #[test]
    fn a_pattern_without_ids_is_refused() {
        assert!(compile_pattern("implements: [A-Z]+")
            .unwrap_err()
            .contains("no named group `ids`"));
        assert!(compile_pattern("(")
            .unwrap_err()
            .contains("isn't a valid regex"));
    }

    #[test]
    fn binary_and_huge_files_are_not_scanned() {
        assert!(is_scannable(b"text"));
        assert!(!is_scannable(b"a\0b"));
        assert!(!is_scannable(&vec![b'a'; MAX_CODE_FILE + 1]));
    }

    fn kiln_model_default() -> String {
        let id = r"[A-Z][A-Z0-9]*-\d+";
        format!(r"\b(?:implements|verifies):\s*(?P<ids>{id}(?:\s*,\s*{id})*)")
    }
}
