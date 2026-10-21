//! The sync's I/O (INTEGRATION.md, "Jira"): reading what [`crate::sync`]
//! plans against, and applying a plan, through `fenix-jira`. Jira Server
//! and Data Center only, like the mirror (G6).
//!
//! Applying stops at the first error and returns what it did so far, the
//! keys it created above all: the binary writes those into the items
//! before reporting the error, so a rerun never creates a ticket twice.
//! Every op is idempotent against the next plan, which picks up from
//! wherever a run stopped.

use fenix_jira::{JiraClient, NewIssue};
use kiln_index::Project;
use kiln_model::{ItemId, JiraSync};

use crate::sync::{needs_moving, Move, Op, Plan, Remote, RemoteTicket, Target};
use crate::{cloud_refusal, is_cloud};

/// Most sub-tasks one project issue is read with.
const MAX: u32 = 1000;

fn client(url: &str, token: &str) -> Result<JiraClient, String> {
    if is_cloud(url) {
        return Err(cloud_refusal(url));
    }
    Ok(JiraClient::new(url.trim_end_matches('/'), token))
}

fn ticket(i: fenix_jira::IssueSummary) -> RemoteTicket {
    RemoteTicket {
        key: i.key,
        summary: i.summary,
        status: i.status,
        category: i.status_category,
        due: i.due,
    }
}

fn moves(client: &JiraClient, key: &str) -> Result<Vec<Move>, String> {
    Ok(client
        .list_transitions(key)
        .map_err(|e| format!("Jira: the transitions of {key}: {e}"))?
        .into_iter()
        .map(|t| Move {
            id: t.id,
            name: t.name,
            to: t.to_name,
            to_category: t.to_category,
        })
        .collect())
}

/// Reads the project issue, its sub-tasks, the issue types and, for the
/// sub-tasks that must move, their transitions. Writes nothing.
pub fn read(url: &str, token: &str, project: &Project, sync: &JiraSync) -> Result<Remote, String> {
    let client = client(url, token)?;
    let jira = |e: String| format!("Jira ({}): {e}", url.trim_end_matches('/'));
    let types = client
        .issue_types(&sync.project)
        .map_err(|e| jira(format!("the issue types of project {}: {e}", sync.project)))?;
    let subtask = match &sync.subtask_type {
        Some(name) => types.iter().find(|t| t.name.eq_ignore_ascii_case(name)),
        None => types.iter().find(|t| t.subtask),
    }
    .ok_or_else(|| {
        let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
        jira(format!(
            "project {} has no {} issue type ({}); set [jira] subtask_type",
            sync.project,
            sync.subtask_type.as_deref().unwrap_or("sub-task"),
            names.join(", ")
        ))
    })?;
    let mut remote = Remote {
        subtask_type: (subtask.id.clone(), subtask.name.clone()),
        ..Remote::default()
    };
    let mut check = vec![(subtask.id.clone(), subtask.name.clone())];
    if sync.parent.is_none() {
        let parent = types
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&sync.parent_type) && !t.subtask)
            .ok_or_else(|| {
                jira(format!(
                    "project {} has no {} issue type for the project issue; set [jira] parent_type",
                    sync.project, sync.parent_type
                ))
            })?;
        remote.parent_type = Some((parent.id.clone(), parent.name.clone()));
        check.push((parent.id.clone(), parent.name.clone()));
    }
    for (id, name) in check {
        // A server that won't say leaves the check to the create itself.
        if let Ok(fields) = client.create_fields(&sync.project, &id) {
            let asked: Vec<String> = fields
                .iter()
                .filter(|f| f.asked() && f.id != "parent")
                .map(|f| f.name.clone())
                .collect();
            if !asked.is_empty() {
                remote.missing_fields.push((name, asked));
            }
        }
    }
    if let Some(parent) = &sync.parent {
        let found = client
            .search_issues(&format!("key = {parent}"), 1)
            .map_err(|e| {
                jira(format!(
                    "[jira] parent {parent} doesn't exist or the token can't see it ({e})"
                ))
            })?;
        remote.parent = found.into_iter().next().map(ticket);
        if remote.parent.is_none() {
            return Err(jira(format!(
                "[jira] parent {parent} doesn't exist or the token can't see it"
            )));
        }
        remote.subtasks = client
            .search_issues(&format!("parent = {parent} ORDER BY key"), MAX)
            .map_err(|e| jira(format!("the sub-tasks of {parent}: {e}")))?
            .into_iter()
            .map(ticket)
            .collect();
    }
    for key in needs_moving(project, sync, &remote) {
        let m = moves(&client, &key).map_err(jira)?;
        remote.moves.insert(key, m);
    }
    Ok(remote)
}

