//! `kiln confluence comments`: the unresolved footer comments on a
//! target's published pages, each attributed to an item where that can be
//! told (CONFLUENCE.md, "Comments"), so feedback left in Confluence
//! reaches the people who edit the repository.
//!
//! A comment on an item's own page (`tree`) belongs to that item. On a
//! page holding many items (`document`, `sections`), a comment that
//! quotes the page (a `<blockquote>`, as Confluence's "quote" does) is
//! placed at the nearest item anchor above the quoted text; any other
//! comment is about the page as a whole. Reading only, never writing: a
//! comment is answered in Confluence.

use serde::Serialize;

use crate::client::{ClientError, Comment, Confluence};
use crate::model::Snapshot;

/// One unresolved comment, placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placed {
    /// The item it is about, when that can be told.
    pub item: Option<String>,
    pub page: u64,
    pub page_title: String,
    pub author: String,
    pub created: String,
    /// The comment as one line of plain text.
    pub text: String,
    pub url: String,
}

/// Every unresolved footer comment on the pages of `project`/`export` in
/// `snapshot`, in page order, attributed.
pub fn unresolved(
    client: &dyn Confluence,
    snapshot: &Snapshot,
    base_url: &str,
    project: &str,
    export: &str,
) -> Result<Vec<Placed>, ClientError> {
    let base = base_url.trim_end_matches('/');
    let mut out = Vec::new();
    for page in snapshot.pages() {
        let Some(property) = page.property.as_ref() else {
            continue;
        };
        if property.project != project || property.export != export || property.retired {
            continue;
        }
        let comments: Vec<Comment> = client
            .comments(page.id)?
            .into_iter()
            .filter(|c| !c.resolved)
            .collect();
        if comments.is_empty() {
            continue;
        }
        let own_item = property.key.strip_prefix("item:").map(str::to_owned);
        // Read only when something has to be placed in it.
        let anchors = if own_item.is_none() && comments.iter().any(|c| quote(&c.body).is_some()) {
            Some(Anchored::new(&client.body(page.id)?))
        } else {
            None
        };
        for c in comments {
            let item = own_item.clone().or_else(|| {
                let quoted = quote(&c.body)?;
                anchors.as_ref()?.item_before(&quoted)
            });
            out.push(Placed {
                item,
                page: page.id,
                page_title: page.title.clone(),
                author: c.author.clone(),
                created: c.created.clone(),
                text: one_line(&plain(&c.body).0),
                url: format!(
                    "{base}/pages/viewpage.action?pageId={}&focusedCommentId={}#comment-{}",
                    page.id, c.id, c.id
                ),
            });
        }
    }
    Ok(out)
}

/// The text a comment quotes: its first `<blockquote>`.
fn quote(body: &str) -> Option<String> {
    let start = body.find("<blockquote")?;
    let open_end = start + body[start..].find('>')? + 1;
    let end = open_end + body[open_end..].find("</blockquote>")?;
    let text = squash(&plain(&body[open_end..end]).0);
    (!text.is_empty()).then_some(text)
}

/// A page's text with the item anchors in it: plain text, and each
/// anchor's ID at its offset.
struct Anchored {
    text: String,
    anchors: Vec<(usize, String)>,
}

impl Anchored {
    fn new(body: &str) -> Anchored {
        let (text, anchors) = plain(body);
        let squashed = squash(&text);
        // Offsets move when whitespace is squashed: map them over.
        let mut map = Vec::with_capacity(text.len() + 1);
        let mut out_len = 0;
        let mut last_space = true;
        for c in text.chars() {
            map.push(out_len);
            let space = c.is_whitespace();
            if !(space && last_space) {
                out_len += if space { 1 } else { c.len_utf8() };
            }
            last_space = space;
            for _ in 1..c.len_utf8() {
                map.push(out_len);
            }
        }
        map.push(out_len);
        let anchors = anchors
            .into_iter()
            .map(|(at, id)| (map.get(at).copied().unwrap_or(out_len), id))
            .collect();
        Anchored {
            text: squashed,
            anchors,
        }
    }

    /// The last anchor at or before where `quoted` appears.
    fn item_before(&self, quoted: &str) -> Option<String> {
        let at = self.text.find(quoted)?;
        self.anchors
            .iter()
            .take_while(|(pos, _)| *pos <= at)
            .last()
            .map(|(_, id)| id.clone())
    }
}

/// Storage format to plain text, with the anchor macros' names and where
/// they were. Tags are dropped (a block's end becomes a space), the usual
/// entities decoded, CDATA kept as text.
fn plain(body: &str) -> (String, Vec<(usize, String)>) {
    let mut text = String::new();
    let mut anchors = Vec::new();
    let mut rest = body;
    let mut in_anchor = false;
    while let Some(lt) = rest.find('<') {
        text.push_str(&decode(&rest[..lt]));
        let tail = &rest[lt..];
        if let Some(cdata) = tail.strip_prefix("<![CDATA[") {
            let end = cdata.find("]]>").unwrap_or(cdata.len());
            text.push_str(&cdata[..end]);
            rest = cdata.get(end + 3..).unwrap_or("");
            continue;
        }
        let Some(gt) = tail.find('>') else {
            rest = "";
            break;
        };
        let tag = &tail[..=gt];
        if tag.starts_with("<ac:structured-macro") && tag.contains("ac:name=\"anchor\"") {
            in_anchor = true;
        } else if in_anchor && tag.starts_with("<ac:parameter") {
            let after = &tail[gt + 1..];
            if let Some(end) = after.find("</ac:parameter>") {
                anchors.push((text.len(), decode(&after[..end]).trim().to_owned()));
                in_anchor = false;
                rest = &after[end..];
                continue;
            }
        } else if tag.starts_with("</") || !(tag.starts_with("<ac:") || tag.starts_with("<ri:")) {
            // Any HTML tag ends a word; Confluence's own elements don't.
            text.push(' ');
        }
        rest = &tail[gt + 1..];
    }
    text.push_str(&decode(rest));
    (text, anchors)
}

fn decode(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Runs of whitespace as one space, trimmed.
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// At most 200 characters on one line.
fn one_line(s: &str) -> String {
    let s = squash(s);
    if s.chars().count() <= 200 {
        s
    } else {
        let mut t: String = s.chars().take(199).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_land_on_the_item_above_them() {
        let body = "<h1>Intro</h1><p>General words.</p><h2><ac:structured-macro ac:name=\"anchor\"><ac:parameter ac:name=\"\">REQ-001</ac:parameter></ac:structured-macro>1.1 REQ-001 Fast</h2><p>Starts in   a second &amp; stays.</p><h2><ac:structured-macro ac:name=\"anchor\"><ac:parameter ac:name=\"\">REQ-002</ac:parameter></ac:structured-macro>1.2 REQ-002 Small</h2><p>Under ten megabytes.</p>";
        let page = Anchored::new(body);
        assert_eq!(
            page.item_before("Starts in a second & stays").as_deref(),
            Some("REQ-001")
        );
        assert_eq!(
            page.item_before("ten megabytes").as_deref(),
            Some("REQ-002")
        );
        assert_eq!(page.item_before("General words"), None, "above every item");
        assert_eq!(page.item_before("not on the page"), None);
        assert_eq!(
            quote("<blockquote><p>Starts in a\nsecond</p></blockquote><p>Really?</p>").as_deref(),
            Some("Starts in a second")
        );
        assert_eq!(quote("<p>No quote</p>"), None);
        assert_eq!(
            one_line(&plain("<p>Is this<br/>measured cold?</p>").0),
            "Is this measured cold?"
        );
    }
}
