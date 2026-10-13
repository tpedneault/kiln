//! An in-process fake Confluence for tests (DESIGN.md, "Testing"): the
//! read half of the calls in CONFLUENCE.md's table, in both flavours,
//! over real HTTP on a local port, so the real client is what's tested.
//! It checks the `Authorization` header, pages its listings, and can
//! answer 429 once to exercise retries. The write half arrives with K3.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct FakePage {
    pub title: String,
    pub parent: Option<u64>,
    pub version: u64,
    pub author: String,
    pub body: String,
    pub property: Option<Value>,
}

#[derive(Default)]
pub struct State {
    pub pages: BTreeMap<u64, FakePage>,
    /// Paths answered 429 (with `Retry-After: 0`) once.
    pub rate_limit_once: Vec<String>,
    /// Every request path served, in order.
    pub log: Vec<String>,
}

pub struct Fake {
    pub url: String,
    pub state: Arc<Mutex<State>>,
}

/// The expected `Authorization` header for tests.
pub const BEARER: &str = "Bearer test-token";

impl Fake {
    /// Serves `state` on a free local port; `cloud` picks the flavour's
    /// paths (Cloud under `/wiki`).
    pub fn start(state: State, cloud: bool) -> Fake {
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
                    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
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
                    let (code, extra, body) = respond(&state, &path, &auth, cloud);
                    let reply = format!(
                        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(reply.as_bytes());
                });
            }
        });
        let url = if cloud {
            format!("http://127.0.0.1:{port}/wiki")
        } else {
            format!("http://127.0.0.1:{port}")
        };
        Fake { url, state }
    }
}

