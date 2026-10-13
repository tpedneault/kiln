//! The `kiln` content property (CONFLUENCE.md, "The `kiln` content
//! property") and the snapshot of what a target's pages look like in
//! Confluence, which `--snapshot <file>` reads and `--save-snapshot`
//! writes for offline dry runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The `kiln` property on every page Kiln publishes: which project,
/// export and key the page is, and what Kiln last wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KilnProperty {
    pub schema: u32,
    pub project: String,
    pub export: String,
    pub key: String,
    /// Items on the page → the last commit that changed each.
    #[serde(default)]
    pub items: BTreeMap<String, String>,
    /// The commit of the publish that last wrote this page.
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub render_hash: String,
    #[serde(default)]
    pub remote_hash: String,
    #[serde(default)]
    pub published_version: u64,
    #[serde(default)]
    pub attachments: BTreeMap<String, String>,
}

/// One page as Confluence has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePage {
    pub id: u64,
    pub title: String,
    /// `None` for a page at the space's top.
    #[serde(default)]
    pub parent_id: Option<u64>,
    pub version: u64,
    #[serde(default)]
    pub version_author: String,
    #[serde(default)]
    pub version_when: String,
    /// The `kiln` property, when the page has one.
    #[serde(default)]
    pub property: Option<KilnProperty>,
    /// The normalised hash of the current body, read only when the
    /// version moved past the property's `publishedVersion` (a possible
    /// edit).
    #[serde(default)]
    pub body_hash: Option<String>,
    /// Whether the body is empty (for adopting a page).
    #[serde(default)]
    pub empty: bool,
}

/// What a target's place in Confluence looks like.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The page the target writes under (`parent`), or adopts (`page`).
    pub anchor_page: u64,
    /// The root page: this target's `doc` page under `parent`, or the
    /// adopted page; `None` before the first publish.
    #[serde(default)]
    pub root: Option<RemotePage>,
    /// The root's descendants (`sections` and `tree`).
    #[serde(default)]
    pub descendants: Vec<RemotePage>,
    /// Titles in the space held by pages this target doesn't own, among
    /// the titles it wants (the space's titles are unique).
    #[serde(default)]
    pub taken_titles: Vec<String>,
}

impl Snapshot {
    /// Every page read, root first.
    pub fn pages(&self) -> impl Iterator<Item = &RemotePage> {
        self.root.iter().chain(self.descendants.iter())
    }
}
