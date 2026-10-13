//! An in-process fake Jira Data Center for tests: the search endpoint
//! (`key IN (…)` and any other JQL) over real HTTP on a local port, so
//! `fenix-jira`'s real client is what's tested. Like Jira, a `key IN`
//! search naming a key that doesn't exist is a 400; a wrong token is a
//! 401.
//!
//! Test code: it unwraps where a test should stop loudly.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

/// The token the fake accepts.
pub const TOKEN: &str = "test-token";

/// One issue: (summary, status, status category key, assignee).
pub type Issue = (String, String, String, Option<String>);

#[derive(Default)]
pub struct State {
    pub issues: BTreeMap<String, Issue>,
    /// Every request served, as `METHOD path?query`, in order.
    pub log: Vec<String>,
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
            (
                format!("{key} summary"),
                status.to_owned(),
                category.to_owned(),
                assignee.map(str::to_owned),
            ),
        );
        self
    }
}

pub struct Fake {
    pub url: String,
    pub state: Arc<Mutex<State>>,
}

impl Fake {
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
                    let mut auth = String::new();
                    loop {
                        let mut header = String::new();
                        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                            break;
                        }
                        if let Some((k, v)) = header.split_once(':') {
                            if k.eq_ignore_ascii_case("authorization") {
                                auth = v.trim().to_owned();
                            }
                        }
                    }
                    let (code, reply) = respond(&state, &method, &target, &auth);
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

fn respond(state: &Mutex<State>, method: &str, target: &str, auth: &str) -> (u16, String) {
    let mut state = state.lock().unwrap();
    state.log.push(format!("{method} {target}"));
    if auth != format!("Bearer {TOKEN}") {
        return (
            401,
            json!({ "errorMessages": ["unauthorized"] }).to_string(),
        );
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if method != "GET" || path != "/rest/api/2/search" {
        return (404, "{}".into());
    }
    let params: BTreeMap<String, String> = query
        .split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect();
    let jql = params.get("jql").cloned().unwrap_or_default();
    let max: usize = params
        .get("maxResults")
        .and_then(|m| m.parse().ok())
        .unwrap_or(50);
    let keys: Vec<String> = match jql
        .strip_prefix("key IN (")
        .and_then(|r| r.strip_suffix(')'))
    {
        Some(list) => {
            let keys: Vec<String> = list
                .split(',')
                .map(|k| k.trim().trim_matches('"').to_owned())
                .collect();
            if let Some(bad) = keys.iter().find(|k| !state.issues.contains_key(*k)) {
                return (
                    400,
                    json!({ "errorMessages": [format!("An issue with key '{bad}' does not exist for field 'key'.")] })
                        .to_string(),
                );
            }
            keys
        }
        None => state.issues.keys().cloned().collect(),
    };
    let issues: Vec<Value> = keys
        .iter()
        .take(max)
        .filter_map(|k| state.issues.get(k).map(|i| (k, i)))
        .map(|(k, (summary, status, category, assignee))| {
            json!({
                "key": k,
                "fields": {
                    "summary": summary,
                    "status": { "name": status, "statusCategory": { "key": category } },
                    "assignee": assignee.as_ref().map(|a| json!({ "displayName": a })),
                    "updated": "2026-10-12T10:00:00.000+0000",
                }
            })
        })
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
