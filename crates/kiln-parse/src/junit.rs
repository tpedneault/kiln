//! JUnit XML test reports (`--results`, K18): the one format every test
//! runner can write (cargo-nextest, pytest, JUnit, Jest, go-junit-report),
//! read as a flat list of test cases with their outcome. And the name of
//! the test a `verifies` code reference sits above, so a report's cases
//! can be matched to references (ADR-021).
//!
//! Only what matching needs is kept: a case's name, class name and file,
//! and whether it passed, failed (a `failure` or `error` child) or was
//! skipped. Suites, timings and output are ignored. Pure text in, values
//! out; finding and reading report files is the binary's.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::sync::LazyLock;

use regex::Regex;

/// How a test case ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestOutcome {
    Passed,
    Skipped,
    Failed,
}

/// One `<testcase>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCase {
    pub name: String,
    pub classname: Option<String>,
    /// The source file, when the runner writes one (pytest does).
    pub file: Option<String>,
    pub outcome: TestOutcome,
}

/// Every test case in a JUnit report, in document order. An error is one
/// line naming where the XML stopped being readable.
pub fn parse_junit(text: &str) -> Result<Vec<TestCase>, String> {
    let mut reader = Reader::from_str(text);
    let mut out = Vec::new();
    let mut open: Option<TestCase> = None;
    loop {
        let event = reader.read_event().map_err(|e| {
            format!(
                "not readable XML near byte {}: {e}",
                reader.error_position()
            )
        })?;
        match event {
            Event::Eof => break,
            Event::Start(ref e) if e.name().as_ref() == b"testcase" => {
                if let Some(case) = open.take() {
                    out.push(case);
                }
                open = Some(case_from(e)?);
            }
            Event::Empty(ref e) if e.name().as_ref() == b"testcase" => {
                out.push(case_from(e)?);
            }
            Event::Start(ref e) | Event::Empty(ref e) => {
                if let Some(case) = open.as_mut() {
                    match e.name().as_ref() {
                        b"failure" | b"error" => case.outcome = TestOutcome::Failed,
                        b"skipped" if case.outcome == TestOutcome::Passed => {
                            case.outcome = TestOutcome::Skipped
                        }
                        _ => {}
                    }
                }
            }
            Event::End(ref e) if e.name().as_ref() == b"testcase" => {
                if let Some(case) = open.take() {
                    out.push(case);
                }
            }
            _ => {}
        }
    }
    if let Some(case) = open {
        out.push(case);
    }
    Ok(out)
}

fn case_from(e: &BytesStart) -> Result<TestCase, String> {
    let mut case = TestCase {
        name: String::new(),
        classname: None,
        file: None,
        outcome: TestOutcome::Passed,
    };
    for attr in e.attributes() {
        let attr = attr.map_err(|e| format!("a testcase attribute isn't readable: {e}"))?;
        let value = unescape(&String::from_utf8_lossy(&attr.value));
        match attr.key.as_ref() {
            b"name" => case.name = value,
            b"classname" => case.classname = Some(value),
            b"file" => case.file = Some(value.replace('\\', "/")),
            _ => {}
        }
    }
    Ok(case)
}

/// The five XML entities and numeric references; anything else is kept.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let Some(end) = after.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = after;
            continue;
        };
        let entity = &after[..end];
        let decoded = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Lines looked at below a code reference for the test it is about.
const LOOK_AHEAD: usize = 8;

/// The name of the test declared below line `line` (1-based, the code
/// reference's own line) of `text` (ADR-021): skipping blank lines,
/// comments, attributes (`#[test]`) and decorators (`@Test`), the name
/// after `fn`, `def`, `func`, `function` or `sub`; else the string in
/// `it("…")`, `test("…")` or `describe("…")`; else the identifier before
/// the first `(`. `None` when nothing in the next few lines declares one.
pub fn test_name_after(text: &str, line: u32) -> Option<String> {
    static KEYWORD: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::new(r"\b(?:fn|def|func|function|sub)\s+([A-Za-z_][A-Za-z0-9_]*)").ok()
    });
    static DESCRIBED: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r#"\b(?:it|test|describe)\s*\(\s*["'`]([^"'`]+)["'`]"#).ok());
    static CALLED: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(").ok());
    let (keyword, described, called) = (KEYWORD.as_ref()?, DESCRIBED.as_ref()?, CALLED.as_ref()?);
    let skip = |l: &str| {
        l.is_empty()
            || l.starts_with("#[")
            || l.starts_with('@')
            || l.starts_with("//")
            || l.starts_with('#')
            || l.starts_with("--")
            || l.starts_with('*')
            || l.starts_with("/*")
    };
    let candidate = text
        .lines()
        .skip(line as usize)
        .take(LOOK_AHEAD)
        .map(str::trim)
        .find(|l| !skip(l))?;
    if let Some(c) = keyword.captures(candidate) {
        return Some(c[1].to_owned());
    }
    if let Some(c) = described.captures(candidate) {
        return Some(c[1].to_owned());
    }
    called.captures(candidate).map(|c| c[1].to_owned())
}

