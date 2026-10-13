//! The planner (CONFLUENCE.md, "How a publish runs", step 3): a pure
//! function of desired pages and a snapshot of remote ones, tested
//! table-driven. Each desired page is matched to a remote page by the key
//! in its `kiln` property; pages without this target's property are
//! foreign and never touched.

use std::collections::{BTreeSet, HashMap};

use kiln_model::OnDrift;
use serde::Serialize;

use crate::desired::DesiredPage;
use crate::model::{RemotePage, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Create,
    Update,
    Unchanged,
    /// Edited in Confluence since Kiln's last write, and left alone
    /// (`on_drift = "skip"`) or refusing the plan (`stop`).
    Drift,
    /// A page of this target that nothing produces any more.
    Retire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Op {
    pub kind: OpKind,
    pub key: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<u64>,
    pub reasons: Vec<String>,
    /// The storage body's size in bytes (desired pages).
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Plan {
    pub ops: Vec<Op>,
    /// Titles of pages under the root without this target's property.
    pub foreign: Vec<String>,
    pub warnings: Vec<String>,
    /// Why nothing may be applied; empty when the plan can go ahead.
    pub refusals: Vec<String>,
}

impl Plan {
    pub fn count(&self, kind: OpKind) -> usize {
        self.ops.iter().filter(|o| o.kind == kind).count()
    }

    /// Whether applying would change anything.
    pub fn changes(&self) -> bool {
        self.ops
            .iter()
            .any(|o| matches!(o.kind, OpKind::Create | OpKind::Update | OpKind::Retire))
    }
}

/// What the planner needs besides the pages.
#[derive(Debug, Clone)]
pub struct PlanOptions {
    pub project: String,
    pub export: String,
    pub on_drift: OnDrift,
    /// `--force`: overwrite drift.
    pub force: bool,
    pub max_page_kib: u64,
    /// The target writes into an existing page (`page =`).
    pub adopting: bool,
    /// `--adopt` (K3).
    pub adopt: bool,
}

/// Plans `desired` against `snapshot`.
pub fn plan(desired: &[DesiredPage], snapshot: &Snapshot, options: &PlanOptions) -> Plan {
    let mut out = Plan::default();
    let own = |p: &RemotePage| {
        p.property
            .as_ref()
            .is_some_and(|k| k.project == options.project && k.export == options.export)
    };
    let by_key: HashMap<&str, &RemotePage> = snapshot
        .pages()
        .filter(|p| own(p))
        .filter_map(|p| p.property.as_ref().map(|k| (k.key.as_str(), p)))
        .collect();
    let key_of_id: HashMap<u64, &str> = by_key.iter().map(|(k, p)| (p.id, *k)).collect();
    for page in snapshot.descendants.iter().filter(|p| !own(p)) {
        out.foreign.push(page.title.clone());
    }

    let limit = options.max_page_kib * 1024;
    for d in desired {
        let size = d.body.len();
        if size as u64 > 4 * limit {
            out.refusals.push(format!(
                "\"{}\" is {} KiB, over four times max_page_kib ({} KiB); use layout = \"sections\"",
                d.title,
                size / 1024,
                options.max_page_kib
            ));
        } else if size as u64 > limit {
            out.warnings.push(format!(
                "\"{}\" is {} KiB, over max_page_kib ({} KiB): large pages are slow to open and edit; consider layout = \"sections\"",
                d.title,
                size / 1024,
                options.max_page_kib
            ));
        }
        let op = |kind, page_id, reasons| Op {
            kind,
            key: d.key.clone(),
            title: d.title.clone(),
            page_id,
            reasons,
            size,
        };

        // The adopted page: the root without a property of ours.
        let adopted = (d.key == "doc" && options.adopting)
            .then_some(snapshot.root.as_ref())
            .flatten()
            .filter(|r| !own(r));
        if let Some(root) = adopted {
            if root.empty || options.adopt {
                out.ops.push(op(
                    OpKind::Update,
                    Some(root.id),
                    vec!["adopt the existing page".into()],
                ));
            } else {
                out.refusals.push(format!(
                    "page {} (\"{}\") already has content; publishing into it replaces it, which needs --adopt",
                    root.id, root.title
                ));
                out.ops.push(op(
                    OpKind::Update,
                    Some(root.id),
                    vec!["adopt the existing page (needs --adopt)".into()],
                ));
            }
            continue;
        }

        let Some(remote) = by_key.get(d.key.as_str()) else {
            out.ops.push(op(OpKind::Create, None, vec!["new".into()]));
            continue;
        };
        let property = remote.property.as_ref().expect("own pages have a property");
        let edited = remote.version > property.published_version
            && remote
                .body_hash
                .as_ref()
                .is_some_and(|h| *h != property.remote_hash);
        if edited {
            let who = format!(
                "drift: edited by {} at {}, version {}",
                if remote.version_author.is_empty() {
                    "someone"
                } else {
                    &remote.version_author
                },
                if remote.version_when.is_empty() {
                    "an unknown time"
                } else {
                    &remote.version_when
                },
                remote.version
            );
            let action = if options.force {
                OnDrift::Overwrite
            } else {
                options.on_drift
            };
            match action {
                OnDrift::Stop => {
                    out.refusals.push(format!(
                        "\"{}\" was edited in Confluence ({}); copy the edit into the repository, then run again with --force, or set on_drift = \"skip\"",
                        remote.title,
                        who.trim_start_matches("drift: ")
                    ));
                    out.ops.push(op(OpKind::Drift, Some(remote.id), vec![who]));
                    continue;
                }
                OnDrift::Skip => {
                    out.ops.push(op(
                        OpKind::Drift,
                        Some(remote.id),
                        vec![who, "skipped".into()],
                    ));
                    continue;
                }
                OnDrift::Overwrite => {
                    out.ops.push(op(
                        OpKind::Update,
                        Some(remote.id),
                        vec![who, "overwritten".into()],
                    ));
                    continue;
                }
            }
        }
        if property.render_hash == d.render_hash {
            out.ops
                .push(op(OpKind::Unchanged, Some(remote.id), Vec::new()));
            continue;
        }
        let mut reasons = Vec::new();
        if remote.title != d.title {
            reasons.push(format!("title \"{}\" → \"{}\"", remote.title, d.title));
        }
        let remote_parent = remote.parent_id.and_then(|id| key_of_id.get(&id).copied());
        if d.key != "doc" && remote_parent != d.parent.as_deref() {
            reasons.push(format!(
                "parent {} → {}",
                remote_parent.unwrap_or("?"),
                d.parent.as_deref().unwrap_or("?")
            ));
        }
        let mut changed = Vec::new();
        let mut added = Vec::new();
        for (id, commit) in &d.items {
            match property.items.get(id) {
                None => added.push(id.clone()),
                Some(c) if c != commit => changed.push(id.clone()),
                _ => {}
            }
        }
        let removed: Vec<String> = property
            .items
            .keys()
            .filter(|id| !d.items.contains_key(*id))
            .cloned()
            .collect();
        if !changed.is_empty() {
            reasons.push(format!("{} changed", summarise(&changed)));
        }
        if !added.is_empty() {
            reasons.push(format!("{} new", summarise(&added)));
        }
        if !removed.is_empty() {
            reasons.push(format!("{} gone", summarise(&removed)));
        }
        if reasons.is_empty() {
            reasons.push("content changed (template, layout or references)".into());
        }
        out.ops.push(op(OpKind::Update, Some(remote.id), reasons));
    }

    let desired_keys: BTreeSet<&str> = desired.iter().map(|d| d.key.as_str()).collect();
    let mut retired: Vec<(&str, &RemotePage)> = by_key
        .iter()
        .filter(|(k, _)| !desired_keys.contains(*k) && **k != "removed")
        .map(|(k, p)| (*k, *p))
        .collect();
    retired.sort_by_key(|(k, _)| *k);
    for (key, page) in retired {
        out.ops.push(Op {
            kind: OpKind::Retire,
            key: key.to_owned(),
            title: page.title.clone(),
            page_id: Some(page.id),
            reasons: vec!["no longer produced: moved under \"Removed from the repository\"".into()],
            size: 0,
        });
    }
    out
}

/// `REQ-014, ADR-004` or `REQ-014, ADR-004, REQ-031 (+3 more)`.
fn summarise(ids: &[String]) -> String {
    if ids.len() <= 3 {
        ids.join(", ")
    } else {
        format!("{} (+{} more)", ids[..3].join(", "), ids.len() - 3)
    }
}
