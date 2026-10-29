//! What a change touches (K18, `kiln affected`): from the lines changed
//! in each file to the items whose code or file changed (ADR-022), and,
//! for each, what verifies it, to re-check.
//!
//! A code reference covers its own line down to the line before the next
//! code reference in the same file, or the end of the file: Kiln doesn't
//! parse code (ADR-008), so where the code a reference is about ends
//! isn't known, and over-reporting beats missing. An item file that
//! changed touches its item. It reads nothing: the changed lines come
//! from `kiln-git`, the binary joins the two.

use std::collections::{BTreeMap, HashMap};

use kiln_model::{Item, ItemId};

use crate::project::{Project, Source};
use crate::results::VERIFIES;

/// How a change reached an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// Changed lines `from..=to` inside the range of the code reference at
    /// `path:line`, which links to the item by `role`.
    Code {
        path: String,
        line: u32,
        role: String,
        from: u32,
        to: u32,
    },
    /// The item's own file changed.
    File { path: String },
}

/// An item a change touches, how, and what verifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reached<'p> {
    pub item: &'p Item,
    pub touches: Vec<Touch>,
    /// What `verifies` it, to re-check: items and code locations.
    pub recheck: Vec<Source>,
}

impl Project {
    /// The items `changed` (path → changed `(first, last)` line ranges)
    /// reaches, in item order.
    // implements: REQ-023
    pub fn affected(&self, changed: &HashMap<String, Vec<(u32, u32)>>) -> Vec<Reached<'_>> {
        let mut touched: BTreeMap<usize, Vec<Touch>> = BTreeMap::new();
        let index_of = |id: &ItemId| self.indices(id).first().copied();
        // Code references are sorted by path, then position: each one's
        // range ends where the next line with a reference in its file
        // starts.
        for (i, code) in self.code_refs.iter().enumerate() {
            let Some(ranges) = changed.get(&code.path) else {
                continue;
            };
            let line = code.target.span.line;
            let end = self.code_refs[i + 1..]
                .iter()
                .take_while(|c| c.path == code.path)
                .map(|c| c.target.span.line)
                .find(|&l| l > line)
                .map_or(u32::MAX, |l| l - 1);
            let Ok(target) = &code.target.id else {
                continue;
            };
            let Some(item) = index_of(target) else {
                continue;
            };
            for &(from, to) in ranges {
                if from <= end && to >= line {
                    let touch = Touch::Code {
                        path: code.path.clone(),
                        line,
                        role: code.role.clone(),
                        from: from.max(line),
                        to: to.min(end),
                    };
                    let list = touched.entry(item).or_default();
                    if !list.contains(&touch) {
                        list.push(touch);
                    }
                }
            }
        }
        for (i, item) in self.items.iter().enumerate() {
            if changed.contains_key(&item.path) {
                touched.entry(i).or_default().push(Touch::File {
                    path: item.path.clone(),
                });
            }
        }
        touched
            .into_iter()
            .map(|(i, touches)| {
                let item = &self.items[i];
                let recheck = self
                    .inbound(&item.id)
                    .iter()
                    .filter(|l| l.role == VERIFIES && self.source_is_open(l.source))
                    .map(|l| l.source)
                    .collect();
                Reached {
                    item,
                    touches,
                    recheck,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Touch;
    use crate::tests::item;
    use crate::{MemorySource, Project};

    #[test]
    // verifies: REQ-023
    fn a_changed_line_reaches_the_reference_above_it() {
        let r = |role: &str, id: &str| format!("// {role}: {id}\n");
        let code = format!(
            "{}fn a() {{\n    1\n}}\n{}fn b() {{}}\n{}fn test_a() {{}}\n",
            r("implements", "REQ-001"),
            r("implements", "REQ-002"),
            r("verifies", "REQ-001")
        );
        let source = MemorySource::new()
            .with(
                "kiln.toml",
                format!(
                    "{}\n[code]\npaths = [\"src\"]\n",
                    kiln_model::default_config("T")
                ),
            )
            .with(
                "design/REQ-001.md",
                item("REQ-001", "requirement", "approved", ""),
            )
            .with(
                "design/REQ-002.md",
                item("REQ-002", "requirement", "approved", ""),
            )
            .with(
                "design/REQ-003.md",
                item("REQ-003", "requirement", "approved", ""),
            )
            .with("src/lib.rs", code);
        let p = Project::load(&source).unwrap();
        // Line 3 is inside fn a, under REQ-001's reference (lines 1–4).
        let changed = HashMap::from([
            ("src/lib.rs".to_owned(), vec![(3, 3)]),
            ("design/REQ-003.md".to_owned(), vec![(6, 6)]),
        ]);
        let reached = p.affected(&changed);
        let ids: Vec<String> = reached.iter().map(|r| r.item.id.to_string()).collect();
        assert_eq!(ids, ["REQ-001", "REQ-003"], "REQ-002's code didn't change");
        assert_eq!(
            reached[0].touches,
            [Touch::Code {
                path: "src/lib.rs".into(),
                line: 1,
                role: "implements".into(),
                from: 3,
                to: 3
            }]
        );
        assert_eq!(reached[0].recheck.len(), 1, "the test verifying REQ-001");
        assert_eq!(
            reached[1].touches,
            [Touch::File {
                path: "design/REQ-003.md".into()
            }]
        );
    }
}
