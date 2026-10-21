//! Planning a sync (INTEGRATION.md, "Jira"): what Kiln's items want of
//! the tickets it manages, against what Jira has. Pure: the remote state
//! is read by [`crate::push::read`], and the plan is applied by
//! [`crate::push::apply`].
//!
//! Kiln manages the sub-tasks of the project issue (`[jira] parent`), one
//! per synced item, and only those: it creates them, keeps their summary
//! (`TASK-001 Measure it`) and due date, and moves them to the status the
//! item's status maps to. A ticket an item links by hand, outside the
//! project issue, is never written (K207 still compares it). A managed
//! ticket no item links is reported, never deleted. Assignees, sprints
//! and comments stay Jira's.

use std::collections::BTreeMap;

use kiln_index::Project;
use kiln_model::{Item, ItemId, JiraSync};

/// One ticket as Jira has it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteTicket {
    pub key: String,
    pub summary: String,
    pub status: String,
    /// `new`, `indeterminate` or `done`.
    pub category: String,
    /// `YYYY-MM-DD`.
    pub due: Option<String>,
}

/// One way a ticket can move, from `GET …/transitions`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Move {
    pub id: String,
    pub name: String,
    pub to: String,
    pub to_category: String,
}

/// What the sync reads before planning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Remote {
    /// The project issue; `None` when `[jira] parent` isn't set yet.
    pub parent: Option<RemoteTicket>,
    /// Its sub-tasks: the tickets Kiln manages.
    pub subtasks: Vec<RemoteTicket>,
    /// For the sub-tasks whose status must change: their transitions.
    pub moves: BTreeMap<String, Vec<Move>>,
    /// The sub-task issue type, as (id, name).
    pub subtask_type: (String, String),
    /// The project issue's type, when it is to be created.
    pub parent_type: Option<(String, String)>,
    /// Required create fields Kiln doesn't set, by issue type name.
    pub missing_fields: Vec<(String, Vec<String>)>,
}

/// The status a ticket should reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A workflow status by name (`[jira.status]`).
    Status(String),
    /// A status category: `new`, `indeterminate`, `done`.
    Category(String),
}

impl Target {
    pub fn reached_by(&self, status: &str, category: &str) -> bool {
        match self {
            Target::Status(s) => s.eq_ignore_ascii_case(status),
            Target::Category(c) => c == category,
        }
    }

    fn of(&self, m: &Move) -> bool {
        self.reached_by(&m.to, &m.to_category)
    }

    pub fn describe(&self) -> String {
        match self {
            Target::Status(s) => s.clone(),
            Target::Category(c) => match c.as_str() {
                "new" => "a to-do status".into(),
                "done" => "a done status".into(),
                _ => "an in-progress status".into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// The project issue, which every sub-task goes under.
    CreateParent { summary: String },
    /// A sub-task for `item`, then a move to `target` when that isn't
    /// where a new ticket starts.
    Create {
        item: ItemId,
        summary: String,
        due: Option<String>,
        description: String,
        target: Option<Target>,
    },
    /// Summary or due date differ.
    Update {
        item: ItemId,
        key: String,
        summary: Option<String>,
        due: Option<Option<String>>,
    },
    /// A move to the mapped status, by transition `id`.
    Transition {
        item: ItemId,
        key: String,
        from: String,
        to: String,
        id: String,
    },
    /// No transition reaches the mapped status: left as it is.
    Refused {
        item: ItemId,
        key: String,
        why: String,
    },
    /// A managed ticket no item links, or whose item isn't synced.
    Orphan { key: String, summary: String },
    /// An item linked to a ticket outside the project issue.
    LeftAlone { item: ItemId, keys: Vec<String> },
}

/// A sync's plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub ops: Vec<Op>,
    /// Items whose ticket is already as wanted.
    pub unchanged: usize,
    /// Reasons nothing can be created (fields Jira requires).
    pub refusals: Vec<String>,
}

impl Plan {
    pub fn count(&self, f: impl Fn(&Op) -> bool) -> usize {
        self.ops.iter().filter(|o| f(o)).count()
    }

    /// Whether applying it writes anything.
    pub fn changes(&self) -> bool {
        self.ops.iter().any(|o| {
            matches!(
                o,
                Op::CreateParent { .. }
                    | Op::Create { .. }
                    | Op::Update { .. }
                    | Op::Transition { .. }
            )
        })
    }

    pub fn refused(&self) -> bool {
        !self.refusals.is_empty() || self.ops.iter().any(|o| matches!(o, Op::Refused { .. }))
    }
}

/// The items `sync` pushes, first copy of each ID, in project order.
pub fn synced<'p>(project: &'p Project, sync: &JiraSync) -> Vec<&'p Item> {
    let mut seen = std::collections::HashSet::new();
    project
        .items
        .iter()
        .filter(|i| sync.types.contains(&i.type_name))
        .filter(|i| seen.insert(i.id.clone()))
        .collect()
}

/// The ticket summary for an item.
pub fn summary(item: &Item) -> String {
    format!("{} {}", item.id, item.title)
}

/// The status `item`'s ticket should be in: `[jira.status]` by name, else
/// by category (the type's initial status: to do; a done or closed one:
/// done; anything between: in progress).
pub fn target(project: &Project, sync: &JiraSync, item: &Item) -> Target {
    if let Some(name) = sync.status.get(&item.status) {
        return Target::Status(name.clone());
    }
    let category = match project.type_of(item) {
        Some(t) if t.is_done(&item.status) || t.is_closed(&item.status) => "done",
        Some(t) if t.initial == item.status => "new",
        _ => "indeterminate",
    };
    Target::Category(category.into())
}

