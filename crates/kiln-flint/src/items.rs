//! What Flint shows: one static item per open item, and the drill-down
//! results (`status ID`, `impact ID`, `all text`). Pure: a function of the
//! project and where it lives, so the item list is tested (and measured)
//! without a Flint.

use std::path::Path;

use flint_protocol::item::{Action, ActionKind, Item, MAX_TITLE};
use kiln_index::{Project, Source};
use kiln_model::ItemId;
use serde_json::json;

/// What items need besides the project.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The provider id, which prefixes every item id.
    pub provider: &'a str,
    /// The scope keyword, for drill-down queries (`k status REQ-014 `).
    pub prefix: &'a str,
    /// The project root, for absolute paths.
    pub root: &'a Path,
    /// A Confluence target exists: offer "Open in Confluence".
    pub confluence: bool,
}

/// `kiln:<project name>:<ID>`: stable across sessions, for Flint's
/// frecency.
pub fn item_id(ctx: &Context, project: &Project, id: &ItemId) -> String {
    format!("{}:{}:{id}", ctx.provider, project.config.name)
}

/// What an item id names, for `execute`: the ID and the rest (`status:verified`).
pub fn parse_item_id<'s>(
    ctx: &Context,
    project: &Project,
    item_id: &'s str,
) -> Option<(ItemId, &'s str)> {
    let rest = item_id.strip_prefix(&format!("{}:{}:", ctx.provider, project.config.name))?;
    let (id, tail) = rest.split_once(':').unwrap_or((rest, ""));
    Some((id.parse().ok()?, tail))
}

fn action(id: &str, title: &str, key: char, kind: ActionKind) -> Action {
    Action {
        id: id.to_owned(),
        title: title.to_owned(),
        key: Some(key.to_string()),
        kind,
    }
}

fn cap(title: String) -> String {
    if title.chars().count() <= MAX_TITLE {
        title
    } else {
        let mut t: String = title.chars().take(MAX_TITLE - 1).collect();
        t.push('…');
        t
    }
}

fn icon(type_name: &str) -> &'static str {
    match type_name {
        "task" | "milestone" => "task",
        "decision" | "design" => "note",
        _ => "ticket",
    }
}

/// Opening `rel` in Fenix at `line`.
fn open_in_fenix(ctx: &Context, rel: &str, line: u32) -> Action {
    // The OS's separators throughout: `C:\p\design\x.md`, not a mix.
    let path = ctx
        .root
        .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
        .to_string_lossy()
        .into_owned();
    action(
        "open-in-fenix",
        "Open in Fenix",
        'e',
        ActionKind::Fenix {
            method: "fenix/open".into(),
            params: json!({ "paths": [{ "path": path, "line": line }] }),
        },
    )
}

/// The actions every item has; the first is `Enter`'s.
fn actions(ctx: &Context, project: &Project, item: &kiln_model::Item) -> Vec<Action> {
    let id = &item.id;
    let mut out = vec![
        open_in_fenix(ctx, &item.path, item.title_span.line.max(1)),
        action(
            "copy-id",
            "Copy ID",
            'c',
            ActionKind::Copy {
                text: id.to_string(),
            },
        ),
        action(
            "copy-reference",
            &format!("Copy reference [[{id}]]"),
            'r',
            ActionKind::Copy {
                text: format!("[[{id}]]"),
            },
        ),
        action(
            "status",
            "Set status…",
            's',
            ActionKind::Query {
                text: format!("{} status {id} ", ctx.prefix),
            },
        ),
        action(
            "impact",
            "Show what depends on it",
            'i',
            ActionKind::Query {
                text: format!("{} impact {id} ", ctx.prefix),
            },
        ),
    ];
    // The GUI, at this item; `kilnw` on Windows opens it without a console.
    let program = if cfg!(windows) { "kilnw" } else { "kiln" };
    let argv = vec![
        program.to_owned(),
        "gui".to_owned(),
        "--project".to_owned(),
        ctx.root.to_string_lossy().into_owned(),
        "--item".to_owned(),
        id.to_string(),
    ];
    out.push(action(
        "open-in-kiln",
        "Open in Kiln",
        'g',
        ActionKind::Run {
            argv,
            cwd: None,
            terminal: false,
        },
    ));
    if ctx.confluence {
        out.push(action(
            "open-confluence",
            "Open in Confluence",
            'w',
            ActionKind::Provider,
        ));
    }
    if let (Some(key), Some(url)) = (item.jira.first(), project.config.jira_url.as_deref()) {
        out.push(action(
            "open-jira",
            &format!("Open {key} in Jira"),
            'j',
            ActionKind::Open {
                target: format!("{}/browse/{key}", url.trim_end_matches('/')),
            },
        ));
    }
    if let Some(key) = item.jira.first() {
        out.push(action(
            "clock-in",
            &format!("Start working on it (clock in on {key})"),
            'k',
            ActionKind::Provider,
        ));
    }
    out
}

