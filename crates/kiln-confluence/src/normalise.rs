//! Normalising storage before hashing (CONFLUENCE.md, "Drift"), so a
//! body Confluence rewrote on save hashes the same as what Kiln sent, and
//! only a person's edit counts as drift: parse as XML; drop
//! `ac:macro-id`, `ac:local-id`, `local-id` and `data-*` attributes; drop
//! `ac:inline-comment-marker` elements (keeping their text); sort
//! attributes; drop whitespace-only text between elements; treat CDATA as
//! text; serialise canonically.

use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};

const DROPPED_ELEMENT: &[u8] = b"ac:inline-comment-marker";

fn dropped_attribute(name: &str) -> bool {
    matches!(name, "ac:macro-id" | "ac:local-id" | "local-id") || name.starts_with("data-")
}

/// The canonical form of a storage body; an error for a body that isn't
/// well-formed.
pub fn normalise(body: &str) -> Result<String, String> {
    let wrapped = format!("<kiln-root>{body}</kiln-root>");
    let mut reader = Reader::from_str(&wrapped);
    reader.config_mut().trim_text(false);
    let mut out = String::with_capacity(body.len());
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut String| {
        if !text.trim().is_empty() {
            out.push_str(&escape_text(text));
        }
        text.clear();
    };
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("not well-formed: {e}"))?;
        match event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == DROPPED_ELEMENT => {}
            Event::End(e) if e.name().as_ref() == DROPPED_ELEMENT => {}
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"kiln-root" => {}
            Event::End(e) if e.name().as_ref() == b"kiln-root" => {}
            Event::Start(ref e) | Event::Empty(ref e) => {
                flush(&mut text, &mut out);
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let mut attrs: Vec<(String, String)> = Vec::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| format!("not well-formed: {e}"))?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    if dropped_attribute(&key) {
                        continue;
                    }
                    attrs.push((key, unescape(&String::from_utf8_lossy(&attr.value))));
                }
                attrs.sort();
                out.push('<');
                out.push_str(&name);
                for (k, v) in attrs {
                    out.push_str(&format!(" {k}=\"{}\"", escape_attr(&v)));
                }
                out.push_str(if matches!(event, Event::Empty(_)) {
                    "/>"
                } else {
                    ">"
                });
            }
            Event::End(e) => {
                flush(&mut text, &mut out);
                out.push_str(&format!(
                    "</{}>",
                    String::from_utf8_lossy(e.name().as_ref())
                ));
            }
            Event::Text(t) => text.push_str(&t.decode().map_err(|e| e.to_string())?),
            Event::CData(c) => text.push_str(&String::from_utf8_lossy(&c)),
            Event::GeneralRef(r) => {
                let name = r.decode().map_err(|e| e.to_string())?;
                text.push_str(&entity(&name));
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    flush(&mut text, &mut out);
    Ok(out)
}

/// `sha256:<hex>` of the normalised body, or of the raw body when it
/// isn't well-formed (then any change at all counts).
pub fn body_hash(body: &str) -> String {
    let canonical = normalise(body).unwrap_or_else(|_| body.to_owned());
    hash(canonical.as_bytes())
}

/// `sha256:<hex>`.
pub fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// The character an entity names: XML's five, numeric references, and the
/// HTML entities Confluence writes; any other stays as written.
fn entity(name: &str) -> String {
    let c = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        "ndash" => Some('–'),
        "mdash" => Some('—'),
        "hellip" => Some('…'),
        "rsquo" => Some('’'),
        "lsquo" => Some('‘'),
        "rdquo" => Some('”'),
        "ldquo" => Some('“'),
        n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
            .ok()
            .and_then(char::from_u32),
        n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
        _ => None,
    };
    c.map(String::from).unwrap_or_else(|| format!("&{name};"))
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        match rest[at..].find(';') {
            Some(end) if end > 1 && end < 12 => {
                out.push_str(&entity(&rest[at + 1..at + end]));
                rest = &rest[at + end + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confluence_rewrites_hash_the_same() {
        let sent = "<h1><ac:structured-macro ac:name=\"anchor\"><ac:parameter ac:name=\"\">REQ-014</ac:parameter></ac:structured-macro>REQ-014 Fast</h1>\n<p>a &amp; b</p><ac:structured-macro ac:name=\"code\"><ac:plain-text-body><![CDATA[x < y]]></ac:plain-text-body></ac:structured-macro>";
        let saved = "<h1><ac:structured-macro ac:macro-id=\"1f2e\" ac:name=\"anchor\" ac:schema-version=\"1\"><ac:parameter ac:name=\"\">REQ-014</ac:parameter></ac:structured-macro>REQ-014 Fast</h1><p data-x=\"1\">a &amp; <ac:inline-comment-marker ac:ref=\"c1\">b</ac:inline-comment-marker></p><ac:structured-macro ac:name=\"code\"><ac:plain-text-body>x &lt; y</ac:plain-text-body></ac:structured-macro>";
        // `ac:schema-version` is something Confluence adds; it isn't in the
        // documented list, so it is (deliberately) still a difference.
        let saved_without_schema = saved.replace(" ac:schema-version=\"1\"", "");
        assert_eq!(body_hash(sent), body_hash(&saved_without_schema));
        assert_ne!(body_hash(sent), body_hash(saved));
        assert_ne!(body_hash(sent), body_hash(&sent.replace("Fast", "Slow")));
    }

    #[test]
    fn entities_and_attribute_order() {
        assert_eq!(
            normalise("<p b=\"2\" a=\"1\">x&nbsp;y &#8212; &unknown;</p>").unwrap(),
            "<p a=\"1\" b=\"2\">x\u{a0}y — &amp;unknown;</p>"
        );
        assert!(normalise("<p>unclosed").is_err());
    }
}
