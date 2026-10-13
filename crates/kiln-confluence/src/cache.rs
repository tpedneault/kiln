//! The local page cache (CONFLUENCE.md, "How a publish runs", step 2):
//! page id → version and `kiln` property, kept in Kiln's state folder as
//! `confluence/<space>-<project>-<export>.json`. A snapshot reads a page's
//! property only when its version differs from the cache, so an
//! unchanged tree of hundreds of pages costs one listing. CI has no cache
//! and reads every property, which is only slower.
//!
//! Pure: (de)serialising here, reading and writing the file is the
//! caller's.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{KilnProperty, Snapshot};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedPage {
    pub version: u64,
    pub property: Option<KilnProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCache {
    #[serde(default)]
    pub pages: BTreeMap<u64, CachedPage>,
}

impl PageCache {
    /// The cached property of page `id` at `version`; `None` when the
    /// cache doesn't know that version (read it).
    pub fn property(&self, id: u64, version: u64) -> Option<Option<KilnProperty>> {
        self.pages
            .get(&id)
            .filter(|c| c.version == version)
            .map(|c| c.property.clone())
    }

    pub fn remember(&mut self, id: u64, version: u64, property: Option<KilnProperty>) {
        self.pages.insert(id, CachedPage { version, property });
    }

    /// Every page a snapshot read.
    pub fn from_snapshot(snapshot: &Snapshot) -> PageCache {
        let mut cache = PageCache::default();
        for page in snapshot.pages() {
            cache.remember(page.id, page.version, page.property.clone());
        }
        cache
    }

    /// A cache file's text; an unreadable one is an empty cache.
    pub fn parse(text: &str) -> PageCache {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}
