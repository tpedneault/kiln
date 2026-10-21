//! Adding a preset to `kiln.toml` (DESIGN.md, "Presets"): text in, text
//! out, like the other edits. The file is spliced, not re-serialised, so
//! the author's comments and layout survive:
//!
//! - each preset type and link role the file doesn't declare is inserted
//!   after the file's last `[types.*]` (or `[links.*]`) table, with the
//!   preset's comment above it;
//! - a type the file already declares is kept as it is, even when it
//!   differs: the project's definition wins, and the preset's template
//!   uses no status names, so it works with either;
//! - a link role the file already declares gains the preset's `from` and
//!   `to` types it lacks, so the template's links stay valid;
//! - the export target is appended at the end.
//!
//! It refuses rather than guess: a prefix another type already has, an
//! export name already taken, or a result that `parse_config` finds new
//! errors in. Whether the files it would write already exist is the
//! caller's question, since this crate never touches the disk.

use kiln_model::{dir_prefix, Preset, Severity};
use toml_edit::{Array, DocumentMut, ImDocument, Item, Value};

use crate::config::parse_config;
use crate::edit::EditError;

/// What adding a preset does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetPlan {
    /// `kiln.toml` afterwards.
    pub config: String,
    /// Types inserted, in the preset's order.
    pub added_types: Vec<String>,
    /// Preset types the file already declared, left as they are.
    pub kept_types: Vec<String>,
    /// Link roles inserted.
    pub added_links: Vec<String>,
    /// Existing roles that gained types: (`role`, "from block, decision").
    pub extended_links: Vec<(String, String)>,
    /// The export target's name.
    pub export: String,
    /// Files to write: (path relative to the project, contents).
    pub files: Vec<(String, String)>,
}

/// One `[types.X]` or `[links.X]` table of a preset, with its comment.
#[derive(Debug)]
struct Chunk {
    table: &'static str,
    name: String,
    text: String,
}

/// Splits a preset's types text into its tables. A comment directly
/// above a header (no blank line between) belongs to that table; a
/// comment standing on its own, like the file's opening one, is dropped.
fn chunks(text: &str) -> Vec<Chunk> {
    let mut out: Vec<Chunk> = Vec::new();
    let mut current: Option<Chunk> = None;
    let mut pending = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(header) = trimmed.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            out.extend(current.take());
            let (table, name) = header.split_once('.').unwrap_or((header, ""));
            let table = if table == "types" { "types" } else { "links" };
            current = Some(Chunk {
                table,
                name: name.to_owned(),
                text: format!("{pending}{line}\n"),
            });
            pending.clear();
        } else if trimmed.is_empty() {
            out.extend(current.take());
            pending.clear();
        } else if trimmed.starts_with('#') && current.is_none() {
            pending.push_str(line);
            pending.push('\n');
        } else if let Some(c) = current.as_mut() {
            c.text.push_str(line);
            c.text.push('\n');
        }
    }
    out.extend(current);
    out
}

fn fill(text: &str, dir: &str, name: &str) -> String {
    let dir = match dir_prefix(dir) {
        "" => String::new(),
        d => format!("{d}/"),
    };
    let name = name.replace('\\', "\\\\").replace('"', "\\\"");
    text.replace("{dir}/", &dir).replace("{name}", &name)
}

