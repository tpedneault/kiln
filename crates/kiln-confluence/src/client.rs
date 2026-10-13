//! The Confluence API, by flavour (CONFLUENCE.md, "The API, by flavour"):
//! one trait, two implementations. K2 implements the read half only;
//! every call Kiln makes is in that table and nothing else is used.
//!
//! Errors are one line naming the endpoint and the server's reason, never
//! the token. 429 and 503 wait for `Retry-After` (or back off from 1 s)
//! up to 6 attempts per call.

use std::time::Duration;

use kiln_model::{ConfluenceConfig, ConfluenceFlavour};
use kiln_secrets::Token;
use serde_json::Value;

use crate::model::{KilnProperty, RemotePage};

/// Why a call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientError {
    pub message: String,
    pub kind: ErrorKind,
}

/// The failures apply reacts to; everything else is `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorKind {
    #[default]
    Other,
    /// 409: the page changed since it was read.
    Conflict,
    /// 413: the page is too large for the server.
    TooLarge,
}

impl ClientError {
    pub fn new(message: impl Into<String>) -> Self {
        ClientError {
            message: message.into(),
            kind: ErrorKind::Other,
        }
    }
}

/// A request body.
pub enum Body {
    None,
    Json(Value),
    /// One file, as multipart field `file`.
    Multipart {
        name: String,
        bytes: Vec<u8>,
        mime: String,
    },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ClientError {}

fn err(message: impl Into<String>) -> ClientError {
    ClientError {
        message: message.into(),
        kind: ErrorKind::Other,
    }
}

/// Who a token belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    /// What restrictions name a user by: the username on Data Center,
    /// the account id on Cloud.
    pub key: String,
}

/// A page just created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    pub id: u64,
    pub version: u64,
}

/// The calls Kiln makes (CONFLUENCE.md, "The API, by flavour"), and
/// nothing else.
pub trait Confluence {
    /// The account the token belongs to, by display name.
    fn whoami(&self) -> Result<String, ClientError>;
    /// The account the token belongs to, for restrictions.
    fn identity(&self) -> Result<Identity, ClientError>;
    /// A page's title, parent, version and author; `None` when it doesn't
    /// exist (or the token can't see it).
    fn page(&self, id: u64) -> Result<Option<RemotePage>, ClientError>;
    /// A page's child pages, with versions.
    fn children(&self, id: u64) -> Result<Vec<RemotePage>, ClientError>;
    /// Every page below a page, with versions and parents.
    fn descendants(&self, id: u64) -> Result<Vec<RemotePage>, ClientError>;
    /// A page's `kiln` property.
    fn property(&self, id: u64) -> Result<Option<KilnProperty>, ClientError>;
    /// A page's storage body.
    fn body(&self, id: u64) -> Result<String, ClientError>;
    /// The id of the page titled `title` in `space`, if any.
    fn find_title(&self, space: &str, title: &str) -> Result<Option<u64>, ClientError>;
    /// Creates a page under `parent`.
    fn create_page(
        &self,
        space: &str,
        parent: u64,
        title: &str,
        body: &str,
    ) -> Result<Created, ClientError>;
    /// Writes version `version` of a page (the current one plus one),
    /// moving it under `parent` when given; returns the new version.
    fn update_page(
        &self,
        id: u64,
        parent: Option<u64>,
        title: &str,
        body: &str,
        version: u64,
        message: &str,
    ) -> Result<u64, ClientError>;
    /// Creates or replaces the page's `kiln` property.
    fn write_property(&self, id: u64, property: &KilnProperty) -> Result<(), ClientError>;
    fn labels(&self, id: u64) -> Result<Vec<String>, ClientError>;
    fn add_labels(&self, id: u64, labels: &[String]) -> Result<(), ClientError>;
    /// The page's attachments: (attachment id, file name).
    fn attachments(&self, id: u64) -> Result<Vec<(String, String)>, ClientError>;
    fn upload(&self, id: u64, name: &str, bytes: &[u8], mime: &str) -> Result<(), ClientError>;
    fn delete_attachment(&self, attachment_id: &str) -> Result<(), ClientError>;
    /// Restricts editing the page to `who` (`restrict_edits`).
    fn restrict_edits(&self, id: u64, who: &Identity) -> Result<(), ClientError>;
    /// Sends a page to the space's trash (`--prune-removed`).
    fn trash(&self, id: u64) -> Result<(), ClientError>;
}

