//! An in-process fake Confluence for tests (DESIGN.md, "Testing"): every
//! call in CONFLUENCE.md's table, in both flavours, over real HTTP on a
//! local port, so the real client is what's tested. It keeps versions
//! (an update must name the next one, else 409), properties with their
//! own versions, labels, attachments (multipart uploads), restrictions and
//! unique titles per space; it checks the `Authorization` header, pages
//! its listings, and can answer 429 once, 409 once, 413 past a size, or
//! fail every write after a number of them (an interrupted publish).
//!
//! Test code: it unwraps where a test should stop loudly.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
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
    /// The `kiln` property's value.
    pub property: Option<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct Extra {
    pub property_version: u64,
    pub labels: Vec<String>,
    /// (attachment id, file name, bytes)
    pub attachments: Vec<(String, String, Vec<u8>)>,
    pub restricted_to: Option<String>,
}

#[derive(Default)]
pub struct State {
    pub pages: BTreeMap<u64, FakePage>,
    pub extra: BTreeMap<u64, Extra>,
    /// Paths answered 429 (with `Retry-After: 0`) once.
    pub rate_limit_once: Vec<String>,
    /// Pages whose next update is answered 409 once.
    pub conflict_once: Vec<u64>,
    /// Bodies larger than this are answered 413.
    pub max_body: Option<usize>,
    /// Writes allowed before every further write fails with 500.
    pub writes_left: Option<usize>,
    /// Every request served, as `METHOD path`, in order.
    pub log: Vec<String>,
    pub next_id: u64,
}

impl State {
    /// A page as a person would edit it: new body, next version, by
    /// someone else.
    pub fn edit(&mut self, id: u64, body: &str) {
        if let Some(p) = self.pages.get_mut(&id) {
            p.body = body.to_owned();
            p.version += 1;
            p.author = "alice".into();
        }
    }

    /// Requests other than GETs.
    pub fn writes(&self) -> usize {
        self.log.iter().filter(|l| !l.starts_with("GET ")).count()
    }