/// The keys of the managed tickets whose status must change: what
/// [`crate::push::read`] lists transitions for.
pub fn needs_moving(project: &Project, sync: &JiraSync, remote: &Remote) -> Vec<String> {
    let by_key: BTreeMap<&str, &RemoteTicket> = remote
        .subtasks
        .iter()
        .map(|t| (t.key.as_str(), t))
        .collect();
    let mut out = Vec::new();
    for item in synced(project, sync) {
        for key in &item.jira {
            if let Some(t) = by_key.get(key.as_str()) {
                if !target(project, sync, item).reached_by(&t.status, &t.category) {
                    out.push(t.key.clone());
                }
            }
        }
    }
    out
}

/// What applying would do. `description` is the text a new ticket gets.
pub fn plan(
    project: &Project,
    sync: &JiraSync,
    remote: &Remote,
    description: &dyn Fn(&Item) -> String,
) -> Plan {
    let mut out = Plan::default();
    for (ty, fields) in &remote.missing_fields {
        out.refusals.push(format!(
            "{} needs fields Kiln doesn't set ({}): give them defaults in Jira, or create the tickets by hand",
            ty,
            fields.join(", ")
        ));
    }
    if sync.parent.is_none() {
        out.ops.push(Op::CreateParent {
            summary: format!("Kiln: {}", project.config.name),
        });
    }
    let by_key: BTreeMap<&str, &RemoteTicket> = remote
        .subtasks
        .iter()
        .map(|t| (t.key.as_str(), t))
        .collect();
    let mut linked: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for item in synced(project, sync) {
        let mine: Vec<&RemoteTicket> = item
            .jira
            .iter()
            .filter_map(|k| by_key.get(k.as_str()).copied())
            .collect();
        let closed = project.is_closed(item);
        let want = target(project, sync, item);
        if mine.is_empty() {
            if !item.jira.is_empty() {
                out.ops.push(Op::LeftAlone {
                    item: item.id.clone(),
                    keys: item.jira.clone(),
                });
            } else if !closed {
                let starts = Target::Category("new".into());
                out.ops.push(Op::Create {
                    item: item.id.clone(),
                    summary: summary(item),
                    due: item.due.map(|d| d.to_string()),
                    description: description(item),
                    target: (want != starts).then_some(want),
                });
            }
            continue;
        }
        for t in mine {
            linked.insert(&t.key);
            let mut changed = false;
            let wanted_summary = summary(item);
            let wanted_due = item.due.map(|d| d.to_string());
            let summary = (t.summary != wanted_summary).then_some(wanted_summary);
            let due = (t.due != wanted_due).then_some(wanted_due);
            if summary.is_some() || due.is_some() {
                changed = true;
                out.ops.push(Op::Update {
                    item: item.id.clone(),
                    key: t.key.clone(),
                    summary,
                    due,
                });
            }
            if !want.reached_by(&t.status, &t.category) {
                changed = true;
                let moves = remote.moves.get(&t.key).cloned().unwrap_or_default();
                match moves.iter().find(|m| want.of(m)) {
                    Some(m) => out.ops.push(Op::Transition {
                        item: item.id.clone(),
                        key: t.key.clone(),
                        from: t.status.clone(),
                        to: m.to.clone(),
                        id: m.id.clone(),
                    }),
                    None => {
                        let names: Vec<String> = moves
                            .iter()
                            .map(|m| format!("{} → {}", m.name, m.to))
                            .collect();
                        out.ops.push(Op::Refused {
                            item: item.id.clone(),
                            key: t.key.clone(),
                            why: format!(
                                "no transition from {} to {} (the workflow offers: {})",
                                t.status,
                                want.describe(),
                                if names.is_empty() {
                                    "none".into()
                                } else {
                                    names.join(", ")
                                }
                            ),
                        });
                    }
                }
            }
            if !changed {
                out.unchanged += 1;
            }
        }
    }
    for t in &remote.subtasks {
        if !linked.contains(t.key.as_str()) {
            out.ops.push(Op::Orphan {
                key: t.key.clone(),
                summary: t.summary.clone(),
            });
        }
    }
    out
}

/// The Kiln status a ticket's status maps back to, for `kiln jira pull`:
/// `[jira.status]` inverted, else by category (to do: the type's initial
/// status; done: its first done status; in progress: its first status
/// that is none of those nor closed). `None` when the type has no such
/// status.
pub fn status_back(
    project: &Project,
    sync: &JiraSync,
    item: &Item,
    status: &str,
    category: &str,
) -> Option<String> {
    let ty = project.type_of(item)?;
    if let Some((kiln, _)) = sync
        .status
        .iter()
        .find(|(k, v)| v.eq_ignore_ascii_case(status) && ty.statuses.contains(k))
    {
        return Some(kiln.clone());
    }
    match category {
        "new" => Some(ty.initial.clone()),
        "done" => ty.done.first().cloned(),
        _ => ty
            .statuses
            .iter()
            .find(|s| **s != ty.initial && !ty.is_done(s) && !ty.is_closed(s))
            .cloned(),
    }
}