/// HTTP with authentication, timeouts and retries.
pub struct Http {
    agent: ureq::Agent,
    base: String,
    /// `Bearer …` or `Basic …`. Never formatted anywhere else.
    authorization: String,
    /// Waits between retries; a no-op in tests.
    pub sleep: fn(Duration),
}

/// Percent-encodes a query value.
pub fn query(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl Http {
    /// `user` is the Cloud account e-mail (basic auth); `auth` as in
    /// `[confluence] auth`.
    pub fn new(config: &ConfluenceConfig, token: &Token, user: &str) -> Http {
        let cloud = config.resolved_flavour() == ConfluenceFlavour::Cloud;
        let basic = match config.auth.as_str() {
            "basic" => true,
            "bearer" => false,
            _ => cloud,
        };
        let authorization = if basic {
            format!(
                "Basic {}",
                base64(format!("{user}:{}", token.expose()).as_bytes())
            )
        } else {
            format!("Bearer {}", token.expose())
        };
        let mut builder = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("kiln/", env!("CARGO_PKG_VERSION")));
        // A proxy the environment names and that ureq accepts; an unusable
        // one is ignored rather than failing every call.
        if let Some(proxy) = proxy_for(config.base_url(), |k| std::env::var(k).ok())
            .and_then(|p| ureq::Proxy::new(p).ok())
        {
            builder = builder.proxy(proxy);
        }
        let agent = builder.build();
        Http {
            agent,
            base: config.base_url().to_owned(),
            authorization,
            sleep: std::thread::sleep,
        }
    }

    /// The scheme and host of the base URL, for `_links.next` paths.
    fn origin(&self) -> &str {
        let after = self.base.find("://").map(|i| i + 3).unwrap_or(0);
        match self.base[after..].find('/') {
            Some(i) => &self.base[..after + i],
            None => &self.base,
        }
    }

    /// GETs `path` (from the base URL, or from the host when it starts
    /// with the base's own path, as `_links.next` does). `Ok(None)` for
    /// 404.
    pub fn get(&self, path: &str) -> Result<Option<Value>, ClientError> {
        self.request("GET", path, Body::None)
    }

    /// Sends a request with retries on 429 and 503. `Ok(None)` for 404,
    /// and for a success with no JSON body (a 204).
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Body,
    ) -> Result<Option<Value>, ClientError> {
        let base_path = &self.base[self.origin().len()..];
        let url = if !base_path.is_empty() && path.starts_with(base_path) {
            format!("{}{path}", self.origin())
        } else {
            format!("{}{path}", self.base)
        };
        let endpoint = path.split('?').next().unwrap_or(path);
        // Attachments and big bodies get longer (CONFLUENCE.md, "Errors,
        // limits, retries").
        let long = match &body {
            Body::Multipart { .. } => true,
            Body::Json(v) => v.to_string().len() > 1024 * 1024,
            Body::None => false,
        };
        let mut wait = Duration::from_secs(1);
        for attempt in 1..=6 {
            let mut req = self
                .agent
                .request(method, &url)
                .set("Authorization", &self.authorization)
                .set("Accept", "application/json");
            if long {
                req = req.timeout(Duration::from_secs(120));
            }
            if method != "GET" {
                // Confluence's XSRF check; harmless where unneeded.
                req = req.set("X-Atlassian-Token", "no-check");
            }
            let response = match &body {
                Body::None => req.call(),
                Body::Json(v) => req
                    .set("Content-Type", "application/json")
                    .send_string(&v.to_string()),
                Body::Multipart { name, bytes, mime } => {
                    let (content_type, payload) = multipart(name, bytes, mime);
                    req.set("Content-Type", &content_type).send_bytes(&payload)
                }
            };
            match response {
                Ok(r) => {
                    let text = r
                        .into_string()
                        .map_err(|e| err(format!("Confluence {method} {endpoint}: reading the reply: {e}")))?;
                    if text.trim().is_empty() {
                        return Ok(None);
                    }
                    return serde_json::from_str(&text)
                        .map(Some)
                        .map_err(|e| err(format!("Confluence {method} {endpoint}: the reply isn't JSON: {e}")));
                }
                Err(ureq::Error::Status(404, _)) => return Ok(None),
                Err(ureq::Error::Status(401, _)) => {
                    return Err(err(format!(
                        "Confluence refused the token for {} (run `kiln auth confluence`, or check KILN_CONFLUENCE_TOKEN, and KILN_CONFLUENCE_USER on Cloud)",
                        self.base
                    )))
                }
                Err(ureq::Error::Status(429 | 503, r)) if attempt < 6 => {
                    let retry = r.header("Retry-After").and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs);
                    (self.sleep)(retry.unwrap_or(wait));
                    wait = (wait * 2).min(Duration::from_secs(32));
                }
                Err(ureq::Error::Status(code, r)) => {
                    let text = r.into_string().unwrap_or_default();
                    let reason = serde_json::from_str::<Value>(&text)
                        .ok()
                        .and_then(|v| {
                            v.get("message")
                                .or_else(|| v.pointer("/errors/0/title"))
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| text.lines().next().unwrap_or("").chars().take(200).collect());
                    let kind = match code {
                        409 => ErrorKind::Conflict,
                        413 => ErrorKind::TooLarge,
                        _ => ErrorKind::Other,
                    };
                    return Err(ClientError {
                        message: format!("Confluence {method} {endpoint}: HTTP {code}: {reason}"),
                        kind,
                    });
                }
                Err(ureq::Error::Transport(t)) => {
                    return Err(err(format!(
                        "Confluence {method} {endpoint}: {}",
                        t.to_string().lines().next().unwrap_or("network error")
                    )))
                }
            }
        }
        Err(err(format!(
            "Confluence {method} {endpoint}: still rate-limited after 6 attempts"
        )))
    }

    /// GETs every page of a paged result, following `_links.next`.
    pub fn get_all(&self, first: &str) -> Result<Vec<Value>, ClientError> {
        let mut out = Vec::new();
        let mut next = Some(first.to_owned());
        let mut guard = 0;
        while let Some(path) = next.take() {
            guard += 1;
            if guard > 10_000 {
                return Err(err(format!(
                    "Confluence GET {first}: more than 10,000 pages of results"
                )));
            }
            let Some(value) = self.get(&path)? else { break };
            if let Some(results) = value.get("results").and_then(Value::as_array) {
                out.extend(results.iter().cloned());
            }
            next = value
                .pointer("/_links/next")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        Ok(out)
    }
}