    fn id(&mut self) -> u64 {
        self.next_id = self.next_id.max(1000) + 1;
        self.next_id
    }
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
                    let mut words = line.split_whitespace();
                    let method = words.next().unwrap_or("GET").to_owned();
                    let path = words.next().unwrap_or("/").to_owned();
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
                    let (code, extra, reply) = respond(&state, &method, &path, &auth, &body, cloud);
                    let text = format!(
                        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{reply}",
                        reply.len()
                    );
                    let _ = stream.write_all(text.as_bytes());
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

type Reply = (u16, String, String);

fn ok(v: Value) -> Reply {
    (200, String::new(), v.to_string())
}

fn status(code: u16, message: &str) -> Reply {
    (
        code,
        String::new(),
        json!({ "message": message }).to_string(),
    )
}

fn respond(
    state: &Mutex<State>,
    method: &str,
    full: &str,
    auth: &str,
    body: &[u8],
    cloud: bool,
) -> Reply {
    let mut s = state.lock().unwrap();
    s.log.push(format!("{method} {full}"));
    if auth != BEARER {
        return status(401, "Unauthorized");
    }
    if let Some(i) = s
        .rate_limit_once
        .iter()
        .position(|p| full.starts_with(p.as_str()))
    {
        s.rate_limit_once.remove(i);
        return (429, "Retry-After: 0\r\n".into(), "{}".into());
    }
    if method != "GET" {
        match &mut s.writes_left {
            Some(0) => return status(500, "the fake was told to fail from here"),
            Some(n) => *n -= 1,
            None => {}
        }
    }
    let (path, query) = full.split_once('?').unwrap_or((full, ""));
    let param = |k: &str| {
        query
            .split('&')
            .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
            .map(decode)
    };
    let start: usize = param("start").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if cloud {
        if parts.first() != Some(&"wiki") {
            return status(404, "not under /wiki");
        }
        parts.remove(0);
    }
    let json_body: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let num = |s: &str| s.parse::<u64>().unwrap_or(0);
    match (method, parts.as_slice()) {
        // Who am I (v1 on both).
        ("GET", ["rest", "api", "user", "current"]) => {
            ok(json!({ "displayName": "Kiln Test", "username": "kiln", "accountId": "acc-kiln" }))
        }
        // Data Center content.
        ("GET", ["rest", "api", "content"]) if !cloud => {
            let title = param("title").unwrap_or_default();
            let results: Vec<Value> = s
                .pages
                .iter()
                .filter(|(_, p)| p.title == title)
                .map(|(id, p)| dc_page(&s, *id, p))
                .collect();
            ok(json!({ "results": results }))
        }
        ("POST", ["rest", "api", "content"]) if !cloud => {
            let title = json_body["title"].as_str().unwrap_or("").to_owned();
            let parent = json_body
                .pointer("/ancestors/0/id")
                .and_then(Value::as_str)
                .map(num);
            let text = json_body
                .pointer("/body/storage/value")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            create(&mut s, title, parent, text, false)
        }
        ("GET", ["rest", "api", "content", id]) if !cloud => match s.pages.get(&num(id)) {
            Some(p) => ok(dc_page(&s, num(id), p)),
            None => status(404, "not found"),
        },
        ("PUT", ["rest", "api", "content", id]) if !cloud => {
            let parent = json_body
                .pointer("/ancestors/0/id")
                .and_then(Value::as_str)
                .map(num);
            update(
                &mut s,
                num(id),
                &json_body,
                json_body.pointer("/body/storage/value"),
                parent,
                false,
            )
        }
        ("DELETE", ["rest", "api", "content", id]) if !cloud => {
            let id = *id;
            if let Some(att) = id.strip_prefix("att") {
                remove_attachment(&mut s, att)
            } else if s.pages.remove(&num(id)).is_some() {
                (204, String::new(), String::new())
            } else {
                status(404, "not found")
            }
        }
        ("GET", ["rest", "api", "content", id, "child", "page"]) if !cloud => {
            let id = num(id);
            let items = s
                .pages
                .iter()
                .filter(|(_, p)| p.parent == Some(id))
                .map(|(c, p)| dc_page(&s, *c, p))
                .collect();
            ok(paged(
                items,
                start,
                format!("/rest/api/content/{id}/child/page?limit=2&"),
            ))
        }
        ("GET", ["rest", "api", "content", id, "descendant", "page"]) if !cloud => {
            let id = num(id);
            let items = descendants(&s, id)
                .into_iter()
                .map(|c| dc_page(&s, c, &s.pages[&c]))
                .collect();
            ok(paged(
                items,
                start,
                format!("/rest/api/content/{id}/descendant/page?limit=2&"),
            ))
        }
        ("GET", ["rest", "api", "content", id, "property", "kiln"]) if !cloud => {
            match s.pages.get(&num(id)).and_then(|p| p.property.clone()) {
                Some(value) => {
                    let version = s
                        .extra
                        .get(&num(id))
                        .map(|e| e.property_version)
                        .unwrap_or(1);
                    ok(json!({ "key": "kiln", "value": value, "version": { "number": version } }))
                }
                None => status(404, "not found"),
            }
        }
        ("POST", ["rest", "api", "content", id, "property"]) if !cloud => {
            put_property(&mut s, num(id), &json_body, None)
        }
        ("PUT", ["rest", "api", "content", id, "property", "kiln"]) if !cloud => {
            let v = json_body.pointer("/version/number").and_then(Value::as_u64);
            put_property(&mut s, num(id), &json_body, v)
        }
        ("GET", ["rest", "api", "content", id, "label"]) if !cloud => labels(&s, num(id)),
        ("GET", ["rest", "api", "content", id, "child", "attachment"]) if !cloud => {
            attachments(&s, num(id))
        }
        ("DELETE", ["api", "v2", "attachments", att]) if cloud => {
            remove_attachment(&mut s, att.trim_start_matches("att"))
        }
        // Both flavours: v1 label writes, uploads and restrictions.
        ("POST", ["rest", "api", "content", id, "label"]) => {
            let names: Vec<String> = json_body
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l["name"].as_str().map(str::to_owned))
                .collect();
            let e = s.extra.entry(num(id)).or_default();
            for n in names {
                if !e.labels.contains(&n) {
                    e.labels.push(n);
                }
            }
            ok(json!({}))
        }
        ("POST", ["rest", "api", "content", id, "child", "attachment"]) => {
            let (name, bytes) = parse_multipart(body);
            let att = format!("att{}", s.id());
            s.extra
                .entry(num(id))
                .or_default()
                .attachments
                .push((att.clone(), name, bytes));
            ok(json!({ "results": [{ "id": att }] }))
        }
        ("PUT", ["rest", "api", "content", id, "restriction"]) => {
            let who = json_body
                .pointer("/0/restrictions/user/0/username")
                .or_else(|| json_body.pointer("/0/restrictions/user/0/accountId"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            s.extra.entry(num(id)).or_default().restricted_to = who;
            ok(json!({}))
        }
        // Cloud v2.
        ("GET", ["api", "v2", "spaces"]) if cloud => {
            ok(json!({ "results": [{ "id": "777", "key": param("keys") }] }))
        }
        ("GET", ["api", "v2", "pages"]) if cloud => {
            let title = param("title").unwrap_or_default();
            let results: Vec<Value> = s
                .pages
                .iter()
                .filter(|(_, p)| p.title == title)
                .map(|(id, p)| cloud_page(*id, p))
                .collect();
            ok(json!({ "results": results }))
        }
        ("POST", ["api", "v2", "pages"]) if cloud => {
            let title = json_body["title"].as_str().unwrap_or("").to_owned();
            let parent = json_body["parentId"].as_str().map(num);
            let text = json_body
                .pointer("/body/value")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            create(&mut s, title, parent, text, true)
        }
        ("GET", ["api", "v2", "pages", id]) if cloud => match s.pages.get(&num(id)) {
            Some(p) => ok(cloud_page(num(id), p)),
            None => status(404, "not found"),
        },
        ("PUT", ["api", "v2", "pages", id]) if cloud => {
            let parent = json_body["parentId"].as_str().map(num);
            update(
                &mut s,
                num(id),
                &json_body,
                json_body.pointer("/body/value"),
                parent,
                true,
            )
        }
        ("DELETE", ["api", "v2", "pages", id]) if cloud => match s.pages.remove(&num(id)) {
            Some(_) => (204, String::new(), String::new()),
            None => status(404, "not found"),
        },
        ("GET", ["api", "v2", "pages", id, "children"]) if cloud => {
            let id = num(id);
            let items = s
                .pages
                .iter()
                .filter(|(_, p)| p.parent == Some(id))
                .map(|(c, p)| json!({ "id": c.to_string(), "title": p.title }))
                .collect();
            ok(paged(
                items,
                start,
                format!("/wiki/api/v2/pages/{id}/children?limit=2&"),
            ))
        }
        ("GET", ["api", "v2", "pages", id, "descendants"]) if cloud => {
            let id = num(id);
            let items = descendants(&s, id)
                .into_iter()
                .map(|c| json!({ "id": c.to_string(), "title": s.pages[&c].title, "parentId": s.pages[&c].parent.map(|x| x.to_string()) }))
                .collect();
            ok(paged(
                items,
                start,
                format!("/wiki/api/v2/pages/{id}/descendants?limit=2&"),
            ))
        }
        ("GET", ["api", "v2", "pages", id, "properties"]) if cloud => {
            let id = num(id);
            let version = s.extra.get(&id).map(|e| e.property_version).unwrap_or(1);
            let results: Vec<Value> = s
                .pages
                .get(&id)
                .and_then(|p| p.property.clone())
                .map(|v| vec![json!({ "id": "1", "key": "kiln", "value": v, "version": { "number": version } })])
                .unwrap_or_default();
            ok(json!({ "results": results }))
        }
        ("POST", ["api", "v2", "pages", id, "properties"]) if cloud => {
            put_property(&mut s, num(id), &json_body, None)
        }
        ("PUT", ["api", "v2", "pages", id, "properties", _]) if cloud => {
            let v = json_body.pointer("/version/number").and_then(Value::as_u64);
            put_property(&mut s, num(id), &json_body, v)
        }
        ("GET", ["api", "v2", "pages", id, "labels"]) if cloud => labels(&s, num(id)),
        ("GET", ["api", "v2", "pages", id, "attachments"]) if cloud => attachments(&s, num(id)),
        _ => status(404, "no such endpoint in the fake"),
    }
}

fn ancestors(s: &State, id: u64) -> Vec<Value> {
    let mut chain = Vec::new();
    let mut at = s.pages.get(&id).and_then(|p| p.parent);
    while let Some(a) = at {
        chain.insert(0, json!({ "id": a.to_string() }));
        at = s.pages.get(&a).and_then(|p| p.parent);
    }
    chain
}

fn dc_page(s: &State, id: u64, p: &FakePage) -> Value {
    json!({
        "id": id.to_string(), "type": "page", "title": p.title,
        "version": { "number": p.version, "by": { "displayName": p.author }, "when": "2026-10-10T14:02:00.000Z" },
        "ancestors": ancestors(s, id),
        "body": { "storage": { "value": p.body, "representation": "storage" } },
    })
}

fn cloud_page(id: u64, p: &FakePage) -> Value {
    json!({
        "id": id.to_string(), "title": p.title, "parentId": p.parent.map(|x| x.to_string()),
        "version": { "number": p.version, "authorId": p.author, "createdAt": "2026-10-10T14:02:00.000Z" },
        "body": { "storage": { "value": p.body, "representation": "storage" } },
    })
}

fn descendants(s: &State, root: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut queue = vec![root];
    while let Some(id) = queue.pop() {
        for (child, _) in s.pages.iter().filter(|(_, p)| p.parent == Some(id)) {
            out.push(*child);
            queue.push(*child);
        }
    }
    out.sort();
    out
}

/// Pages of 2, so paging is exercised.
fn paged(items: Vec<Value>, start: usize, next_base: String) -> Value {
    let end = (start + 2).min(items.len());
    let mut v = json!({ "results": items[start.min(items.len())..end].to_vec() });
    if end < items.len() {
        v["_links"] = json!({ "next": format!("{next_base}start={end}") });
    }
    v
}

fn create(s: &mut State, title: String, parent: Option<u64>, body: String, cloud: bool) -> Reply {
    if s.pages.values().any(|p| p.title == title) {
        return status(400, "A page with this title already exists in this space");
    }
    if s.max_body.is_some_and(|max| body.len() > max) {
        return status(413, "Request entity too large");
    }
    let id = s.id();
    let page = FakePage {
        title,
        parent,
        version: 1,
        author: "Kiln Test".into(),
        body,
        property: None,
    };
    let reply = if cloud {
        cloud_page(id, &page)
    } else {
        json!({ "id": id.to_string(), "version": { "number": 1 } })
    };
    s.pages.insert(id, page);
    ok(reply)
}

fn update(
    s: &mut State,
    id: u64,
    req: &Value,
    body: Option<&Value>,
    parent: Option<u64>,
    cloud: bool,
) -> Reply {
    if let Some(i) = s.conflict_once.iter().position(|c| *c == id) {
        s.conflict_once.remove(i);
        return status(409, "Version must be incremented on update");
    }
    let Some(current) = s.pages.get(&id).cloned() else {
        return status(404, "not found");
    };
    let wanted = req
        .pointer("/version/number")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if wanted != current.version + 1 {
        return status(409, "Version must be incremented on update");
    }
    let body = body.and_then(Value::as_str).unwrap_or("").to_owned();
    if s.max_body.is_some_and(|max| body.len() > max) {
        return status(413, "Request entity too large");
    }
    let title = req["title"].as_str().unwrap_or(&current.title).to_owned();
    if title != current.title
        && s.pages
            .iter()
            .any(|(other, p)| *other != id && p.title == title)
    {
        return status(400, "A page with this title already exists in this space");
    }
    let page = s.pages.get_mut(&id).expect("checked above");
    page.title = title;
    page.body = body;
    page.version = wanted;
    page.author = "Kiln Test".into();
    if let Some(parent) = parent {
        page.parent = Some(parent);
    }
    let page = page.clone();
    ok(if cloud {
        cloud_page(id, &page)
    } else {
        json!({ "id": id.to_string(), "version": { "number": wanted } })
    })
}

fn put_property(s: &mut State, id: u64, req: &Value, version: Option<u64>) -> Reply {
    let current = s.extra.get(&id).map(|e| e.property_version).unwrap_or(0);
    let exists = s.pages.get(&id).is_some_and(|p| p.property.is_some());
    match version {
        None if exists => return status(409, "property exists"),
        Some(v) if v != current + 1 => return status(409, "property version must be incremented"),
        _ => {}
    }
    let Some(page) = s.pages.get_mut(&id) else {
        return status(404, "not found");
    };
    page.property = Some(req["value"].clone());
    s.extra.entry(id).or_default().property_version = current + 1;
    ok(json!({ "key": "kiln", "version": { "number": current + 1 } }))
}

fn labels(s: &State, id: u64) -> Reply {
    let names: Vec<Value> = s
        .extra
        .get(&id)
        .map(|e| e.labels.iter().map(|l| json!({ "name": l })).collect())
        .unwrap_or_default();
    ok(json!({ "results": names }))
}

fn attachments(s: &State, id: u64) -> Reply {
    let list: Vec<Value> = s
        .extra
        .get(&id)
        .map(|e| {
            e.attachments
                .iter()
                .map(|(a, n, _)| json!({ "id": a, "title": n }))
                .collect()
        })
        .unwrap_or_default();
    ok(json!({ "results": list }))
}

fn remove_attachment(s: &mut State, att: &str) -> Reply {
    let id = format!("att{att}");
    for e in s.extra.values_mut() {
        if let Some(i) = e.attachments.iter().position(|(a, _, _)| *a == id) {
            e.attachments.remove(i);
            return (204, String::new(), String::new());
        }
    }
    status(404, "no such attachment")
}

/// The file name and bytes of a one-part multipart body.
fn parse_multipart(body: &[u8]) -> (String, Vec<u8>) {
    let text = String::from_utf8_lossy(body);
    let name = text
        .split("filename=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .unwrap_or("")
        .to_owned();
    let start = body
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(0);
    let end = body
        .windows(4)
        .rposition(|w| w == b"\r\n--")
        .unwrap_or(body.len());
    (name, body[start..end.max(start)].to_vec())
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
