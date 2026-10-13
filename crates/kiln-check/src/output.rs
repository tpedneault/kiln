//! Findings as text, JSON, GitLab Code Quality and GitHub workflow
//! commands (FORMAT.md, "Check rules"). Deterministic: the same findings
//! always give the same bytes.

use kiln_model::{Finding, Severity};
use serde_json::{json, Value};

/// An output format for `kiln check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `path:line:col: K102 error: message`.
    Text,
    /// An array of findings with all fields.
    Json,
    /// GitLab Code Quality report JSON.
    Gitlab,
    /// `::error file=…,line=…::` workflow commands.
    Github,
}

/// One finding per line (or a JSON document), ending with a newline
/// unless there is nothing to say in text.
// implements: REQ-012
pub fn render(findings: &[Finding], format: Format) -> String {
    match format {
        Format::Text => findings.iter().map(|f| format!("{f}\n")).collect(),
        Format::Json => {
            let array: Vec<Value> = findings
                .iter()
                .map(|f| {
                    json!({
                        "code": f.rule.code(),
                        "severity": f.severity.as_str(),
                        "path": f.path,
                        "line": f.line,
                        "column": f.column,
                        "message": f.message,
                    })
                })
                .collect();
            pretty(&Value::Array(array))
        }
        Format::Gitlab => {
            let mut seen = std::collections::HashMap::new();
            let array: Vec<Value> = findings
                .iter()
                .map(|f| {
                    // Line numbers stay out of the fingerprint, so a finding
                    // that only moved isn't "new" on the merge request; a
                    // count tells repeated identical findings apart.
                    let key = format!("{}\0{}\0{}", f.rule.code(), f.path, f.message);
                    let n = seen.entry(key.clone()).or_insert(0u32);
                    *n += 1;
                    json!({
                        "description": format!("{}: {}", f.rule.code(), f.message),
                        "check_name": f.rule.code(),
                        "fingerprint": format!("{:016x}", fnv1a(format!("{key}\0{n}").as_bytes())),
                        "severity": match f.severity {
                            Severity::Error => "major",
                            _ => "minor",
                        },
                        "location": { "path": f.path, "lines": { "begin": f.line } },
                    })
                })
                .collect();
            pretty(&Value::Array(array))
        }
        Format::Github => findings
            .iter()
            .map(|f| {
                let level = match f.severity {
                    Severity::Error => "error",
                    _ => "warning",
                };
                format!(
                    "::{level} file={},line={},col={},title={}::{}\n",
                    property(&f.path),
                    f.line,
                    f.column,
                    f.rule.code(),
                    data(&f.message)
                )
            })
            .collect(),
    }
}

fn pretty(value: &Value) -> String {
    let mut s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "[]".to_owned());
    s.push('\n');
    s
}

/// GitHub's escaping for a workflow command's message.
fn data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// GitHub's escaping for a workflow command's property value.
fn property(s: &str) -> String {
    data(s).replace(':', "%3A").replace(',', "%2C")
}

/// FNV-1a, 64 bits: stable across versions and platforms, unlike std's
/// hasher, which fingerprints need.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// `2 errors, 1 warning`, or `no findings`.
pub fn summary(findings: &[Finding]) -> String {
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    match (errors, warnings) {
        (0, 0) => "no findings".to_owned(),
        (e, 0) => plural(e, "error"),
        (0, w) => plural(w, "warning"),
        (e, w) => format!("{}, {}", plural(e, "error"), plural(w, "warning")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_model::{Rule, Span};

    fn findings() -> Vec<Finding> {
        let span = Span {
            start: 0,
            end: 0,
            line: 3,
            column: 7,
        };
        let mut warning = Finding::new(
            Rule::K105,
            "design/a,b.md",
            span,
            "the file name doesn't start with REQ-001",
        );
        warning.severity = Severity::Warning;
        vec![
            Finding::new(
                Rule::K102,
                "design/a.md",
                span,
                "REQ-099 doesn't exist\nreally",
            ),
            warning,
        ]
    }

    #[test]
    // verifies: REQ-012
    fn text_and_github() {
        let f = findings();
        assert_eq!(
            render(&f, Format::Text),
            "design/a.md:3:7: K102 error: REQ-099 doesn't exist\nreally\ndesign/a,b.md:3:7: K105 warning: the file name doesn't start with REQ-001\n"
        );
        assert_eq!(
            render(&f, Format::Github),
            "::error file=design/a.md,line=3,col=7,title=K102::REQ-099 doesn't exist%0Areally\n::warning file=design/a%2Cb.md,line=3,col=7,title=K105::the file name doesn't start with REQ-001\n"
        );
    }

    #[test]
    // verifies: REQ-012
    fn json_and_gitlab() {
        let f = findings();
        let json: Value = serde_json::from_str(&render(&f, Format::Json)).unwrap();
        assert_eq!(json[0]["code"], "K102");
        assert_eq!(json[1]["severity"], "warning");
        let gitlab: Value = serde_json::from_str(&render(&f, Format::Gitlab)).unwrap();
        assert_eq!(gitlab[0]["location"]["lines"]["begin"], 3);
        assert_eq!(gitlab[0]["severity"], "major");
        assert_eq!(gitlab[1]["severity"], "minor");
        assert_ne!(gitlab[0]["fingerprint"], gitlab[1]["fingerprint"]);
        assert_eq!(render(&[], Format::Json), "[]\n");
    }

    #[test]
    fn summaries() {
        assert_eq!(summary(&findings()), "1 error, 1 warning");
        assert_eq!(summary(&[]), "no findings");
    }
}