fn strings(value: Option<&Item>) -> Vec<String> {
    value
        .and_then(Item::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The byte offset just after the last table of `[table.*]` in `text`
/// (before the comments and blank lines leading to the next header), or
/// `None` when there is no such table.
fn after_last(text: &str, table: &str) -> Option<usize> {
    let doc = ImDocument::parse(text).ok()?;
    // Every explicit table header, in file order: (start, is `table.*`).
    let mut headers: Vec<(usize, bool)> = Vec::new();
    fn walk(t: &toml_edit::Table, inside: bool, headers: &mut Vec<(usize, bool)>) {
        for (_, item) in t.iter() {
            match item {
                Item::Table(sub) => {
                    if let Some(span) = sub.span() {
                        headers.push((span.start, inside));
                    }
                    walk(sub, inside, headers);
                }
                Item::ArrayOfTables(array) => {
                    for sub in array.iter() {
                        if let Some(span) = sub.span() {
                            headers.push((span.start, inside));
                        }
                        walk(sub, inside, headers);
                    }
                }
                _ => {}
            }
        }
    }
    for (key, item) in doc.as_table().iter() {
        let inside = key == table;
        match item {
            Item::Table(t) => {
                if let Some(span) = t.span() {
                    headers.push((span.start, inside));
                }
                walk(t, inside, &mut headers);
            }
            Item::ArrayOfTables(array) => {
                for t in array.iter() {
                    if let Some(span) = t.span() {
                        headers.push((span.start, inside));
                    }
                    walk(t, inside, &mut headers);
                }
            }
            _ => {}
        }
    }
    headers.sort();
    let last = headers.iter().rposition(|(_, inside)| *inside)?;
    let Some(&(next, _)) = headers.get(last + 1) else {
        return Some(text.len());
    };
    // Back over the comments and blank lines that lead to the next
    // header: they are about it.
    let mut end = next;
    while end > 0 {
        let line_start = text[..end - 1].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = text[line_start..end].trim();
        if line.is_empty() || line.starts_with('#') {
            end = line_start;
        } else {
            break;
        }
    }
    Some(end)
}

/// Inserts `block` (tables, each ending in a newline) at `at`, with one
/// blank line before it.
fn insert(text: &str, at: usize, block: &str) -> String {
    let (head, tail) = text.split_at(at);
    let head = head.trim_end_matches('\n');
    let tail = tail.trim_start_matches('\n');
    let sep = if tail.is_empty() { "" } else { "\n" };
    format!("{head}\n\n{block}{sep}{tail}")
}

/// The error findings of `text`, without positions, to compare a file
/// before and after.
fn errors(text: &str) -> Vec<String> {
    let (_, findings) = parse_config(text);
    findings
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect()
}

/// Plans adding `preset` to the project whose `kiln.toml` is `text`
/// (already normalised).
pub fn add_preset(text: &str, preset: &Preset) -> Result<PresetPlan, EditError> {
    let (config, _) = parse_config(text);
    let config = config.ok_or_else(|| {
        EditError("kiln.toml can't be read; run `kiln check` and fix it first".into())
    })?;
    let dir = config.paths.first().map(String::as_str).unwrap_or("design");
    let doc: DocumentMut = text
        .parse()
        .map_err(|_| EditError("kiln.toml isn't valid TOML (run `kiln check`)".into()))?;
    let declared = |table: &str, name: &str| {
        doc.get(table)
            .and_then(Item::as_table_like)
            .is_some_and(|t| t.contains_key(name))
    };

    // The export name, before anything else: the clearest refusal.
    let export_text = fill(preset.export, dir, &config.name);
    let export_name = preset.name.to_owned();
    let taken = doc
        .get("export")
        .and_then(Item::as_array_of_tables)
        .is_some_and(|a| {
            a.iter()
                .any(|t| t.get("name").and_then(Item::as_str) == Some(export_name.as_str()))
        });
    if taken {
        return Err(EditError(format!(
            "kiln.toml already has an export named `{export_name}`; the {} preset is added already, or rename that export first",
            preset.name
        )));
    }

    let chunks: Vec<Chunk> = chunks(&fill(preset.types, dir, &config.name));
    let mut added_types = Vec::new();
    let mut kept_types = Vec::new();
    let mut type_block = String::new();
    for c in chunks.iter().filter(|c| c.table == "types") {
        if declared("types", &c.name) {
            kept_types.push(c.name.clone());
            continue;
        }
        let parsed: DocumentMut = c.text.parse().map_err(|e| {
            EditError(format!(
                "the {} preset's type `{}` isn't valid TOML: {e}",
                preset.name, c.name
            ))
        })?;
        let prefix = parsed["types"][c.name.as_str()]["prefix"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let owner = doc
            .get("types")
            .and_then(Item::as_table_like)
            .and_then(|types| {
                types.iter().find_map(|(name, t)| {
                    (t.get("prefix").and_then(Item::as_str) == Some(prefix.as_str()))
                        .then(|| name.to_owned())
                })
            });
        if let Some(owner) = owner {
            return Err(EditError(format!(
                "the {} preset's type `{}` uses the prefix {prefix}, which kiln.toml gives to `{owner}`; rename one of them, then add the preset",
                preset.name, c.name
            )));
        }
        if !type_block.is_empty() {
            type_block.push('\n');
        }
        type_block.push_str(&c.text);
        added_types.push(c.name.clone());
    }

    // Existing link roles first, through toml_edit (only the arrays they
    // gain change); then the new tables, spliced in.
    let mut doc = doc;
    let mut added_links = Vec::new();
    let mut extended_links = Vec::new();
    let mut link_block = String::new();
    for c in chunks.iter().filter(|c| c.table == "links") {
        let parsed: DocumentMut = c.text.parse().map_err(|e| {
            EditError(format!(
                "the {} preset's role `{}` isn't valid TOML: {e}",
                preset.name, c.name
            ))
        })?;
        let wanted = &parsed["links"][c.name.as_str()];
        let existing = doc
            .get_mut("links")
            .and_then(Item::as_table_like_mut)
            .and_then(|t| t.get_mut(&c.name))
            .and_then(Item::as_table_like_mut);
        let Some(existing) = existing else {
            if !link_block.is_empty() {
                link_block.push('\n');
            }
            link_block.push_str(&c.text);
            added_links.push(c.name.clone());
            continue;
        };
        let mut gained = Vec::new();
        for side in ["from", "to"] {
            let have = strings(existing.get(side));
            let missing: Vec<String> = strings(wanted.get(side))
                .into_iter()
                .filter(|t| !have.contains(t))
                .collect();
            if missing.is_empty() {
                continue;
            }
            match existing.get_mut(side).and_then(Item::as_array_mut) {
                Some(array) => {
                    for m in &missing {
                        array.push(m.as_str());
                    }
                }
                None => {
                    let array: Array = missing.iter().map(String::as_str).collect();
                    existing.insert(side, Item::Value(Value::Array(array)));
                }
            }
            gained.push(format!("{side} {}", missing.join(", ")));
        }
        if !gained.is_empty() {
            extended_links.push((c.name.clone(), gained.join("; ")));
        }
    }
    let mut out = doc.to_string();

    if !type_block.is_empty() {
        let at = after_last(&out, "types").unwrap_or(out.len());
        out = insert(&out, at, &type_block);
    }
    if !link_block.is_empty() {
        let at = after_last(&out, "links")
            .or_else(|| after_last(&out, "types"))
            .unwrap_or(out.len());
        out = insert(&out, at, &link_block);
    }
    let out = format!("{}\n\n{export_text}", out.trim_end_matches('\n'));

    let before = errors(text);
    if let Some(new) = errors(&out).into_iter().find(|e| !before.contains(e)) {
        return Err(EditError(format!(
            "adding the {} preset would make kiln.toml invalid ({new}); nothing was changed",
            preset.name
        )));
    }

    let mut files = vec![(preset.template_path(dir), preset.template.to_owned())];
    for t in preset.item_templates {
        if added_types.iter().any(|a| a == t.type_name) {
            let path = match dir_prefix(dir) {
                "" => format!("templates/{}", t.file),
                d => format!("{d}/templates/{}", t.file),
            };
            files.push((path, t.text.to_owned()));
        }
    }
    Ok(PresetPlan {
        config: out,
        added_types,
        kept_types,
        added_links,
        extended_links,
        export: export_name,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_model::{default_config, preset, PRESETS};

    fn arc42() -> &'static Preset {
        preset("arc42").unwrap()
    }

    #[test]
    fn every_preset_merges_into_the_default_config_and_parses_clean() {
        for p in PRESETS {
            let plan = add_preset(&default_config("Flint"), p).unwrap();
            let (config, findings) = parse_config(&plan.config);
            assert!(findings.is_empty(), "{}: {findings:?}", p.name);
            let config = config.unwrap();
            let target = config.export(p.name).unwrap();
            assert_eq!(target.template.as_deref(), Some(plan.files[0].0.as_str()));
            assert_eq!(plan.files[0].0, format!("design/templates/{}.md", p.name));
            assert!(target.title.starts_with("Flint — "), "{}", target.title);
            for t in plan.added_types.iter().chain(&plan.kept_types) {
                assert!(config.types.iter().any(|d| &d.name == t), "{}: {t}", p.name);
            }
            // The original text is all still there, in order (a role's
            // arrays may have grown).
            let mut rest = plan.config.as_str();
            for line in default_config("Flint")
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with("from =") && !l.starts_with("to ="))
            {
                let at = rest
                    .find(line)
                    .unwrap_or_else(|| panic!("{}: lost {line:?}", p.name));
                rest = &rest[at + line.len()..];
            }
        }
    }

    #[test]
    fn arc42_adds_its_types_after_the_last_type_and_extends_shared_roles() {
        let plan = add_preset(&default_config("Flint"), arc42()).unwrap();
        assert_eq!(
            plan.added_types,
            [
                "quality-goal",
                "quality-scenario",
                "constraint",
                "block",
                "term"
            ]
        );
        assert_eq!(plan.kept_types, ["decision", "risk"]);
        assert_eq!(plan.added_links, ["addresses"]);
        assert_eq!(
            plan.extended_links,
            [("mitigates".to_owned(), "from decision, block".to_owned())]
        );
        // New types come right after [types.risk], before the links, each
        // with its comment.
        let risk = plan.config.find("[types.risk]").unwrap();
        let goal = plan.config.find("[types.quality-goal]").unwrap();
        let links = plan.config.find("[links.implements]").unwrap();
        assert!(risk < goal && goal < links);
        assert!(plan
            .config
            .contains("closed = [\"closed\"]\n\n# Section 1.2: the three to five quality goals"));
        assert!(plan
            .config
            .contains("template = \"design/templates/quality-scenario.md\""));
        // The new role after the last role; the export at the end.
        let depends = plan.config.find("[links.depends-on]").unwrap();
        let addresses = plan.config.find("[links.addresses]").unwrap();
        let export = plan.config.find("[[export]]").unwrap();
        assert!(depends < addresses && addresses < export);
        assert!(plan.config.contains("title = \"Flint — architecture\""));
        assert_eq!(
            plan.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
            [
                "design/templates/arc42.md",
                "design/templates/quality-scenario.md",
                "design/templates/building-block.md"
            ]
        );
    }

    #[test]
    fn comments_stay_with_the_table_they_are_about() {
        let text = "format = 1\nname = \"X\"\npaths = [\"docs/design\"]\n\n[types.note]\nprefix = \"N\" # notes\nstatuses = [\"open\"]\n\n# Severity overrides.\n[check]\nK105 = \"off\"\n";
        let plan = add_preset(text, preset("srs").unwrap()).unwrap();
        let note = plan.config.find("[types.note]").unwrap();
        let req = plan.config.find("[types.requirement]").unwrap();
        let check = plan.config.find("# Severity overrides.\n[check]").unwrap();
        assert!(note < req && req < check, "{}", plan.config);
        assert!(plan.config.contains("prefix = \"N\" # notes"));
        // No [links.*] yet: the roles go after the types.
        let verifies = plan.config.find("[links.verifies]").unwrap();
        assert!(req < verifies && verifies < check, "{}", plan.config);
        assert!(plan
            .config
            .contains("template = \"docs/design/templates/requirement.md\""));
        assert_eq!(plan.files[0].0, "docs/design/templates/srs.md");
        assert!(parse_config(&plan.config).1.is_empty());
    }

    #[test]
    fn refuses_a_taken_prefix_or_export_name_and_twice() {
        let text = format!(
            "{}\n[types.goal]\nprefix = \"QG\"\nstatuses = [\"open\"]\n",
            default_config("Flint")
        );
        let e = add_preset(&text, arc42()).unwrap_err();
        assert_eq!(
            e.0,
            "the arc42 preset's type `quality-goal` uses the prefix QG, which kiln.toml gives to `goal`; rename one of them, then add the preset"
        );
        let once = add_preset(&default_config("Flint"), arc42()).unwrap();
        let e = add_preset(&once.config, arc42()).unwrap_err();
        assert!(
            e.0.starts_with("kiln.toml already has an export named `arc42`"),
            "{e}"
        );
        // Two presets side by side: shared types are kept.
        let both = add_preset(&once.config, preset("srs").unwrap()).unwrap();
        assert!(both.kept_types.contains(&"constraint".to_owned()));
        assert!(parse_config(&both.config).1.is_empty());
    }

    /// Every `[types.*]` and `[links.*]` header of `text`, and every
    /// export name.
    fn headers(text: &str) -> Vec<String> {
        text.lines()
            .filter(|l| {
                l.starts_with("[types.") || l.starts_with("[links.") || l.starts_with("name = ")
            })
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn several_presets_in_any_order_declare_nothing_twice() {
        let names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
        let orders = [
            [names[0], names[1], names[2]],
            [names[0], names[2], names[1]],
            [names[1], names[0], names[2]],
            [names[1], names[2], names[0]],
            [names[2], names[0], names[1]],
            [names[2], names[1], names[0]],
        ];
        for order in orders {
            let mut text = default_config("Flint");
            for name in order {
                text = add_preset(&text, preset(name).unwrap()).unwrap().config;
            }
            let (config, findings) = parse_config(&text);
            assert!(findings.is_empty(), "{order:?}: {findings:?}");
            let mut seen = std::collections::HashSet::new();
            for h in headers(&text) {
                assert!(seen.insert(h.clone()), "{order:?}: {h} twice\n{text}");
            }
            let config = config.unwrap();
            for role in &config.links {
                for side in [&role.from, &role.to] {
                    let mut unique = side.clone();
                    unique.sort();
                    unique.dedup();
                    assert_eq!(
                        unique.len(),
                        side.len(),
                        "{order:?}: {} {side:?}",
                        role.name
                    );
                }
            }
            for name in order {
                assert!(config.export(name).is_some(), "{order:?}: export {name}");
            }
        }
    }

    #[test]
    fn items_at_the_root_and_a_name_with_quotes() {
        let text = "format = 1\nname = \"The \\\"best\\\" one\"\npaths = [\".\"]\n\n[types.requirement]\nprefix = \"REQ\"\nstatuses = [\"draft\"]\n";
        let plan = add_preset(text, arc42()).unwrap();
        assert_eq!(plan.files[0].0, "templates/arc42.md");
        let (config, findings) = parse_config(&plan.config);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(
            config.unwrap().export("arc42").unwrap().title,
            "The \"best\" one — architecture"
        );
    }
}
