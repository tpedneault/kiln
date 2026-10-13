//! Item-level changes between two loaded projects (DESIGN.md,
//! "Baseline"): which items were added, removed or changed, and in what
//! (status, title, type, parent, links, tags, owner, due, Jira keys, body,
//! the file it lives in). Items are matched by ID, so a renumbered item
//! reads as one removed and one added, which is what a reader of the old
//! document needs to know.
//!
//! Pure: the caller loads both sides (from a git revision or the working
//! tree); this compares what was parsed. Code references are not
//! compared: they move with every refactoring, and the item diff is for
//! readers of the design, not of the code.

use std::collections::BTreeSet;

use kiln_model::{Item, ItemId};

use crate::project::Project;

/// What happened to an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

impl ChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Changed => "changed",
            ChangeKind::Removed => "removed",
        }
    }
}

/// One difference in a changed item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detail {
    Status {
        from: String,
        to: String,
    },
    Title {
        from: String,
        to: String,
    },
    Type {
        from: String,
        to: String,
    },
    Parent {
        from: Option<String>,
        to: Option<String>,
    },
    LinkAdded {
        role: String,
        target: String,
    },
    LinkRemoved {
        role: String,
        target: String,
    },
    Tags {
        from: Vec<String>,
        to: Vec<String>,
    },
    Owner {
        from: Option<String>,
        to: Option<String>,
    },
    Due {
        from: Option<String>,
        to: Option<String>,
    },
    Jira {
        from: Vec<String>,
        to: Vec<String>,
    },
    Body,
    Moved {
        from: String,
        to: String,
    },
}

impl Detail {
    /// `status approved → implemented`, `+ verifies REQ-002`, `body`.
    pub fn describe(&self) -> String {
        let opt = |o: &Option<String>| o.clone().unwrap_or_else(|| "(none)".into());
        let list = |l: &[String]| {
            if l.is_empty() {
                "(none)".to_owned()
            } else {
                l.join(", ")
            }
        };
        match self {
            Detail::Status { from, to } => format!("status {from} → {to}"),
            Detail::Title { from, to } => format!("title \"{from}\" → \"{to}\""),
            Detail::Type { from, to } => format!("type {from} → {to}"),
            Detail::Parent { from, to } => format!("parent {} → {}", opt(from), opt(to)),
            Detail::LinkAdded { role, target } => format!("+ {role} {target}"),
            Detail::LinkRemoved { role, target } => format!("− {role} {target}"),
            Detail::Tags { from, to } => format!("tags {} → {}", list(from), list(to)),
            Detail::Owner { from, to } => format!("owner {} → {}", opt(from), opt(to)),
            Detail::Due { from, to } => format!("due {} → {}", opt(from), opt(to)),
            Detail::Jira { from, to } => format!("jira {} → {}", list(from), list(to)),
            Detail::Body => "body".into(),
            Detail::Moved { from, to } => format!("moved {from} → {to}"),
        }
    }
}

/// An added, removed or changed item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemChange {
    pub id: ItemId,
    pub kind: ChangeKind,
    /// The newer title (the old one for a removed item).
    pub title: String,
    pub type_name: String,
    /// Where it is now (where it was, for a removed item).
    pub path: String,
    /// Empty unless `kind` is `Changed`.
    pub details: Vec<Detail>,
}

impl ItemChange {
    /// The details, one line: `status a → b; + verifies REQ-002; body`.
    pub fn summary(&self) -> String {
        self.details
            .iter()
            .map(Detail::describe)
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn first<'p>(p: &'p Project, id: &ItemId) -> Option<&'p Item> {
    p.item(id)
}

fn parent(i: &Item) -> Option<String> {
    i.parent.as_ref().map(|r| r.raw.clone())
}

fn links(i: &Item) -> BTreeSet<(String, String)> {
    i.links
        .iter()
        .map(|l| (l.role.clone(), l.target.raw.clone()))
        .collect()
}

fn sorted(v: &[String]) -> Vec<String> {
    let mut v = v.to_vec();
    v.sort();
    v
}

/// What changed in one item from `a` to `b`.
fn details(a: &Item, b: &Item) -> Vec<Detail> {
    let mut out = Vec::new();
    if a.status != b.status {
        out.push(Detail::Status {
            from: a.status.clone(),
            to: b.status.clone(),
        });
    }
    if a.title != b.title {
        out.push(Detail::Title {
            from: a.title.clone(),
            to: b.title.clone(),
        });
    }
    if a.type_name != b.type_name {
        out.push(Detail::Type {
            from: a.type_name.clone(),
            to: b.type_name.clone(),
        });
    }
    if parent(a) != parent(b) {
        out.push(Detail::Parent {
            from: parent(a),
            to: parent(b),
        });
    }
    let (la, lb) = (links(a), links(b));
    for (role, target) in lb.difference(&la) {
        out.push(Detail::LinkAdded {
            role: role.clone(),
            target: target.clone(),
        });
    }
    for (role, target) in la.difference(&lb) {
        out.push(Detail::LinkRemoved {
            role: role.clone(),
            target: target.clone(),
        });
    }
    if sorted(&a.tags) != sorted(&b.tags) {
        out.push(Detail::Tags {
            from: sorted(&a.tags),
            to: sorted(&b.tags),
        });
    }
    if a.owner != b.owner {
        out.push(Detail::Owner {
            from: a.owner.clone(),
            to: b.owner.clone(),
        });
    }
    if a.due != b.due {
        out.push(Detail::Due {
            from: a.due.map(|d| d.to_string()),
            to: b.due.map(|d| d.to_string()),
        });
    }
    if sorted(&a.jira) != sorted(&b.jira) {
        out.push(Detail::Jira {
            from: sorted(&a.jira),
            to: sorted(&b.jira),
        });
    }
    if a.body.trim() != b.body.trim() {
        out.push(Detail::Body);
    }
    if a.path != b.path {
        out.push(Detail::Moved {
            from: a.path.clone(),
            to: b.path.clone(),
        });
    }
    out
}

/// Every item added, changed or removed from `old` to `new`: added and
/// changed ones in `new`'s order, then removed ones in `old`'s. A
/// duplicated ID is compared by its first copy on each side.
// implements: REQ-016
pub fn diff(old: &Project, new: &Project) -> Vec<ItemChange> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for b in &new.items {
        if !seen.insert(b.id.clone()) {
            continue;
        }
        let change = |kind, details| ItemChange {
            id: b.id.clone(),
            kind,
            title: b.title.clone(),
            type_name: b.type_name.clone(),
            path: b.path.clone(),
            details,
        };
        match first(old, &b.id) {
            None => out.push(change(ChangeKind::Added, Vec::new())),
            Some(a) => {
                let d = details(a, b);
                if !d.is_empty() {
                    out.push(change(ChangeKind::Changed, d));
                }
            }
        }
    }
    let mut gone = BTreeSet::new();
    for a in &old.items {
        if first(new, &a.id).is_none() && gone.insert(a.id.clone()) {
            out.push(ItemChange {
                id: a.id.clone(),
                kind: ChangeKind::Removed,
                title: a.title.clone(),
                type_name: a.type_name.clone(),
                path: a.path.clone(),
                details: Vec::new(),
            });
        }
    }
    out
}
