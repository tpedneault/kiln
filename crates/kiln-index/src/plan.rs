//! The plan (ADR-018): which work is ready to start, which waits and on
//! what, grouped by the milestone it is part of. `kiln next` and `kiln
//! status` print it and the plan checks (K211–K214) use its definitions,
//! so what one calls ready the others agree on.
//!
//! **Work** is an item of a type the `part-of` role may link from (`task`
//! by default). It is **open** when neither done nor closed, and
//! **ready** when open, not `blocked` by status, and everything it
//! `depends-on` is done or closed. A milestone groups and orders work but
//! doesn't gate it: work in a milestone that depends on an unfinished one
//! is ready when its own dependencies are, because teams overlap
//! milestones and a gate would hide what could be picked up.
//!
//! Like `progress`, it names the default roles (`part-of`, `depends-on`)
//! and the `blocked` status on purpose: a project that calls them
//! otherwise simply has no plan to show. It doesn't judge the plan; the
//! contradictions are `kiln-check`'s.

use kiln_model::Item;

use crate::project::{Progress, Project};

/// The role that puts work in a milestone.
pub const PART_OF: &str = "part-of";
/// The role that orders work.
pub const DEPENDS_ON: &str = "depends-on";
/// The status a person sets on work that can't move for a reason the
/// links don't show.
pub const BLOCKED: &str = "blocked";

/// Open work that can't start yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting<'p> {
    pub item: &'p Item,
    /// What it `depends-on` that is still open, in link order.
    pub on: Vec<&'p Item>,
    /// Its status is `blocked`.
    pub blocked: bool,
}

/// One milestone's work, or the work in no milestone (`milestone: None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group<'p> {
    pub milestone: Option<&'p Item>,
    /// Done ÷ non-closed over this milestone's own work
    /// ([`Project::progress`]); for the work in no milestone, over it.
    pub progress: Progress,
    pub ready: Vec<&'p Item>,
    pub waiting: Vec<Waiting<'p>>,
}

