//! The Jira mirror (INTEGRATION.md, "Jira"): the status, assignee and
//! resolution of the tickets items link with `jira = "KEY-1"`, read-only,
//! through `fenix-jira`, kept in Kiln's state folder for ten minutes so
//! `kiln list --jira`, `kiln check` (K207) and the GUI don't each ask
//! Jira again.
//!
//! The mirror and K207 are pure; [`fetch`] reads it. From K10 Kiln also
//! pushes the items it manages (`[jira] project`) as sub-tasks of one
//! project issue (ADR-014): [`sync`] plans, purely, and [`push`] reads and
//! applies. Nothing else is written: assignees, sprints and comments stay
//! Jira's, and no ticket is ever deleted.
//!
//! `fenix-jira` speaks Jira Server/Data Center with a personal access
//! token. Jira Cloud needs Fenix's G6, and a proxy from the environment
//! needs G9 (INTEGRATION.md); until then a Cloud URL is refused by name,
//! and the mirror connects directly.

use std::collections::{BTreeMap, BTreeSet};

use kiln_index::Project;
use kiln_model::{Finding, Rule};
use serde::{Deserialize, Serialize};

/// How long a fetched mirror is used before Jira is asked again.
pub const FRESH_SECS: u64 = 600;

/// Keys are asked for this many at a time (`key IN (…)`).
const CHUNK: usize = 100;

/// One linked ticket, as the mirror keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub key: String,
    pub summary: String,
    /// The workflow's own name for the status (`In Review`).
    pub status: String,
    /// Whether the status is in Jira's Done category: resolved, whatever
    /// the workflow calls it. `fenix-jira` doesn't read the resolution
    /// field itself, and the category is what every workflow agrees on.
    pub resolved: bool,
    pub assignee: Option<String>,
}

impl Ticket {
    /// `In Review · Alice`, `Done`, for a column.
    pub fn line(&self) -> String {
        match &self.assignee {
            Some(a) => format!("{} · {a}", self.status),
            None => self.status.clone(),
        }
    }
}

/// The tickets of one Jira, with when they were read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    /// `[jira] url`, without a trailing slash.
    pub url: String,
    /// Seconds since the Unix epoch.
    pub fetched: u64,
    pub tickets: BTreeMap<String, Ticket>,
    /// Keys Jira doesn't know (or the token can't see).
    #[serde(default)]
    pub missing: BTreeSet<String>,
}

impl Mirror {
    /// Whether this mirror answers for `keys` of `url` at `now` without
    /// asking Jira.
    pub fn covers(&self, url: &str, keys: &[String], now: u64) -> bool {
        self.url == url.trim_end_matches('/')
            && now >= self.fetched
            && now - self.fetched < FRESH_SECS
            && keys
                .iter()
                .all(|k| self.tickets.contains_key(k) || self.missing.contains(k))
    }

    pub fn ticket(&self, key: &str) -> Option<&Ticket> {
        self.tickets.get(key)
    }

