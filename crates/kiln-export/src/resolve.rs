//! Resolving item references once pages are known (EXPORT.md, "The
//! pipeline", step 3): same page → anchor; another page of this export →
//! that page (plus anchor); not in this export → the first `link_to`
//! export holding it; else the item's file in the repository.

use kiln_index::Project;
use kiln_model::ItemId;
use kiln_render::{Block, Inline, Target};

use std::collections::HashMap;

use crate::document::Node;
use crate::split::{Location, Page};

/// Where each item is, in one export.
pub type Locations = HashMap<ItemId, Location>;

/// What references can point into.
pub struct Links<'a> {
    pub project: &'a Project,
    pub own: &'a Locations,
    /// `link_to` exports, by name, in order.
    pub others: Vec<(String, &'a Locations)>,
}

impl Links<'_> {
    fn target(&self, id: &ItemId, page: &str) -> Option<Target> {
        let item = self.project.item(id)?;
        if let Some(loc) = self.own.get(id) {
            return Some(if loc.page == page {
                if loc.own_page {
                    Target::Page {
                        export: None,
                        key: loc.page.clone(),
                        anchor: None,
                    }
                } else {
                    Target::Anchor(id.to_string())
                }
            } else {
                Target::Page {
                    export: None,
                    key: loc.page.clone(),
                    anchor: (!loc.own_page).then(|| id.to_string()),
                }
            });
        }
        for (name, locations) in &self.others {
            if let Some(loc) = locations.get(id) {
                return Some(Target::Page {
                    export: Some(name.clone()),
                    key: loc.page.clone(),
                    anchor: (!loc.own_page).then(|| id.to_string()),
                });
            }
        }
        Some(Target::Repo {
            path: item.path.clone(),
            line: None,
            fragment: None,
        })
    }
}

/// Replaces every `Inline::Ref` on `page` with a link (or plain text for
/// an ID that doesn't exist, which `kiln check` reports as K102).
pub fn resolve_page(page: &mut Page, links: &Links) {
    let key = page.key.clone();
    for node in &mut page.nodes {
        resolve_node(node, links, &key);
    }
}

fn resolve_node(node: &mut Node, links: &Links, page: &str) {
    match node {
        Node::Block(b) => resolve_block(b, links, page),
        Node::Section(s) => {
            resolve_inlines(&mut s.title, links, page);
            for n in &mut s.nodes {
                resolve_node(n, links, page);
            }
        }
    }
}

fn resolve_block(block: &mut Block, links: &Links, page: &str) {
    match block {
        Block::Heading { content, .. } | Block::Paragraph(content) => {
            resolve_inlines(content, links, page)
        }
        Block::Quote(blocks)
        | Block::Callout {
            content: blocks, ..
        } => blocks
            .iter_mut()
            .for_each(|b| resolve_block(b, links, page)),
        Block::List { items, .. } => items.iter_mut().for_each(|i| {
            i.content
                .iter_mut()
                .for_each(|b| resolve_block(b, links, page))
        }),
        Block::Table(t) => {
            t.header
                .iter_mut()
                .for_each(|c| resolve_inlines(c, links, page));
            t.rows
                .iter_mut()
                .flatten()
                .for_each(|c| resolve_inlines(c, links, page));
        }
        Block::Props(props) => {
            for entry in &mut props.entries {
                entry
                    .values
                    .iter_mut()
                    .for_each(|v| resolve_inlines(v, links, page));
            }
        }
        _ => {}
    }
}

fn resolve_inlines(inlines: &mut [Inline], links: &Links, page: &str) {
    for inline in inlines.iter_mut() {
        match inline {
            Inline::Ref { id, text } => {
                let label = match (text.clone(), links.project.item(id)) {
                    (Some(t), _) => t,
                    (None, Some(item)) => format!("{} {}", item.id, item.title),
                    (None, None) => id.to_string(),
                };
                *inline = match links.target(id, page) {
                    Some(target) => Inline::Link {
                        target,
                        content: vec![Inline::Text(label)],
                    },
                    None => Inline::Text(label),
                };
            }
            Inline::Emph(c) | Inline::Strong(c) | Inline::Strike(c) => {
                resolve_inlines(c, links, page)
            }
            // A reference inside a link's text stays text: links don't nest.
            Inline::Link { content, .. } => flatten_refs(content),
            _ => {}
        }
    }
}

fn flatten_refs(inlines: &mut [Inline]) {
    for inline in inlines.iter_mut() {
        if let Inline::Ref { id, text } = inline {
            *inline = Inline::Text(text.clone().unwrap_or_else(|| id.to_string()));
        }
    }
}