impl Project {
    /// The types that are work: those `part-of` may link from.
    pub fn work_types(&self) -> Vec<&str> {
        self.config
            .role(PART_OF)
            .map(|r| {
                r.from
                    .iter()
                    .map(String::as_str)
                    .filter(|t| *t != kiln_model::CODE)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_work(&self, item: &Item) -> bool {
        self.work_types().contains(&item.type_name.as_str())
    }

    /// Neither done nor closed.
    pub fn is_open(&self, item: &Item) -> bool {
        !self.is_done(item) && !self.is_closed(item)
    }

    /// What `item` `depends-on` that is still open, in link order; a target
    /// that doesn't exist is K102's, not a wait.
    pub fn open_dependencies(&self, item: &Item) -> Vec<&Item> {
        item.links
            .iter()
            .filter(|l| l.role == DEPENDS_ON)
            .filter_map(|l| l.target.id.as_ref().ok())
            .filter_map(|id| self.item(id))
            .filter(|d| d.id != item.id && self.is_open(d))
            .collect()
    }

    /// The milestones `item` is `part-of`, or, when it names none, those
    /// of its nearest ancestor that does: a sub-task belongs where its
    /// task does.
    pub fn milestones_of(&self, item: &Item) -> Vec<&Item> {
        let own = |i: &Item| -> Vec<&Item> {
            i.links
                .iter()
                .filter(|l| l.role == PART_OF)
                .filter_map(|l| l.target.id.as_ref().ok())
                .filter_map(|id| self.item(id))
                .collect()
        };
        let mine = own(item);
        if !mine.is_empty() {
            return mine;
        }
        self.ancestors(&item.id)
            .into_iter()
            .map(own)
            .find(|m| !m.is_empty())
            .unwrap_or_default()
    }

    /// Whether open `item` is ready to start.
    pub fn is_ready(&self, item: &Item) -> bool {
        self.is_open(item) && item.status != BLOCKED && self.open_dependencies(item).is_empty()
    }

    /// The plan: a group per milestone that is open or still has open
    /// work, in item order, then the work in no milestone when there is
    /// any. Work counts under the first milestone it is part of.
    // implements: REQ-020
    pub fn plan(&self) -> Vec<Group<'_>> {
        let milestone_types: Vec<&str> = self
            .config
            .role(PART_OF)
            .map(|r| r.to.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let mut groups: Vec<Group> = self
            .items
            .iter()
            .filter(|i| milestone_types.contains(&i.type_name.as_str()))
            .map(|m| Group {
                milestone: Some(m),
                progress: Progress { done: 0, total: 0 },
                ready: Vec::new(),
                waiting: Vec::new(),
            })
            .collect();
        let mut loose = Group {
            milestone: None,
            progress: Progress { done: 0, total: 0 },
            ready: Vec::new(),
            waiting: Vec::new(),
        };
        let work_types = self.work_types();
        for item in self
            .items
            .iter()
            .filter(|i| work_types.contains(&i.type_name.as_str()))
        {
            if self.is_closed(item) {
                continue;
            }
            let first = self.milestones_of(item).first().map(|m| m.id.clone());
            let group = match first.and_then(|id| {
                groups
                    .iter_mut()
                    .find(|g| g.milestone.is_some_and(|m| m.id == id))
            }) {
                Some(g) => g,
                None => &mut loose,
            };
            group.progress.total += 1;
            if self.is_done(item) {
                group.progress.done += 1;
            } else if self.is_ready(item) {
                group.ready.push(item);
            } else {
                group.waiting.push(Waiting {
                    item,
                    on: self.open_dependencies(item),
                    blocked: item.status == BLOCKED,
                });
            }
        }
        // A milestone's count is [`Project::progress`], so it agrees with
        // every other view even for work `part-of` several milestones,
        // which is listed only under its first.
        for g in &mut groups {
            if let Some(m) = g.milestone {
                g.progress = self
                    .progress(&m.id)
                    .unwrap_or(Progress { done: 0, total: 0 });
            }
        }
        groups.retain(|g| {
            g.milestone.is_some_and(|m| self.is_open(m))
                || !g.ready.is_empty()
                || !g.waiting.is_empty()
        });
        if !loose.ready.is_empty() || !loose.waiting.is_empty() {
            groups.push(loose);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::item;
    use crate::{MemorySource, Project};

    fn project(files: &[(&str, String)]) -> Project {
        let mut source = MemorySource::new().with("kiln.toml", kiln_model::default_config("Demo"));
        for (path, text) in files {
            source = source.with(path, text.clone());
        }
        Project::load(&source).unwrap()
    }

    fn ids(items: &[&kiln_model::Item]) -> Vec<String> {
        items.iter().map(|i| i.id.to_string()).collect()
    }

    #[test]
    // verifies: REQ-020
    fn ready_work_has_nothing_open_to_wait_on() {
        let p = project(&[
            (
                "design/MS-001.md",
                item("MS-001", "milestone", "active", ""),
            ),
            (
                "design/MS-002.md",
                item(
                    "MS-002",
                    "milestone",
                    "planned",
                    "[links]\ndepends-on = [\"MS-001\"]\n",
                ),
            ),
            ("design/MS-003.md", item("MS-003", "milestone", "done", "")),
            (
                "design/TASK-001.md",
                item(
                    "TASK-001",
                    "task",
                    "done",
                    "[links]\npart-of = [\"MS-001\"]\n",
                ),
            ),
            (
                "design/TASK-002.md",
                item(
                    "TASK-002",
                    "task",
                    "doing",
                    "[links]\npart-of = [\"MS-001\"]\ndepends-on = [\"TASK-001\"]\n",
                ),
            ),
            (
                "design/TASK-003.md",
                item(
                    "TASK-003",
                    "task",
                    "todo",
                    "[links]\npart-of = [\"MS-001\"]\ndepends-on = [\"TASK-002\"]\n",
                ),
            ),
            (
                "design/TASK-004.md",
                item(
                    "TASK-004",
                    "task",
                    "blocked",
                    "[links]\npart-of = [\"MS-002\"]\n",
                ),
            ),
            // Ready although its milestone depends on an unfinished one.
            (
                "design/TASK-005.md",
                item(
                    "TASK-005",
                    "task",
                    "todo",
                    "[links]\npart-of = [\"MS-002\"]\n",
                ),
            ),
            // A sub-task belongs where its parent does.
            (
                "design/TASK-006.md",
                item("TASK-006", "task", "todo", "parent = \"TASK-005\"\n"),
            ),
            ("design/TASK-007.md", item("TASK-007", "task", "todo", "")),
            (
                "design/TASK-008.md",
                item("TASK-008", "task", "dropped", ""),
            ),
        ]);
        let plan = p.plan();
        let names: Vec<String> = plan
            .iter()
            .map(|g| g.milestone.map_or("none".into(), |m| m.id.to_string()))
            .collect();
        assert_eq!(
            names,
            ["MS-001", "MS-002", "none"],
            "MS-003 is done with nothing open"
        );
        assert_eq!(ids(&plan[0].ready), ["TASK-002"]);
        assert_eq!(plan[0].progress.done, 1);
        assert_eq!(plan[0].progress.total, 3);
        assert_eq!(plan[0].waiting.len(), 1);
        assert_eq!(plan[0].waiting[0].item.id.to_string(), "TASK-003");
        assert_eq!(ids(&plan[0].waiting[0].on), ["TASK-002"]);
        assert_eq!(ids(&plan[1].ready), ["TASK-005", "TASK-006"]);
        assert!(plan[1].waiting[0].blocked && plan[1].waiting[0].on.is_empty());
        assert_eq!(ids(&plan[2].ready), ["TASK-007"], "closed work is left out");
    }
}
