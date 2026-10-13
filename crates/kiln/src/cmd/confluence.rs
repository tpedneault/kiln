//! `kiln confluence comments [EXPORT…]`: the unresolved footer comments
//! on each Confluence target's pages, per item where Kiln can tell
//! (CONFLUENCE.md, "Comments"). Read-only; answering happens in
//! Confluence.

use std::path::Path;

use kiln_confluence::{cache_file_name, take_snapshot, PageCache};
use kiln_model::{ExportFormat, ExportTarget};
use serde_json::json;

use super::json_out;
use crate::project::{folders, load};
use crate::{Fail, TextOrJson, EXIT_NETWORK};

pub fn comments(
    project: Option<&Path>,
    names: Vec<String>,
    format: TextOrJson,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let config = p
        .config
        .confluence
        .as_ref()
        .ok_or_else(|| Fail::usage("kiln.toml has no [confluence]"))?;
    let all: Vec<&ExportTarget> = p
        .config
        .exports
        .iter()
        .filter(|t| t.format == ExportFormat::Confluence)
        .collect();
    let targets: Vec<&ExportTarget> = if names.is_empty() {
        all.clone()
    } else {
        names
            .iter()
            .map(|n| {
                all.iter().copied().find(|t| &t.name == n).ok_or_else(|| {
                    let known: Vec<&str> = all.iter().map(|t| t.name.as_str()).collect();
                    Fail::usage(format!("no Confluence target `{n}` ({})", known.join(", ")))
                })
            })
            .collect::<Result<_, _>>()?
    };
    if targets.is_empty() {
        return Err(Fail::usage(
            "kiln.toml declares no Confluence export target",
        ));
    }
    let (token, _) = kiln_secrets::confluence_token_for(&config.url)
        .map_err(Fail::usage)?
        .ok_or_else(|| {
            Fail::usage(format!(
                "no token for {}: run `kiln auth confluence`, or set {}",
                config.url,
                kiln_secrets::CONFLUENCE_TOKEN
            ))
        })?;
    let user = kiln_secrets::confluence_user().unwrap_or_else(|| config.user.clone());
    let client = kiln_confluence::connect(config, &token, &user);
    let mut json_targets = Vec::new();
    for t in targets {
        let space = t
            .confluence
            .space
            .clone()
            .unwrap_or_else(|| config.space.clone());
        let cache = folders()
            .map(|f| {
                f.state
                    .join("confluence")
                    .join(cache_file_name(&space, &p.config.name, &t.name))
            })
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| PageCache::parse(&text))
            .unwrap_or_default();
        let snapshot = take_snapshot(client.as_ref(), t, &p.config.name, &space, &[], &cache)
            .map_err(|e| Fail::new(EXIT_NETWORK, e.to_string()))?;
        if snapshot.root.is_none() {
            if format == TextOrJson::Text {
                println!("{} → space {space}: not published yet", t.name);
            }
            continue;
        }
        let placed = kiln_confluence::comments::unresolved(
            client.as_ref(),
            &snapshot,
            &config.url,
            &p.config.name,
            &t.name,
        )
        .map_err(|e| Fail::new(EXIT_NETWORK, e.to_string()))?;
        if format == TextOrJson::Json {
            json_targets.push(json!({ "name": t.name, "space": space, "comments": placed }));
            continue;
        }
        println!(
            "{} → space {space}: {} unresolved comment{}",
            t.name,
            placed.len(),
            if placed.len() == 1 { "" } else { "s" }
        );
        for c in &placed {
            let about = c.item.clone().unwrap_or_else(|| "(page)".to_owned());
            let date = c.created.get(..10).unwrap_or(&c.created);
            println!("  {about:9} {}, {date}: {}", c.author, c.text);
            if c.item.is_none() {
                println!("  {:9} on \"{}\"", "", c.page_title);
            }
            println!("  {:9} {}", "", c.url);
        }
    }
    if format == TextOrJson::Json {
        print!("{}", json_out(&json!(json_targets)));
    }
    Ok(0)
}
