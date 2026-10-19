//! Taking a snapshot (CONFLUENCE.md, "How a publish runs", step 2): only
//! under the target's parent (or the adopted page). Kiln never looks at or
//! touches other pages, apart from title lookups for the titles it wants.

use std::collections::HashSet;

use kiln_model::ExportTarget;

use crate::cache::PageCache;
use crate::client::{ClientError, Confluence};
use crate::model::{RemotePage, Snapshot};
use crate::normalise::body_hash;

/// Reads `target`'s pages. `wanted_titles` are the titles its pages
/// would have, to find which are taken by pages it doesn't own.
pub fn take_snapshot(
    client: &dyn Confluence,
    target: &ExportTarget,
    project: &str,
    space: &str,
    wanted_titles: &[String],
    cache: &PageCache,
) -> Result<Snapshot, ClientError> {
    let property = |id: u64, version: u64| -> Result<_, ClientError> {
        match cache.property(id, version) {
            Some(known) => Ok(known),
            None => client.property(id),
        }
    };
    let ours = |p: &RemotePage| {
        p.property
            .as_ref()
            .is_some_and(|k| k.project == project && k.export == target.name)
    };
    let mut snapshot = Snapshot::default();
    if let Some(page_id) = target.confluence.page {
        snapshot.anchor_page = page_id;
        let mut root = client
            .page(page_id)?
            .ok_or_else(|| ClientError::new(format!("Confluence page {page_id} (export `{}`, page =) doesn't exist or the token can't see it", target.name)))?;
        snapshot.anchor_title = root.title.clone();
        root.property = client.property(page_id)?;
        if root.property.is_none() {
            root.empty = client.body(page_id)?.trim().is_empty();
        }
        snapshot.root = Some(root);
    } else if let Some(parent) = target.confluence.parent {
        snapshot.anchor_page = parent;
        let page = client.page(parent)?.ok_or_else(|| ClientError::new(format!("Confluence page {parent} (export `{}`, parent =) doesn't exist or the token can't see it", target.name)))?;
        snapshot.anchor_title = page.title;
        for mut child in client.children(parent)? {
            child.property = property(child.id, child.version)?;
            if ours(&child) && child.property.as_ref().is_some_and(|k| k.key == "doc") {
                snapshot.root = Some(child);
                break;
            }
        }
    }
    if let Some(root) = &snapshot.root {
        // Every layout: pages left from an earlier layout (a tree turned
        // into a document) are retired, so they must be seen.
        {
            for mut page in client.descendants(root.id)? {
                page.property = property(page.id, page.version)?;
                if page.property.is_none() {
                    page.leftover = client.body(page.id)? == crate::apply::PLACEHOLDER;
                }
                snapshot.descendants.push(page);
            }
        }
    }
    // Bodies only where the version moved past Kiln's last write: a
    // possible edit (drift).
    let root_id = snapshot.root.as_ref().map(|r| r.id);
    let mut pages: Vec<&mut RemotePage> = snapshot
        .root
        .iter_mut()
        .chain(snapshot.descendants.iter_mut())
        .collect();
    for page in pages.iter_mut() {
        let moved = page
            .property
            .as_ref()
            .is_some_and(|k| page.version > k.published_version);
        if moved {
            page.body_hash = Some(body_hash(&client.body(page.id)?));
        }
    }
    let own_titles: HashSet<String> = snapshot
        .pages()
        .filter(|p| ours(p) || p.leftover)
        .map(|p| p.title.clone())
        .collect();
    let mut taken = Vec::new();
    for title in wanted_titles {
        if own_titles.contains(title) {
            continue;
        }
        if let Some(id) = client.find_title(space, title)? {
            if Some(id) != root_id {
                taken.push(title.clone());
            }
        }
    }
    taken.sort();
    taken.dedup();
    snapshot.taken_titles = taken;
    Ok(snapshot)
}