/// What applying a plan did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// The project issue, when this run created it.
    pub parent: Option<String>,
    /// The sub-tasks created, by item.
    pub created: Vec<(ItemId, String)>,
    pub updated: usize,
    pub moved: usize,
    /// Moves that couldn't be made: a new ticket no transition takes to
    /// its item's status.
    pub refused: Vec<String>,
    /// Why it stopped early, when it did.
    pub error: Option<String>,
}

/// Applies `plan` in order: the project issue, creates, updates, moves.
pub fn apply(url: &str, token: &str, sync: &JiraSync, remote: &Remote, plan: &Plan) -> Applied {
    let mut done = Applied::default();
    if let Err(e) = apply_into(url, token, sync, remote, plan, &mut done) {
        done.error = Some(format!("Jira ({}): {e}", url.trim_end_matches('/')));
    }
    done
}

fn apply_into(
    url: &str,
    token: &str,
    sync: &JiraSync,
    remote: &Remote,
    plan: &Plan,
    done: &mut Applied,
) -> Result<(), String> {
    let client = client(url, token)?;
    let mut parent = sync.parent.clone();
    for op in &plan.ops {
        if let Op::CreateParent { summary } = op {
            let (type_id, _) = remote
                .parent_type
                .clone()
                .ok_or("the project issue's type wasn't read")?;
            let key = client
                .create(&NewIssue {
                    project: sync.project.clone(),
                    issue_type_id: type_id,
                    summary: summary.clone(),
                    description: "Kiln's tasks are this issue's sub-tasks. Kiln creates them and keeps their summary, due date and status in step with the repository (kiln jira sync); edit the items there, not the sub-tasks here.".into(),
                    ..NewIssue::default()
                })
                .map_err(|e| format!("creating the project issue: {e}"))?;
            done.parent = Some(key.clone());
            parent = Some(key);
        }
    }
    for op in &plan.ops {
        match op {
            Op::Create {
                item,
                summary,
                due,
                description,
                target,
            } => {
                let under = parent.clone().ok_or("no project issue to create under")?;
                let key = client
                    .create(&NewIssue {
                        project: sync.project.clone(),
                        issue_type_id: remote.subtask_type.0.clone(),
                        summary: summary.clone(),
                        description: description.clone(),
                        extra: vec![("parent".into(), serde_json::json!({ "key": under }))],
                        ..NewIssue::default()
                    })
                    .map_err(|e| format!("creating {item}'s sub-task: {e}"))?;
                done.created.push((item.clone(), key.clone()));
                // Set after creating: a due date isn't on every create
                // screen, and the ticket exists (and is recorded) either way.
                if let Some(due) = due {
                    client
                        .update_due(&key, Some(due))
                        .map_err(|e| format!("{key}: the due date: {e}"))?;
                }
                if let Some(target) = target {
                    move_new(&client, &key, target, done)?;
                }
            }
            Op::Update {
                key, summary, due, ..
            } => {
                if let Some(s) = summary {
                    client
                        .update_summary(key, s)
                        .map_err(|e| format!("{key}: the summary: {e}"))?;
                }
                if let Some(d) = due {
                    client
                        .update_due(key, d.as_deref())
                        .map_err(|e| format!("{key}: the due date: {e}"))?;
                }
                done.updated += 1;
            }
            Op::Transition { key, id, to, .. } => {
                client
                    .apply_transition(key, id)
                    .map_err(|e| format!("{key}: moving to {to}: {e}"))?;
                done.moved += 1;
            }
            _ => {}
        }
    }
    Ok(())
}

/// A new ticket starts in its workflow's first status; this moves it on
/// when its item is further along, one transition at a time, at most
/// three (To Do → In Progress → Blocked).
fn move_new(
    client: &JiraClient,
    key: &str,
    target: &Target,
    done: &mut Applied,
) -> Result<(), String> {
    for _ in 0..3 {
        let options = moves(client, key)?;
        if let Some(m) = options
            .iter()
            .find(|m| target.reached_by(&m.to, &m.to_category))
        {
            client
                .apply_transition(key, &m.id)
                .map_err(|e| format!("{key}: moving to {}: {e}", m.to))?;
            done.moved += 1;
            return Ok(());
        }
        // One step towards it: an in-progress status, the usual way on.
        match options.iter().find(|m| m.to_category == "indeterminate") {
            Some(m) => {
                client
                    .apply_transition(key, &m.id)
                    .map_err(|e| format!("{key}: moving to {}: {e}", m.to))?;
            }
            None => break,
        }
    }
    done.refused.push(format!(
        "{key}: no transition reaches {}; it was left where the workflow put it",
        target.describe()
    ));
    Ok(())
}
