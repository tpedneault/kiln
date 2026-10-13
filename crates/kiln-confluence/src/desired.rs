//! Desired pages: a target's split pages as Confluence will hold them,
//! with titles, labels, storage bodies, attachments, the items on each
//! page and the `renderHash` the planner compares (CONFLUENCE.md, "How a
//! publish runs", step 1).

use std::collections::{BTreeMap, HashMap, HashSet};

use kiln_export::{short, Prepared, RepoLinks, Revision};
use kiln_index::Project;
use kiln_model::ConfluenceConfig;

use crate::normalise::hash;
use crate::storage::{emit, Attachment, EmitContext};

/// The longest page title Confluence takes.
pub const MAX_TITLE: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredPage {
    pub key: String,
    pub title: String,
    /// The parent page's key; `None` for the root.
    pub parent: Option<String>,
    pub body: String,
    pub labels: Vec<String>,
    pub attachments: Vec<Attachment>,
    /// Items on the page → the last commit that changed each.
    pub items: BTreeMap<String, String>,
    /// sha256 over title, parent key, body, labels and attachment hashes.
    pub render_hash: String,
}

/// What building desired pages needs.
pub struct DesiredContext<'a> {
    pub config: &'a ConfluenceConfig,
    pub project: &'a Project,
    pub revision: &'a Revision,
    /// The last commit that changed each file.
    pub last_commits: &'a HashMap<String, String>,
    /// Repository links (web, or none).
    pub repo: &'a RepoLinks,
    pub image_hash: &'a dyn Fn(&str) -> Option<String>,
    /// Other prepared targets, for `link_to` titles.
    pub all: &'a [Prepared],
    /// Titles held in the space by pages this target doesn't own.
    pub taken_titles: &'a HashSet<String>,
}

/// A page's title before disambiguation, capped at 255 characters by
/// shortening an item's title, never its ID.
fn base_title(target_title: &str, key: &str, page_title: &str) -> String {
    let title = if key == "doc" {
        target_title
    } else {
        page_title
    };
    cap(title)
}

fn cap(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE {
        return title.to_owned();
    }
    let kept: String = title.chars().take(MAX_TITLE - 1).collect();
    format!("{}…", kept.trim_end())
}

/// Every page's final title: the base title, or with the target's title
/// in parentheses when another page in the space has it.
pub fn titles(p: &Prepared, taken: &HashSet<String>) -> HashMap<String, String> {
    p.split
        .pages
        .iter()
        .map(|page| {
            let base = base_title(&p.target.title, &page.key, &page.title);
            let title = if page.key != "doc" && taken.contains(&base) {
                cap(&format!("{base} ({})", p.target.title))
            } else {
                base
            };
            (page.key.clone(), title)
        })
        .collect()
}

/// The desired pages of a resolved Confluence target.
pub fn desired_pages(p: &Prepared, ctx: &DesiredContext) -> Vec<DesiredPage> {
    let space = p
        .target
        .confluence
        .space
        .clone()
        .unwrap_or_else(|| ctx.config.space.clone());
    let own_titles = titles(p, ctx.taken_titles);
    let mut all_titles: HashMap<(Option<String>, String), String> = own_titles
        .iter()
        .map(|(k, t)| ((None, k.clone()), t.clone()))
        .collect();
    let mut spaces = HashMap::new();
    for name in &p.target.link_to {
        if let Some(other) = ctx
            .all
            .iter()
            .find(|o| &o.target.name == name && !o.target.format.is_file())
        {
            for (k, t) in titles(other, &HashSet::new()) {
                all_titles.insert((Some(name.clone()), k), t);
            }
            spaces.insert(
                name.clone(),
                other
                    .target
                    .confluence
                    .space
                    .clone()
                    .unwrap_or_else(|| ctx.config.space.clone()),
            );
        }
    }
    let mut labels: Vec<String> = ctx
        .config
        .labels
        .iter()
        .chain(&p.target.confluence.labels)
        .cloned()
        .collect();
    labels.sort();
    labels.dedup();
    let commit = ctx.revision.commit.clone().unwrap_or_default();
    let toc_levels = p.target.confluence.toc_levels.unwrap_or(3);

    p.split
        .pages
        .iter()
        .map(|page| {
            let item_on_own_page = page.key.strip_prefix("item:").and_then(|id| id.parse().ok()).and_then(|id| ctx.project.item(&id));
            let banner = match item_on_own_page {
                Some(item) => {
                    let last = ctx.last_commits.get(&item.path).unwrap_or(&commit);
                    format!(
                        "Published by Kiln from {} at {}. Edit it in the repository; changes made here are overwritten. Comment at the bottom of the page.",
                        item.path,
                        short(last)
                    )
                }
                None => {
                    let at = match (&ctx.revision.commit, &ctx.revision.date) {
                        (Some(c), Some(d)) => format!(" at {} ({d})", short(c)),
                        (Some(c), None) => format!(" at {}", short(c)),
                        _ => String::new(),
                    };
                    format!(
                        "Published by Kiln from {}{at}. Edit it in the repository; changes made here are overwritten. Comment at the bottom of the page.",
                        ctx.project.config.name
                    )
                }
            };
            let emit_ctx = EmitContext {
                config: ctx.config,
                space: &space,
                titles: &all_titles,
                spaces: &spaces,
                repo: ctx.repo,
                jira_url: ctx.project.config.jira_url.as_deref(),
                banner,
                image_hash: ctx.image_hash,
                toc_levels,
            };
            let emitted = emit(page, &emit_ctx);
            let items: BTreeMap<String, String> = page
                .items()
                .into_iter()
                .filter_map(|id| {
                    let item = ctx.project.item(&id)?;
                    Some((id.to_string(), ctx.last_commits.get(&item.path).cloned().unwrap_or_else(|| commit.clone())))
                })
                .collect();
            let title = own_titles.get(&page.key).cloned().unwrap_or_default();
            let mut hashed = format!("{title}\0{}\0{}\0{}\0", page.parent.clone().unwrap_or_default(), emitted.body, labels.join(","));
            let mut attachment_hashes: Vec<String> = emitted.attachments.iter().map(|a| format!("{}={}", a.name, a.hash)).collect();
            attachment_hashes.sort();
            hashed.push_str(&attachment_hashes.join(","));
            DesiredPage {
                key: page.key.clone(),
                title,
                parent: page.parent.clone(),
                body: emitted.body,
                labels: labels.clone(),
                attachments: emitted.attachments,
                items,
                render_hash: hash(hashed.as_bytes()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_titles_keep_their_start() {
        let long = format!("REQ-014 {}", "x".repeat(300));
        let capped = cap(&long);
        assert_eq!(capped.chars().count(), MAX_TITLE);
        assert!(capped.starts_with("REQ-014 ") && capped.ends_with('…'));
    }
}