/// The proxy for `url` from the environment (read through `get`):
/// `HTTPS_PROXY` for https and `HTTP_PROXY` for http (either case), else
/// `ALL_PROXY`, unless `NO_PROXY` names the host. ureq's own reading
/// ignores `NO_PROXY`, which would send an internal Confluence through a
/// corporate proxy that can't reach it.
pub fn proxy_for(url: &str, get: impl Fn(&str) -> Option<String>) -> Option<String> {
    let var = |name: &str| {
        get(name)
            .or_else(|| get(&name.to_ascii_lowercase()))
            .filter(|v| !v.trim().is_empty())
    };
    let (scheme, rest) = url.split_once("://")?;
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = match host_port.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host_port
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(host_port),
    }
    .to_ascii_lowercase();
    if let Some(no_proxy) = var("NO_PROXY") {
        for entry in no_proxy
            .split(',')
            .map(|e| e.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty())
        {
            let entry = entry
                .rsplit_once(':')
                .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()))
                .map(|(h, _)| h.to_owned())
                .unwrap_or(entry);
            let domain = entry.trim_start_matches('.');
            if entry == "*"
                || host == domain
                || host.ends_with(&format!(".{domain}"))
                || in_cidr(&host, &entry)
            {
                return None;
            }
        }
    }
    let specific = if scheme.eq_ignore_ascii_case("https") {
        "HTTPS_PROXY"
    } else {
        "HTTP_PROXY"
    };
    var(specific).or_else(|| var("ALL_PROXY"))
}

