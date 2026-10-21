//! An in-process fake Jira Data Center for tests, over real HTTP on a
//! local port, so `fenix-jira`'s real client is what's tested:
//!
//! - search (`key IN (…)`, `key = X`, `parent = X`, anything else lists
//!   every issue); like Jira, a `key IN` search naming a key that doesn't
//!   exist is a 400;
//! - create-meta issue types (`Task`, `Sub-task`) and fields;
//! - creating an issue (with `parent` for a sub-task), updating summary,
//!   description and due date;
//! - transitions over a small workflow: To Do → In Progress → Done, In
//!   Progress ⇄ Blocked, Done → In Progress ("Reopen") only;
//! - a wrong token is a 401.
//!
//! Test code: it unwraps where a test should stop loudly.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

/// The token the fake accepts.
pub const TOKEN: &str = "test-token";

/// One issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Issue {
    pub summary: String,
    pub status: String,
    /// `new`, `indeterminate` or `done`.
    pub category: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub issue_type: String,
    pub description: String,
}

#[derive(Default)]
pub struct State {
    pub issues: BTreeMap<String, Issue>,
    /// Every request served, as `METHOD path?query`, in order.
    pub log: Vec<String>,
    /// Required create fields Kiln doesn't set, per issue type name.
    pub required: BTreeMap<String, Vec<String>>,
    /// The next issue number, for `DEMO-<n>`.
    pub next: u32,
}

impl State {
    pub fn issue(
        mut self,
        key: &str,
        status: &str,
        category: &str,
        assignee: Option<&str>,
    ) -> State {
        self.issues.insert(
            key.to_owned(),
            Issue {
                summary: format!("{key} summary"),
                status: status.to_owned(),
                category: category.to_owned(),
                assignee: assignee.map(str::to_owned),
                issue_type: "Task".into(),
                ..Issue::default()
            },
        );
        self
    }

    /// A sub-task of `parent`, in `status`.
    pub fn subtask(mut self, key: &str, parent: &str, summary: &str, status: &str) -> State {
        self.issues.insert(
            key.to_owned(),
            Issue {
                summary: summary.to_owned(),
                status: status.to_owned(),
                category: category(status).to_owned(),
                parent: Some(parent.to_owned()),
                issue_type: "Sub-task".into(),
                ..Issue::default()
            },
        );
        self
    }

    /// Requests other than GETs.
    pub fn writes(&self) -> Vec<String> {
        self.log
            .iter()
            .filter(|l| !l.starts_with("GET "))
            .cloned()
            .collect()
    }
}

/// The fake workflow's status categories.
fn category(status: &str) -> &'static str {
    match status {
        "To Do" => "new",
        "Done" => "done",
        _ => "indeterminate",
    }
}

/// The fake workflow: (transition id, name, from statuses, to status).
const WORKFLOW: &[(&str, &str, &[&str], &str)] = &[
    ("11", "Start", &["To Do"], "In Progress"),
    ("21", "Block", &["In Progress"], "Blocked"),
    ("31", "Unblock", &["Blocked"], "In Progress"),
    ("41", "Finish", &["To Do", "In Progress"], "Done"),
    ("51", "Reopen", &["Done"], "In Progress"),
];

pub struct Fake {
    pub url: String,
    pub state: Arc<Mutex<State>>,
}

impl Fake {
    /// Serves `state` on a free local port.
    pub fn start(state: State) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(state));
        let shared = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let state = shared.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut words = line.split_whitespace();
                    let method = words.next().unwrap_or("GET").to_owned();
                    let target = words.next().unwrap_or("/").to_owned();
                    let (mut auth, mut length) = (String::new(), 0usize);
                    loop {
                        let mut header = String::new();
                        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                            break;
                        }
                        if let Some((k, v)) = header.split_once(':') {
                            if k.eq_ignore_ascii_case("authorization") {
                                auth = v.trim().to_owned();
                            } else if k.eq_ignore_ascii_case("content-length") {
                                length = v.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                    let mut body = vec![0; length];
                    if length > 0 && reader.read_exact(&mut body).is_err() {
                        return;
                    }
                    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                    let (code, reply) = respond(&state, &method, &target, &auth, &body);
                    let text = format!(
                        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                        reply.len()
                    );
                    let _ = stream.write_all(text.as_bytes());
                });
            }
        });
        Fake {
            url: format!("http://127.0.0.1:{port}"),
            state,
        }
    }

    /// Requests served so far.
    pub fn requests(&self) -> usize {
        self.state.lock().unwrap().log.len()
    }
}