    /// A cache file's text; an unreadable one is no mirror.
    pub fn parse(text: &str) -> Option<Mirror> {
        serde_json::from_str(text).ok()
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Every Jira key the project's items link, sorted, once each.
pub fn keys(project: &Project) -> Vec<String> {
    let set: BTreeSet<String> = project
        .items
        .iter()
        .flat_map(|i| i.jira.iter().map(|k| k.trim().to_owned()))
        .filter(|k| !k.is_empty())
        .collect();
    set.into_iter().collect()
}

/// The cache file's name in the state folder for `url`:
/// `jira/<host and path, sanitised>.json`.
pub fn cache_name(url: &str) -> String {
    let bare = url
        .trim_end_matches('/')
        .split_once("://")
        .map_or(url, |(_, rest)| rest);
    let name: String = bare
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("jira/{name}.json")
}

/// Whether `url` is Jira Cloud, which `fenix-jira` can't sign in to yet
/// (G6).
pub fn is_cloud(url: &str) -> bool {
    url.split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .is_some_and(|host| host.to_ascii_lowercase().ends_with(".atlassian.net"))
}

/// The one-line refusal for a Jira Cloud URL.
pub fn cloud_refusal(url: &str) -> String {
    format!("{url} is Jira Cloud, which fenix-jira can't sign in to yet (INTEGRATION.md, G6); the mirror works with Jira Server and Data Center")
}

/// K207 (FORMAT.md): an item whose type has `done` statuses and that
/// links Jira tickets is done while one of its tickets isn't resolved, or
/// every ticket is resolved while the item is neither done nor closed.
/// Tickets the mirror doesn't have are left out of both.
pub fn k207(project: &Project, mirror: &Mirror) -> Vec<Finding> {
    let mut out = Vec::new();
    for item in &project.items {
        let Some(ty) = project.config.type_named(&item.type_name) else {
            continue;
        };
        if ty.done.is_empty() || item.jira.is_empty() {
            continue;
        }
        let tickets: Vec<&Ticket> = item.jira.iter().filter_map(|k| mirror.ticket(k)).collect();
        if tickets.is_empty() {
            continue;
        }
        let done = ty.is_done(&item.status);
        let open: Vec<String> = tickets
            .iter()
            .filter(|t| !t.resolved)
            .map(|t| format!("{} ({})", t.key, t.status))
            .collect();
        let message = if done && !open.is_empty() {
            format!(
                "{} is {} but its Jira ticket isn't resolved: {}",
                item.id,
                item.status,
                open.join(", ")
            )
        } else if !done && !ty.is_closed(&item.status) && open.is_empty() {
            let keys: Vec<&str> = tickets.iter().map(|t| t.key.as_str()).collect();
            format!(
                "{} is {} but its Jira ticket is resolved: {}; set it to {}",
                item.id,
                item.status,
                keys.join(", "),
                ty.done.join(" or ")
            )
        } else {
            continue;
        };
        // Where Kiln manages the tickets, say how to bring them together.
        let message = match &project.config.jira_sync {
            Some(sync) if sync.types.contains(&item.type_name) => format!(
                "{message} (`kiln jira sync` moves the ticket; `kiln jira pull {}` takes its status)",
                item.id
            ),
            _ => message,
        };
        out.push(Finding::new(
            Rule::K207,
            &item.path,
            item.status_span,
            message,
        ));
    }
    out
}

/// Reads `keys` from the Jira at `url` with `token`. Keys Jira doesn't
/// know end up in [`Mirror::missing`]: a `key IN (…)` search fails
/// outright when one key doesn't exist, so a failing chunk is asked again
/// key by key. `now` is the time the mirror is stamped with.
pub fn fetch(url: &str, token: &str, keys: &[String], now: u64) -> Result<Mirror, String> {
    if is_cloud(url) {
        return Err(cloud_refusal(url));
    }
    let url = url.trim_end_matches('/');
    let client = fenix_jira::JiraClient::new(url, token);
    let mut mirror = Mirror {
        url: url.to_owned(),
        fetched: now,
        ..Mirror::default()
    };
    for chunk in keys.chunks(CHUNK) {
        let jql = fenix_jira::build_keys_jql(chunk);
        match client.search_issues(&jql, chunk.len() as u32) {
            Ok(found) => {
                for issue in found {
                    mirror.tickets.insert(issue.key.clone(), ticket(issue));
                }
                for k in chunk {
                    if !mirror.tickets.contains_key(k) {
                        mirror.missing.insert(k.clone());
                    }
                }
            }
            // A bad key is a 400; anything else (401, no network) would
            // fail key by key too, so say it once.
            Err(e) if !e.starts_with("HTTP 400") => return Err(jira_error(url, &e)),
            Err(_) => {
                for k in chunk {
                    match client
                        .search_issues(&fenix_jira::build_keys_jql(std::slice::from_ref(k)), 1)
                    {
                        Ok(found) => match found.into_iter().next() {
                            Some(issue) => {
                                mirror.tickets.insert(issue.key.clone(), ticket(issue));
                            }
                            None => {
                                mirror.missing.insert(k.clone());
                            }
                        },
                        Err(e) if e.starts_with("HTTP 400") => {
                            mirror.missing.insert(k.clone());
                        }
                        Err(e) => return Err(jira_error(url, &e)),
                    }
                }
            }
        }
    }
    Ok(mirror)
}

/// Whether `token` can search the Jira at `url`: one search for the most
/// recently updated issue it can see (`kiln auth jira`).
pub fn sign_in(url: &str, token: &str) -> Result<(), String> {
    if is_cloud(url) {
        return Err(cloud_refusal(url));
    }
    let client = fenix_jira::JiraClient::new(url.trim_end_matches('/'), token);
    client
        .search_issues("order by updated DESC", 1)
        .map(|_| ())
        .map_err(|e| jira_error(url, &e))
}

fn jira_error(url: &str, e: &str) -> String {
    let hint = if e.starts_with("HTTP 401") || e.starts_with("HTTP 403") {
        " (check the token: KILN_JIRA_TOKEN or `kiln auth jira`)"
    } else {
        ""
    };
    format!("Jira at {}: {e}{hint}", url.trim_end_matches('/'))
}

fn ticket(issue: fenix_jira::IssueSummary) -> Ticket {
    Ticket {
        resolved: issue.status_category == "done",
        key: issue.key,
        summary: issue.summary,
        status: issue.status,
        assignee: issue.assignee,
    }
}

pub mod push;
pub mod sync;

/// The fake server, for this crate's tests and, with the `fake` feature,
/// the binary's.
#[cfg(any(test, feature = "fake"))]
pub mod fake;

#[cfg(test)]
mod tests;