/// Whether IPv4 `host` is inside `cidr` (`127.0.0.0/8`).
fn in_cidr(host: &str, cidr: &str) -> bool {
    let parse = |s: &str| s.parse::<std::net::Ipv4Addr>().ok().map(u32::from);
    let Some((net, bits)) = cidr.split_once('/') else {
        return false;
    };
    match (parse(host), parse(net), bits.parse::<u32>()) {
        (Some(h), Some(n), Ok(b)) if b <= 32 => {
            let mask = if b == 0 { 0 } else { u32::MAX << (32 - b) };
            h & mask == n & mask
        }
        _ => false,
    }
}

/// A multipart/form-data body with one part, field `file`: (content type,
/// bytes). `ureq` 2 builds no multipart bodies, so this does, with a
/// boundary that doesn't occur in the bytes.
pub fn multipart(name: &str, bytes: &[u8], mime: &str) -> (String, Vec<u8>) {
    let mut boundary = format!("kiln-{}", &crate::hash(bytes)[7..31]);
    while bytes
        .windows(boundary.len())
        .any(|w| w == boundary.as_bytes())
    {
        boundary.push('x');
    }
    let filename: String = name
        .chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect();
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// The client for `config`'s flavour.
pub fn connect(config: &ConfluenceConfig, token: &Token, user: &str) -> Box<dyn Confluence> {
    let http = Http::new(config, token, user);
    match config.resolved_flavour() {
        ConfluenceFlavour::Cloud => Box::new(crate::cloud::Cloud::new(http)),
        _ => Box::new(crate::dc::DataCenter { http }),
    }
}

/// A property's JSON value as Kiln's.
pub fn parse_property(value: &Value) -> Option<KilnProperty> {
    // Stored as JSON; some flavours hand it back as a JSON string.
    match value {
        Value::String(s) => serde_json::from_str(s).ok(),
        v => serde_json::from_value(v.clone()).ok(),
    }
}

/// Standard base64, for basic authentication.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::proxy_for;

    #[test]
    fn proxies_follow_the_scheme_and_no_proxy() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let corp = env(&[
            ("HTTPS_PROXY", "http://proxy:3128"),
            (
                "no_proxy",
                "localhost,.internal.example.com,10.0.0.0/8,wiki.corp:443",
            ),
        ]);
        assert_eq!(
            proxy_for("https://x.atlassian.net/wiki", corp).as_deref(),
            Some("http://proxy:3128")
        );
        assert_eq!(
            proxy_for("https://confluence.internal.example.com", corp),
            None
        );
        assert_eq!(proxy_for("https://10.1.2.3/confluence", corp), None);
        assert_eq!(proxy_for("https://wiki.corp", corp), None);
        assert_eq!(proxy_for("http://localhost:8090", corp), None);
        assert_eq!(
            proxy_for("http://plain.example.com", corp),
            None,
            "HTTPS_PROXY isn't for http"
        );
        let all = env(&[("ALL_PROXY", "socks5://p:1080")]);
        assert_eq!(
            proxy_for("http://plain.example.com", all).as_deref(),
            Some("socks5://p:1080")
        );
        assert_eq!(
            proxy_for(
                "https://x",
                env(&[("NO_PROXY", "*"), ("HTTPS_PROXY", "http://p")])
            ),
            None
        );
    }
}
