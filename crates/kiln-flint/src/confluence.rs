//! "Open in Confluence": the page an item is published on. First from
//! the page cache `kiln export` keeps in the state folder (no network: a
//! machine that published knows every page), then by asking the space
//! (the token from `kiln auth confluence` or `KILN_CONFLUENCE_TOKEN`), for
//! a machine where only CI publishes. The URL carries the item's anchor,
//! so a one-page document opens at the item.

use std::path::Path;

use kiln_confluence::{cache_file_name, PageCache};
use kiln_index::Project;
use kiln_model::{ExportFormat, ExportTarget, ItemId};

/// The first Confluence target, which "Open in Confluence" means.
pub fn target(project: &Project) -> Option<&ExportTarget> {
    project
        .config
        .exports
        .iter()
        .find(|t| t.format == ExportFormat::Confluence)
}

fn page_url(base: &str, page: u64, id: &ItemId) -> String {
    format!(
        "{}/pages/viewpage.action?pageId={page}#{id}",
        base.trim_end_matches('/')
    )
}

/// The page holding `id` according to a cache file's text: its own page
/// (`item:<ID>`) first, else the page whose items include it.
pub fn from_cache(cache: &str, base: &str, id: &ItemId) -> Option<String> {
    let cache = PageCache::parse(cache);
    let key = format!("item:{id}");
    let wanted = id.to_string();
    let own = cache.pages.iter().find(|(_, p)| {
        p.property
            .as_ref()
            .is_some_and(|k| k.key == key && !k.retired)
    });
    let holding = || {
        cache.pages.iter().find(|(_, p)| {
            p.property
                .as_ref()
                .is_some_and(|k| !k.retired && k.items.contains_key(&wanted))
        })
    };
    own.or_else(holding)
        .map(|(page, _)| page_url(base, *page, id))
}

/// The URL of the page holding `id`.
pub fn url(project: &Project, state: Option<&Path>, id: &ItemId) -> Result<String, String> {
    let config = project
        .config
        .confluence
        .as_ref()
        .ok_or("kiln.toml has no [confluence]")?;
    let target = target(project).ok_or("kiln.toml has no Confluence export target")?;
    let item = project
        .item(id)
        .ok_or_else(|| format!("{id} doesn't exist"))?;
    let space = target
        .confluence
        .space
        .clone()
        .unwrap_or_else(|| config.space.clone());
    if let Some(page) = target.confluence.page {
        // An adopted page is known without asking.
        return Ok(page_url(&config.url, page, id));
    }
    if let Some(state) = state {
        let file = state.join("confluence").join(cache_file_name(
            &space,
            &project.config.name,
            &target.name,
        ));
        if let Some(url) = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| from_cache(&text, &config.url, id))
        {
            return Ok(url);
        }
    }
    let (token, _) = kiln_secrets::confluence_token_for(&config.url)?.ok_or_else(|| {
        format!(
            "{id}'s page isn't in this machine's page cache and there is no token to look it up (`kiln auth confluence`)"
        )
    })?;
    let user = kiln_secrets::confluence_user().unwrap_or_else(|| config.user.clone());
    let client = kiln_confluence::connect(config, &token, &user);
    for title in [format!("{id} {}", item.title), target.title.clone()] {
        if let Some(page) = client
            .find_title(&space, &title)
            .map_err(|e| e.to_string())?
        {
            return Ok(page_url(&config.url, page, id));
        }
    }
    Err(format!("{id} isn't published in space {space} yet"))
}