fn respond(state: &Mutex<State>, full: &str, auth: &str, cloud: bool) -> (u16, String, String) {
    let mut s = state.lock().unwrap();
    s.log.push(full.to_owned());
    if auth != BEARER {
        return (401, String::new(), "{\"message\":\"Unauthorized\"}".into());
    }
    if let Some(i) = s
        .rate_limit_once
        .iter()
        .position(|p| full.starts_with(p.as_str()))
    {
        s.rate_limit_once.remove(i);
        return (429, "Retry-After: 0\r\n".into(), "{}".into());
    }
    let (path, query) = full.split_once('?').unwrap_or((full, ""));
    let param = |k: &str| {
        query
            .split('&')
            .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
            .map(decode)
    };
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let ok = |v: Value| (200, String::new(), v.to_string());
    let missing = (404, String::new(), "{\"message\":\"not found\"}".to_owned());
    let pages = &s.pages;
    let start: usize = param("start").and_then(|v| v.parse().ok()).unwrap_or(0);
    let ancestors = |id: u64| {
        let mut chain = Vec::new();
        let mut at = pages.get(&id).and_then(|p| p.parent);
        while let Some(a) = at {
            chain.insert(0, json!({ "id": a.to_string() }));
            at = pages.get(&a).and_then(|p| p.parent);
        }
        chain
    };
    let dc_page = |id: u64, p: &FakePage| {
        json!({
            "id": id.to_string(), "type": "page", "title": p.title,
            "version": { "number": p.version, "by": { "displayName": p.author }, "when": "2026-10-10T14:02:00.000Z" },
            "ancestors": ancestors(id),
            "body": { "storage": { "value": p.body, "representation": "storage" } },
        })
    };
    let cloud_page = |id: u64, p: &FakePage| {
        json!({
            "id": id.to_string(), "title": p.title, "parentId": p.parent.map(|x| x.to_string()),
            "version": { "number": p.version, "authorId": p.author, "createdAt": "2026-10-10T14:02:00.000Z" },
            "body": { "storage": { "value": p.body, "representation": "storage" } },
        })
    };
    let descendants = |root: u64| {
        let mut out = Vec::new();
        let mut queue = vec![root];
        while let Some(id) = queue.pop() {
            for (child, p) in pages.iter().filter(|(_, p)| p.parent == Some(id)) {
                out.push((*child, p));
                queue.push(*child);
            }
        }
        out.sort_by_key(|(id, _)| *id);
        out
    };
    // Pages of 2, so paging is exercised.
    let paged = |items: Vec<Value>, next_base: String| {
        let end = (start + 2).min(items.len());
        let mut v = json!({ "results": items[start.min(items.len())..end].to_vec() });
        if end < items.len() {
            v["_links"] = json!({ "next": format!("{next_base}start={end}") });
        }
        v
    };
    if !cloud {
        match parts.as_slice() {
            ["rest", "api", "user", "current"] => ok(json!({ "displayName": "Kiln Test" })),
            ["rest", "api", "content"] => {
                let title = param("title").unwrap_or_default();
                let results: Vec<Value> = pages
                    .iter()
                    .filter(|(_, p)| p.title == title)
                    .map(|(id, p)| dc_page(*id, p))
                    .collect();
                ok(json!({ "results": results }))
            }
            ["rest", "api", "content", id] => match id
                .parse()
                .ok()
                .and_then(|id: u64| pages.get(&id).map(|p| (id, p)))
            {
                Some((id, p)) => ok(dc_page(id, p)),
                None => missing,
            },
            ["rest", "api", "content", id, "child", "page"] => {
                let id: u64 = id.parse().unwrap_or(0);
                let items = pages
                    .iter()
                    .filter(|(_, p)| p.parent == Some(id))
                    .map(|(c, p)| dc_page(*c, p))
                    .collect();
                ok(paged(
                    items,
                    format!("/rest/api/content/{id}/child/page?expand=version,ancestors&limit=2&"),
                ))
            }
            ["rest", "api", "content", id, "descendant", "page"] => {
                let id: u64 = id.parse().unwrap_or(0);
                let items = descendants(id)
                    .into_iter()
                    .map(|(c, p)| dc_page(c, p))
                    .collect();
                ok(paged(
                    items,
                    format!(
                        "/rest/api/content/{id}/descendant/page?expand=version,ancestors&limit=2&"
                    ),
                ))
            }
            ["rest", "api", "content", id, "property", "kiln"] => {
                match id
                    .parse()
                    .ok()
                    .and_then(|id: u64| pages.get(&id))
                    .and_then(|p| p.property.clone())
                {
                    Some(value) => {
                        ok(json!({ "key": "kiln", "value": value, "version": { "number": 1 } }))
                    }
                    None => missing,
                }
            }
            _ => missing,
        }
    } else {
        match parts.as_slice() {
            ["wiki", "rest", "api", "user", "current"] => ok(json!({ "displayName": "Kiln Test" })),
            ["wiki", "api", "v2", "spaces"] => {
                ok(json!({ "results": [{ "id": "777", "key": param("keys") }] }))
            }
            ["wiki", "api", "v2", "pages"] => {
                let title = param("title").unwrap_or_default();
                let results: Vec<Value> = pages
                    .iter()
                    .filter(|(_, p)| p.title == title)
                    .map(|(id, p)| cloud_page(*id, p))
                    .collect();
                ok(json!({ "results": results }))
            }
            ["wiki", "api", "v2", "pages", id] => match id
                .parse()
                .ok()
                .and_then(|id: u64| pages.get(&id).map(|p| (id, p)))
            {
                Some((id, p)) => ok(cloud_page(id, p)),
                None => missing,
            },
            ["wiki", "api", "v2", "pages", id, "children"] => {
                let id: u64 = id.parse().unwrap_or(0);
                let items = pages
                    .iter()
                    .filter(|(_, p)| p.parent == Some(id))
                    .map(|(c, p)| json!({ "id": c.to_string(), "title": p.title }))
                    .collect();
                ok(paged(
                    items,
                    format!("/wiki/api/v2/pages/{id}/children?limit=2&"),
                ))
            }
            ["wiki", "api", "v2", "pages", id, "descendants"] => {
                let id: u64 = id.parse().unwrap_or(0);
                let items = descendants(id)
                    .into_iter()
                    .map(|(c, p)| json!({ "id": c.to_string(), "title": p.title, "parentId": p.parent.map(|x| x.to_string()) }))
                    .collect();
                ok(paged(
                    items,
                    format!("/wiki/api/v2/pages/{id}/descendants?limit=2&"),
                ))
            }
            ["wiki", "api", "v2", "pages", id, "properties"] => {
                let results: Vec<Value> = id
                    .parse()
                    .ok()
                    .and_then(|id: u64| pages.get(&id))
                    .and_then(|p| p.property.clone())
                    .map(|v| vec![json!({ "id": "1", "key": "kiln", "value": v, "version": { "number": 1 } })])
                    .unwrap_or_default();
                ok(json!({ "results": results }))
            }
            _ => missing,
        }
    }
}

/// Percent-decoding for query values.
fn decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(n) = u8::from_str_radix(&v[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
