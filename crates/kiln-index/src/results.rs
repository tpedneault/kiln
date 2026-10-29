//! Test results applied to the project (K18, `--results`): each code
//! reference that `verifies` gets a verdict from the JUnit report's cases
//! matched to the test below it (ADR-021), and one that didn't pass stops
//! counting as a link source, so coverage, `kiln trace` and K201 say what
//! passed in this run rather than what was written.
//!
//! It names the default `verifies` role on purpose, as K205 does. It reads
//! code files through the file source; parsing the report is the
//! binary's, through `kiln_parse::parse_junit`.

use std::collections::HashMap;

use kiln_parse::{case_matches, test_name_after, TestCase, TestOutcome};

use crate::project::Project;
use crate::source::FileSource;

/// The role whose code references test results judge.
pub const VERIFIES: &str = "verifies";

/// What the results say about one code reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every case matched passed.
    Passed,
    /// A matched case failed or errored (K216).
    Failed,
    /// No case matched, or only skipped ones (K217).
    NotRun,
}

/// What applying results found, for a summary line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResultsSummary {
    pub passed: usize,
    pub failed: usize,
    pub not_run: usize,
}

impl Project {
    /// Judges every `verifies` code reference by `cases`. Calling it
    /// again replaces the verdicts; with no cases, every reference is
    /// "not run", which is what a report with no tests means.
    // implements: REQ-023
    pub fn apply_results(&mut self, source: &dyn FileSource, cases: &[TestCase]) -> ResultsSummary {
        let mut texts: HashMap<String, Option<String>> = HashMap::new();
        let mut verdicts = HashMap::new();
        let mut summary = ResultsSummary::default();
        for (i, code) in self.code_refs.iter().enumerate() {
            if code.role != VERIFIES {
                continue;
            }
            let text = texts.entry(code.path.clone()).or_insert_with(|| {
                source
                    .read(&code.path)
                    .ok()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
            });
            let name = text
                .as_deref()
                .and_then(|t| test_name_after(t, code.target.span.line));
            let outcomes: Vec<TestOutcome> = match &name {
                None => Vec::new(),
                Some(n) => cases
                    .iter()
                    .filter(|c| case_matches(c, n, &code.path))
                    .map(|c| c.outcome)
                    .collect(),
            };
            let verdict = if outcomes.contains(&TestOutcome::Failed) {
                summary.failed += 1;
                Verdict::Failed
            } else if outcomes.contains(&TestOutcome::Passed) {
                summary.passed += 1;
                Verdict::Passed
            } else {
                summary.not_run += 1;
                Verdict::NotRun
            };
            verdicts.insert(i, verdict);
        }
        self.verdicts = verdicts;
        summary
    }

    /// The verdict on code reference `i`, when results were applied and it
    /// `verifies`.
    pub fn verdict(&self, i: usize) -> Option<Verdict> {
        self.verdicts.get(&i).copied()
    }

    /// The test name `kiln` matched code reference `i` by, for messages.
    pub fn test_name_of(&self, source: &dyn FileSource, i: usize) -> Option<String> {
        let code = self.code_refs.get(i)?;
        let bytes = source.read(&code.path).ok()?;
        test_name_after(&String::from_utf8_lossy(&bytes), code.target.span.line)
    }
}

#[cfg(test)]
mod tests {
    use crate::{MemorySource, Project, Source};
    use kiln_parse::parse_junit;

    #[test]
    // verifies: REQ-023
    fn only_a_passing_test_verifies() {
        let refs = |role: &str, id: &str| format!("// {role}: {id}\n");
        let source = MemorySource::new()
            .with(
                "kiln.toml",
                format!("{}\n[code]\npaths = [\"src\"]\n", kiln_model::default_config("T")),
            )
            .with("design/REQ-001.md", crate::tests::item("REQ-001", "requirement", "approved", ""))
            .with("design/REQ-002.md", crate::tests::item("REQ-002", "requirement", "approved", ""))
            .with("design/REQ-003.md", crate::tests::item("REQ-003", "requirement", "approved", ""))
            .with(
                "src/t.rs",
                format!(
                    "{}#[test]\nfn passes() {{}}\n{}#[test]\nfn fails() {{}}\n{}fn never_ran() {{}}\n",
                    refs("verifies", "REQ-001"),
                    refs("verifies", "REQ-002"),
                    refs("verifies", "REQ-003")
                ),
            );
        let mut p = Project::load(&source).unwrap();
        let id = |s: &str| s.parse().unwrap();
        assert!(
            p.covered(&id("REQ-002"), "verifies"),
            "without results, written is enough"
        );
        let cases = parse_junit(
            r#"<testsuite><testcase name="t::passes"/><testcase name="t::fails"><failure/></testcase></testsuite>"#,
        )
        .unwrap();
        let summary = p.apply_results(&source, &cases);
        assert_eq!((summary.passed, summary.failed, summary.not_run), (1, 1, 1));
        assert!(p.covered(&id("REQ-001"), "verifies"));
        assert!(!p.covered(&id("REQ-002"), "verifies"));
        assert!(!p.covered(&id("REQ-003"), "verifies"));
        assert!(!p.source_is_open(Source::Code(1)));
    }
}