fn subtitle(item: &kiln_model::Item) -> String {
    let mut s = format!("{} · {}", item.type_name, item.status);
    if let Some(Ok(parent)) = item.parent.as_ref().map(|p| &p.id) {
        s.push_str(&format!(" · under {parent}"));
    }
    s
}

fn item(
    ctx: &Context,
    project: &Project,
    item: &kiln_model::Item,
    title: String,
    id: String,
) -> Item {
    let mut keywords = item.tags.clone();
    keywords.extend(item.jira.iter().cloned());
    Item {
        id,
        title: cap(title),
        subtitle: Some(subtitle(item)),
        icon: Some(icon(&item.type_name).to_owned()),
        keywords,
        actions: actions(ctx, project, item),
    }
}

/// One static item per item that isn't closed (closed ones are behind
/// `all`), in the project's order. A duplicated ID gives one item.
// implements: REQ-009
pub fn items(project: &Project, ctx: &Context) -> Vec<Item> {
    let mut seen = std::collections::HashSet::new();
    project
        .items
        .iter()
        .filter(|i| !project.is_closed(i) && seen.insert(&i.id))
        .map(|i| {
            item(
                ctx,
                project,
                i,
                format!("{} {}", i.id, i.title),
                item_id(ctx, project, &i.id),
            )
        })
        .collect()
}

/// The answer to a scoped query, when it is a drill-down: `status ID`,
/// `impact ID` or `all [text]`. Titles start with the query's words, so
/// they match it and rank above the static items it also matches.
pub fn drilldown(project: &Project, text: &str, ctx: &Context) -> Option<Vec<Item>> {
    let mut words = text.split_whitespace();
    let verb = words.next()?;
    let arg = words.next();
    match verb {
        "status" => {
            let id: ItemId = arg?.parse().ok()?;
            let it = project.item(&id)?;
            let ty = project.type_of(it)?;
            Some(
                ty.statuses
                    .iter()
                    .map(|s| Item {
                        id: format!("{}:status:{s}", item_id(ctx, project, &id)),
                        title: format!("status {id} → {s}"),
                        subtitle: Some(if *s == it.status {
                            format!("{} · current status", it.title)
                        } else {
                            format!("{} · now {}", it.title, it.status)
                        }),
                        icon: Some(icon(&it.type_name).to_owned()),
                        keywords: Vec::new(),
                        actions: vec![action(
                            "set-status",
                            &format!("Set status to {s}"),
                            's',
                            ActionKind::Provider,
                        )],
                    })
                    .collect(),
            )
        }
        "impact" => {
            let id: ItemId = arg?.parse().ok()?;
            project.item(&id)?;
            let mut out = Vec::new();
            for impact in project.impact(&id) {
                let how: Vec<String> = impact
                    .path
                    .iter()
                    .map(|s| format!("{} {}", s.relation, s.to))
                    .collect();
                match impact.source {
                    Source::Item(i) => {
                        let it = &project.items[i];
                        let mut x = item(
                            ctx,
                            project,
                            it,
                            format!("impact {id} ← {} {}", it.id, it.title),
                            format!("{}:impact:{}", item_id(ctx, project, &id), it.id),
                        );
                        x.subtitle = Some(format!("{} · {}", subtitle(it), how.join(", ")));
                        out.push(x);
                    }
                    Source::Code(c) => {
                        let code = &project.code_refs[c];
                        let at = code.location();
                        out.push(Item {
                            id: format!("{}:impact:{at}", item_id(ctx, project, &id)),
                            title: cap(format!("impact {id} ← {at}")),
                            subtitle: Some(format!("code · {}", how.join(", "))),
                            icon: Some("file".into()),
                            keywords: Vec::new(),
                            actions: vec![
                                open_in_fenix(ctx, &code.path, code.target.span.line),
                                action(
                                    "copy-location",
                                    "Copy location",
                                    'c',
                                    ActionKind::Copy { text: at.clone() },
                                ),
                            ],
                        });
                    }
                }
            }
            Some(out)
        }
        "all" => {
            let needle = text[text.find("all").map_or(0, |i| i + 3)..]
                .trim()
                .to_lowercase();
            let mut seen = std::collections::HashSet::new();
            Some(
                project
                    .items
                    .iter()
                    .filter(|i| seen.insert(&i.id))
                    .filter(|i| {
                        format!("{} {}", i.id, i.title)
                            .to_lowercase()
                            .contains(&needle)
                    })
                    .take(flint_protocol::provider::MAX_BATCH)
                    .map(|i| {
                        item(
                            ctx,
                            project,
                            i,
                            format!("all {} {}", i.id, i.title),
                            format!("{}:all", item_id(ctx, project, &i.id)),
                        )
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}