/// Whether report case `case` is the test named `name` in `path`: its
/// name, or the last `::`, `.` or `/` segment of it, is `name`; and when
/// the report gives a file, it ends with `path` (or `path` with it).
pub fn case_matches(case: &TestCase, name: &str, path: &str) -> bool {
    let last = case
        .name
        .rsplit([':', '.', '/'])
        .next()
        .unwrap_or(&case.name)
        .trim();
    // A parametrised pytest case is `test_x[1]`.
    let last = last.split('[').next().unwrap_or(last);
    let named = case.name == name || last == name;
    let in_file = case
        .file
        .as_deref()
        .is_none_or(|f| f.ends_with(path) || path.ends_with(f));
    named && in_file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_gives_each_case_and_its_outcome() {
        let xml = r#"<?xml version="1.0"?>
<testsuites><testsuite name="kiln">
  <testcase name="export::tests::is_deterministic" classname="kiln-export"/>
  <testcase name="test_round_trip[2]" classname="tests.test_csv" file="tests\test_csv.py" line="4">
    <failure message="1 != 2">assert 1 == 2 &amp; more</failure>
  </testcase>
  <testcase name="slow &lt;one&gt;"><skipped/></testcase>
  <testcase name="broken"><error type="Panic"/></testcase>
</testsuite></testsuites>"#;
        let cases = parse_junit(xml).unwrap();
        let summary: Vec<(&str, TestOutcome)> =
            cases.iter().map(|c| (c.name.as_str(), c.outcome)).collect();
        assert_eq!(
            summary,
            [
                ("export::tests::is_deterministic", TestOutcome::Passed),
                ("test_round_trip[2]", TestOutcome::Failed),
                ("slow <one>", TestOutcome::Skipped),
                ("broken", TestOutcome::Failed),
            ]
        );
        assert_eq!(cases[1].file.as_deref(), Some("tests/test_csv.py"));
        assert!(parse_junit("<a><b></a>").is_err());
    }

    #[test]
    fn the_test_below_a_reference_is_named_in_any_language() {
        // Assembled at run time, so Kiln's own check doesn't read these as
        // code references.
        let tag = |comment: &str| format!("{comment} {}: REQ-1\n", "verifies");
        let rust = format!("{}#[test]\nfn exports_are_deterministic() {{\n", tag("//"));
        assert_eq!(
            test_name_after(&rust, 1).as_deref(),
            Some("exports_are_deterministic")
        );
        let python = format!("{}@pytest.mark.slow\ndef test_round_trip():\n", tag("#"));
        assert_eq!(
            test_name_after(&python, 1).as_deref(),
            Some("test_round_trip")
        );
        let js = format!("{}it('exports twice the same', () => {{\n", tag("//"));
        assert_eq!(
            test_name_after(&js, 1).as_deref(),
            Some("exports twice the same")
        );
        let java = format!("{}@Test\nvoid exportsTwice() {{\n", tag("//"));
        assert_eq!(test_name_after(&java, 1).as_deref(), Some("exportsTwice"));
        assert_eq!(test_name_after(&format!("{}\n", tag("//")), 1), None);
    }

    #[test]
    fn cases_match_by_last_segment_and_file() {
        let case = |name: &str, file: Option<&str>| TestCase {
            name: name.into(),
            classname: None,
            file: file.map(Into::into),
            outcome: TestOutcome::Passed,
        };
        assert!(case_matches(
            &case("export::tests::is_det", None),
            "is_det",
            "src/a.rs"
        ));
        assert!(case_matches(
            &case("Tests.exportsTwice", None),
            "exportsTwice",
            "A.java"
        ));
        assert!(case_matches(
            &case("test_rt[2]", Some("tests/t.py")),
            "test_rt",
            "tests/t.py"
        ));
        assert!(!case_matches(
            &case("test_rt", Some("other.py")),
            "test_rt",
            "tests/t.py"
        ));
        assert!(!case_matches(
            &case("is_det_slow", None),
            "is_det",
            "src/a.rs"
        ));
    }
}