fn error(code: u16, message: &str) -> (u16, String) {
    (code, json!({ "errorMessages": [message] }).to_string())
}

fn issue_json(key: &str, i: &Issue) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": i.summary,
            "status": { "name": i.status, "statusCategory": { "key": i.category } },
            "assignee": i.assignee.as_ref().map(|a| json!({ "displayName": a })),
            "updated": "2026-10-12T10:00:00.000+0000",
            "duedate": i.due,
            "issuetype": { "name": i.issue_type },
            "parent": i.parent.as_ref().map(|p| json!({ "key": p })),
        }
    })
}

fn respond(
    state: &Mutex<State>,
    method: &str,
    target: &str,
    auth: &str,
    body: &Value,
) -> (u16, String) {
    let mut state = state.lock().unwrap();
    state.log.push(format!("{method} {target}"));
    if auth != format!("Bearer {TOKEN}") {
        return error(401, "unauthorized");
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let params: BTreeMap<String, String> = query
        .split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect();
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (method, parts.as_slice()) {
        ("GET", ["rest", "api", "2", "search"]) => search(&state, &params),
        ("GET", ["rest", "api", "2", "issue", "createmeta", _, "issuetypes"]) => (
            200,
            json!({ "values": [
                { "id": "3", "name": "Task", "subtask": false },
                { "id": "5", "name": "Sub-task", "subtask": true },
            ]})
            .to_string(),
        ),
        ("GET", ["rest", "api", "2", "issue", "createmeta", _, "issuetypes", type_id]) => {
            let name = if *type_id == "5" { "Sub-task" } else { "Task" };
            let mut fields = vec![
                json!({ "fieldId": "summary", "name": "Summary", "required": true, "schema": { "type": "string" } }),
                json!({ "fieldId": "project", "name": "Project", "required": true, "schema": { "type": "project" } }),
            ];
            for f in state.required.get(name).cloned().unwrap_or_default() {
                fields.push(json!({ "fieldId": f, "name": f, "required": true, "schema": { "type": "string" } }));
            }
            (200, json!({ "values": fields }).to_string())
        }
        ("POST", ["rest", "api", "2", "issue"]) => {
            let fields = &body["fields"];
            let project = fields
                .pointer("/project/key")
                .and_then(Value::as_str)
                .unwrap_or("X");
            let issue_type = match fields.pointer("/issuetype/id").and_then(Value::as_str) {
                Some("5") => "Sub-task",
                Some(_) => "Task",
                None => fields
                    .pointer("/issuetype/name")
                    .and_then(Value::as_str)
                    .unwrap_or("Task"),
            };
            let parent = fields
                .pointer("/parent/key")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if issue_type == "Sub-task" {
                match &parent {
                    Some(p) if state.issues.contains_key(p) => {}
                    _ => return error(400, "A sub-task needs an existing parent"),
                }
            }
            for f in state.required.get(issue_type).cloned().unwrap_or_default() {
                if fields.get(&f).is_none() {
                    return error(400, &format!("{f} is required"));
                }
            }
            state.next = state.next.max(100) + 1;
            let key = format!("{project}-{}", state.next);
            let issue = Issue {
                summary: fields["summary"].as_str().unwrap_or("").to_owned(),
                status: "To Do".into(),
                category: "new".into(),
                due: fields["duedate"].as_str().map(str::to_owned),
                parent,
                issue_type: issue_type.into(),
                description: fields["description"].as_str().unwrap_or("").to_owned(),
                ..Issue::default()
            };
            state.issues.insert(key.clone(), issue);
            (201, json!({ "id": "1", "key": key }).to_string())
        }
        ("PUT", ["rest", "api", "2", "issue", key]) => {
            let Some(issue) = state.issues.get_mut(*key) else {
                return error(404, "Issue Does Not Exist");
            };
            let fields = &body["fields"];
            if let Some(s) = fields.get("summary").and_then(Value::as_str) {
                issue.summary = s.to_owned();
            }
            if let Some(d) = fields.get("description").and_then(Value::as_str) {
                issue.description = d.to_owned();
            }
            if let Some(due) = fields.get("duedate") {
                issue.due = due.as_str().map(str::to_owned);
            }
            (204, String::new())
        }
        ("GET", ["rest", "api", "2", "issue", key, "transitions"]) => {
            let Some(issue) = state.issues.get(*key) else {
                return error(404, "Issue Does Not Exist");
            };
            let list: Vec<Value> = WORKFLOW
                .iter()
                .filter(|(_, _, from, _)| from.contains(&issue.status.as_str()))
                .map(|(id, name, _, to)| {
                    json!({ "id": id, "name": name, "to": { "id": to, "name": to, "statusCategory": { "key": category(to) } } })
                })
                .collect();
            (200, json!({ "transitions": list }).to_string())
        }
        ("POST", ["rest", "api", "2", "issue", key, "transitions"]) => {
            let id = body
                .pointer("/transition/id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let Some(issue) = state.issues.get_mut(*key) else {
                return error(404, "Issue Does Not Exist");
            };
            match WORKFLOW
                .iter()
                .find(|(t, _, from, _)| *t == id && from.contains(&issue.status.as_str()))
            {
                Some((_, _, _, to)) => {
                    issue.status = (*to).to_owned();
                    issue.category = category(to).to_owned();
                    (204, String::new())
                }
                None => error(400, "It seems that you have tried to perform a workflow operation that is not valid"),
            }
        }
        _ => (404, "{}".into()),
    }
}

fn search(state: &State, params: &BTreeMap<String, String>) -> (u16, String) {
    let jql = params.get("jql").cloned().unwrap_or_default();
    let max: usize = params
        .get("maxResults")
        .and_then(|m| m.parse().ok())
        .unwrap_or(50);
    let unquote = |s: &str| s.trim().trim_matches('"').to_owned();
    let keys: Vec<String> = if let Some(list) = jql
        .strip_prefix("key IN (")
        .and_then(|r| r.strip_suffix(')'))
    {
        let keys: Vec<String> = list.split(',').map(unquote).collect();
        if let Some(bad) = keys.iter().find(|k| !state.issues.contains_key(*k)) {
            return error(
                400,
                &format!("An issue with key '{bad}' does not exist for field 'key'."),
            );
        }
        keys
    } else if let Some(key) = jql.strip_prefix("key = ") {
        let key = unquote(key);
        if !state.issues.contains_key(&key) {
            return error(
                400,
                &format!("An issue with key '{key}' does not exist for field 'key'."),
            );
        }
        vec![key]
    } else if let Some(parent) = jql.strip_prefix("parent = ") {
        let parent = unquote(parent.split(" ORDER BY").next().unwrap_or(""));
        state
            .issues
            .iter()
            .filter(|(_, i)| i.parent.as_deref() == Some(parent.as_str()))
            .map(|(k, _)| k.clone())
            .collect()
    } else {
        state.issues.keys().cloned().collect()
    };
    let issues: Vec<Value> = keys
        .iter()
        .take(max)
        .filter_map(|k| state.issues.get(k).map(|i| issue_json(k, i)))
        .collect();
    (200, json!({ "issues": issues }).to_string())
}

/// `application/x-www-form-urlencoded` decoding.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
