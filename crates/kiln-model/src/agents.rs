//! The agent guide (DESIGN.md, "The agent guide"): `AGENTS.md` in the
//! first item folder, telling a coding agent how this project's design is
//! organised and how to change it with Kiln rather than by hand.
//!
//! Two parts. The rules, written once by `kiln init` or `kiln agents` and
//! the project's own from then on, like a preset's template (ADR-015).
//! And a section between two marker lines, generated from `kiln.toml`
//! alone (the types, folders, link roles, levels and exports), which
//! `kiln agents` rewrites and `kiln check` compares (K113), so it never
//! drifts from the configuration it describes. Nothing in the generated
//! part depends on the items, the clock or the commit: the same
//! `kiln.toml` gives the same bytes, so the check is exact and the file
//! changes only when the configuration does.
//!
//! Pure text in, text out; reading and writing the file is the binary's.

use crate::config::{Config, CODE};
use crate::export::ExportFormat;

/// The file's name, in the first of `paths`.
pub const AGENTS_FILE: &str = "AGENTS.md";

const BEGIN: &str = "<!-- kiln:agents begin";
const END: &str = "<!-- kiln:agents end -->";

/// Where the guide lives: `AGENTS.md` in the first item folder, so it
/// sits beside the files it describes and never takes the repository's
/// own `AGENTS.md`, which agents read for the code (ADR-017).
pub fn agents_path(config: &Config) -> String {
    match config.paths.first().map(String::as_str) {
        None | Some(".") => AGENTS_FILE.to_owned(),
        Some(dir) => format!("{dir}/{AGENTS_FILE}"),
    }
}

/// Why an existing guide can't be refreshed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentsError {
    /// A begin marker without an end marker after it.
    Unclosed { line: usize },
}

impl std::fmt::Display for AgentsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentsError::Unclosed { line } => write!(
                f,
                "line {line} opens Kiln's section (`{BEGIN} …`) but no `{END}` closes it; restore the end line or delete both"
            ),
        }
    }
}

/// The guide with its generated section current. `existing` is the file's
/// text normalised to `\n` line endings, `None` when there is none. A
/// file without markers keeps its text and gains the section at its end;
/// with markers, only what is between them (the markers included) is
/// replaced.
pub fn write_agents(config: &Config, existing: Option<&str>) -> Result<String, AgentsError> {
    let section = agents_section(config);
    let Some(text) = existing else {
        return Ok(format!("{}\n{section}", intro(config)));
    };
    match markers(text)? {
        Some((start, end)) => Ok(format!("{}{section}{}", &text[..start], &text[end..])),
        None if text.trim().is_empty() => Ok(format!("{}\n{section}", intro(config))),
        None => Ok(format!("{}\n\n{section}", text.trim_end())),
    }
}

/// Whether the generated section of `text` matches `config`: `None` when
/// the file has no section (it is the project's to leave out), else
/// `Some(current)`.
pub fn agents_current(config: &Config, text: &str) -> Result<Option<bool>, AgentsError> {
    Ok(markers(text)?.map(|(start, end)| text[start..end] == agents_section(config)))
}

/// The byte range of the section, from the start of the begin line to
/// past the end line's newline.
fn markers(text: &str) -> Result<Option<(usize, usize)>, AgentsError> {
    let mut offset = 0;
    let mut begin = None;
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_end();
        match begin {
            None if trimmed.starts_with(BEGIN) => begin = Some((offset, n + 1)),
            Some((start, _)) if trimmed == END => {
                return Ok(Some((start, offset + line.len())));
            }
            _ => {}
        }
        offset += line.len();
    }
    match begin {
        Some((_, line)) => Err(AgentsError::Unclosed { line }),
        None => Ok(None),
    }
}

/// The line to add to the repository's own `AGENTS.md` or `CLAUDE.md`, so
/// an agent working on the code learns the guide exists: one in the
/// design folder is read only by agents that look there (ADR-017).
pub fn agents_pointer(config: &Config) -> String {
    format!(
        "The design (requirements, decisions, milestones and tasks) is in {}, kept with Kiln; read `{}` before changing it or implementing what it describes, and `kiln next` lists the work ready to start.",
        folder(config),
        agents_path(config)
    )
}

/// The rules: written once, then the project's.
fn intro(config: &Config) -> String {
    let folder = folder(config);
    format!(
        r#"# Working on {name}'s design

This folder is {name}'s design, kept with [Kiln](https://github.com/tpedneault/kiln):
requirements, decisions and the rest as Markdown files, one item per
file, checked in CI by `kiln check`. If you are a coding agent, read
this before changing a file here or implementing what one describes.
This part is the project's to edit; the section below it is generated.

## Rules

- **Read with Kiln.** `kiln show ID` prints an item, its body and every
  link in and out (`--format json` for all of it as data); `kiln list
  --format json --body` loads every item at once; `kiln impact ID` shows
  everything that depends on one; `kiln trace` shows what is implemented
  and verified, and the gaps.
- **Take work from the plan.** `kiln next --unowned` lists the work
  ready to start that nobody has claimed, earliest milestone first, and
  `kiln status` the plan by milestone. When you take an item, claim it
  and move it past its first status in one command (`kiln set ID
  owner=… status=…`), so another agent or person doesn't take it too;
  when it is finished and `kiln check` is clean, set it to a done status.
  Replace the template's prompts in a body with what the item says
  (`kiln check` reports them, K215).
- **Create and change front matter with Kiln, not by hand.** `kiln new
  TYPE --title "…" [--parent ID] [--link ROLE=ID]… [--tag TAG]…
  [--body-file FILE|-]` allocates the next free ID and writes the item
  whole, then prints its path (`--format json` for its ID too): never
  choose an ID yourself. `kiln set ID status=…` (also `title`, `parent`,
  `owner`, `due`, `tags=a,b`, `jira=KEY`), `kiln link ID ROLE TARGET`
  and `kiln unlink ID ROLE TARGET` keep the file's comments and layout
  and refuse when the file changed under them. Write or change the body
  (everything after the second `+++`) with any editor.
- **Link once, from the source side.** A test says `verifies =
  ["REQ-014"]`; the requirement never says "verified by", which Kiln
  computes. Mention an item in prose as `[[REQ-014]]`; a mention is not
  a link and counts toward nothing.
- **Leave sign-off to people.** Moving a requirement to approved,
  accepting a decision, closing a risk or finishing a milestone is a
  person's call: propose it in the pull request. Moving work forward (a
  task to done, a test to passing when it passes) is fine.
- **When you implement or test a requirement, say so in the code.** A
  comment `implements: REQ-014` near the code and `verifies: REQ-014`
  on its test (see "Links" below for the roles code may use) make the
  code part of the trace. Before you open a pull request, `kiln
  affected origin/main` lists the requirements your change touches and
  the tests that verify them; run those.
- **Keep `kiln check` clean.** Run it before committing; errors fail CI.
  Fix what it reports rather than lowering a severity in `[check]`.
- **Don't edit generated files.** Exports listed below are written by
  `kiln export`; change the items or the template and run it again.
- **Two branches, one ID:** after a merge, `kiln check` reports the
  duplicate (K101) and `kiln renumber ID --to NEW` moves one copy and
  every reference to it.

Item files are Markdown with a TOML front matter block between two
`+++` lines; files here without one, like this one, aren't items. The
format is docs/FORMAT.md in Kiln's repository. After changing
`kiln.toml`, run `kiln agents` to bring the section below up to date
(`kiln check` says when it isn't; K113). Everything Kiln keeps lives
under {folder}.
"#,
        name = config.name
    )
}

fn folder(config: &Config) -> String {
    match config.paths.first().map(String::as_str) {
        None | Some(".") => "the project root".to_owned(),
        Some(dir) => format!("`{dir}/`"),
    }
}

/// The generated section, markers included, ending with a newline.
// implements: REQ-019
pub fn agents_section(config: &Config) -> String {
    let mut out = String::new();
    let mut line = |s: &str| {
        out.push_str(s);
        out.push('\n');
    };
    line(&format!(
        "{BEGIN}: written by `kiln agents` from kiln.toml; change kiln.toml and run `kiln agents`, not this section -->"
    ));
    line("## How this project's design is organised");
    line("");
    let first = config.paths.first().map(String::as_str).unwrap_or("design");
    let dir = |sub: &str| {
        if first == "." {
            format!("{sub}/")
        } else {
            format!("{first}/{sub}/")
        }
    };
    let roots = config
        .paths
        .iter()
        .map(|p| format!("`{p}/`"))
        .collect::<Vec<_>>()
        .join(", ");
    line(&format!(
        "Items are the `.md` files under {roots} that start with `+++`. `kiln new` files each type in its own folder, below; folders carry no meaning to Kiln, and an item's ID is the `id` in its front matter, not its file name."
    ));
    line("");
    line("### Types");
    line("");
    line("| Type | ID | Folder | Statuses | Parent |");
    line("|---|---|---|---|---|");
    for t in &config.types {
        let statuses: Vec<String> = t
            .statuses
            .iter()
            .map(|s| {
                let mut marks = Vec::new();
                if *s == t.initial {
                    marks.push("new");
                }
                if t.is_done(s) {
                    marks.push("done");
                }
                if t.is_closed(s) {
                    marks.push("closed");
                }
                if marks.is_empty() {
                    s.clone()
                } else {
                    format!("{s} ({})", marks.join(", "))
                }
            })
            .collect();
        let parents = if t.parents.is_empty() {
            "—".to_owned()
        } else {
            t.parents.join(", ")
        };
        line(&format!(
            "| {} ({}) | `{}-001` | `{}` | {} | {} |",
            t.name,
            t.title,
            t.prefix,
            dir(&t.folder()),
            statuses.join(", "),
            parents
        ));
    }
    let templates: Vec<String> = config
        .types
        .iter()
        .filter_map(|t| {
            t.template
                .as_ref()
                .map(|p| format!("{} from `{p}`", t.name))
        })
        .collect();
    if !templates.is_empty() {
        line("");
        line(&format!(
            "`kiln new` starts the body of {}.",
            templates.join(", ")
        ));
    }
    line("");
    line("\"new\" is the status `kiln new` writes; \"done\" counts toward progress; \"closed\" items are left out of coverage and progress (prefer closing an item to deleting its file).");
    line("");
    line("### Links");
    line("");
    line("Written in the source item's front matter, `[links]` table, as `role = [\"ID\", …]`; the inverse is computed, never written.");
    line("");
    line("| Role | From | To | Read from the target as |");
    line("|---|---|---|---|");
    for r in &config.links {
        let from: Vec<String> = r
            .from
            .iter()
            .map(|f| {
                if f == CODE {
                    "code (a comment)".to_owned()
                } else {
                    f.clone()
                }
            })
            .collect();
        line(&format!(
            "| `{}` | {} | {} | {} |",
            r.name,
            from.join(", "),
            r.to.join(", "),
            r.inverse
        ));
    }
    let code_roles: Vec<&str> = config
        .links
        .iter()
        .filter(|r| r.allows_from(CODE))
        .map(|r| r.name.as_str())
        .collect();
    line("");
    if config.code.paths.is_empty() || code_roles.is_empty() {
        line("No source code is scanned for references (`[code] paths` in kiln.toml).");
    } else {
        let paths = config
            .code
            .paths
            .iter()
            .map(|p| format!("`{p}/`"))
            .collect::<Vec<_>>()
            .join(", ");
        let roles = code_roles
            .iter()
            .map(|r| format!("`{r}: ID`"))
            .collect::<Vec<_>>()
            .join(" or ");
        if config.code.patterns.is_empty() {
            line(&format!(
                "Code under {paths} links to items with a comment naming the role and IDs: {roles} (several IDs separated by commas), on the line before the code or test it is about."
            ));
        } else {
            line(&format!(
                "Code under {paths} links to items with comments matching `[code] patterns` in kiln.toml ({}).",
                code_roles.join(", ")
            ));
        }
    }
    let levels = config.levels();
    if let Some(trace) = config.trace() {
        line("");
        line("### Requirement levels");
        line("");
        line(&format!(
            "Requirements are written at {} levels, highest first: {}. `parent` breaks a requirement down within its level; `{}` links a requirement to the one it answers at the level directly above, and a child traces through its parent. A requirement that answers nothing above it, on purpose, has `derived = true` in its front matter and says why in its body. `kiln trace --levels` shows the chains.",
            levels.len(),
            levels.join(" → "),
            trace.name
        ));
    }
    let coverage: Vec<String> = config
        .coverage_roles
        .iter()
        .filter_map(|r| config.role(r))
        .map(|r| format!("`{}` (to {})", r.name, r.to.join(", ")))
        .collect();
    if !coverage.is_empty() && !levels.is_empty() {
        line("");
        line("### Coverage");
        line("");
        line(&format!(
            "{} past its first status is expected to be covered by {}, each where its `to` allows: `kiln check` warns about the gaps (K201, K202{}), and `kiln trace` lists them.",
            match levels.as_slice() {
                [one] => format!("A {one}"),
                all => format!("A requirement of any level ({})", all.join(", ")),
            },
            coverage.join(", "),
            if config.trace().is_some() { ", K208–K210" } else { "" }
        ));
    }
    if let Some(part_of) = config.role("part-of") {
        let work: Vec<&str> = part_of
            .from
            .iter()
            .map(String::as_str)
            .filter(|t| *t != CODE)
            .collect();
        if !work.is_empty() {
            line("");
            line("### Planning");
            line("");
            let a = |names: &[&str]| -> String {
                names
                    .iter()
                    .map(|n| format!("a {n}"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            };
            let to: Vec<&str> = part_of.to.iter().map(String::as_str).collect();
            let ordered = config.role("depends-on").is_some();
            line(&format!(
                "Work is {} `part-of` {}{}. `kiln next` lists the work ready to start: not done or closed, not `blocked`{}, earliest milestone first; `kiln status` shows the plan by milestone, with what waits on what. Claim work by setting its `owner` when you take it; `kiln next --unowned` leaves claimed work out.",
                a(&work),
                a(&to),
                if ordered { ", ordered by `depends-on`" } else { "" },
                if ordered { ", and nothing it `depends-on` still open" } else { "" },
            ));
            let finishing: Vec<String> = work
                .iter()
                .filter_map(|w| config.type_named(w))
                .filter(|t| !t.done.is_empty())
                .map(|t| format!("{} {}", t.name, t.done.join(" or ")))
                .collect();
            if !finishing.is_empty() {
                line("");
                line(&format!(
                    "Finished work has a done status ({}); `kiln check` warns when the plan contradicts itself: a done milestone with open work (K211), done work whose dependencies aren't (K212), open work in no milestone (K213), a requirement implemented or verified while work implementing it is open (K214).",
                    finishing.join("; ")
                ));
            }
        }
    }
    let generated: Vec<String> = config
        .exports
        .iter()
        .map(|e| {
            let to = match (e.format, &e.path) {
                (ExportFormat::Confluence, _) => "Confluence".to_owned(),
                (_, Some(path)) => format!("`{path}`"),
                (_, None) => "—".to_owned(),
            };
            let template = e
                .template
                .as_ref()
                .map(|t| format!("`{t}`"))
                .unwrap_or_else(|| "none".to_owned());
            format!(
                "| `{}` | {} | {to} | {template} |",
                e.name,
                e.format.as_str()
            )
        })
        .collect();
    if !generated.is_empty() {
        line("");
        line("### Exports");
        line("");
        line("Written by `kiln export NAME`; never edit what they write. `kiln export --check` fails when a committed one is out of date.");
        line("");
        line("| Export | Format | Writes | Template |");
        line("|---|---|---|---|");
        for row in &generated {
            line(row);
        }
    }
    line(END);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        let mut c = Config {
            format: 1,
            name: "Demo".into(),
            paths: vec!["design".into()],
            code: Default::default(),
            repository_web: None,
            types: vec![crate::config::TypeDef {
                name: "requirement".into(),
                prefix: "REQ".into(),
                title: "Requirement".into(),
                plural: "Requirements".into(),
                statuses: vec!["draft".into(), "approved".into(), "obsolete".into()],
                initial: "draft".into(),
                done: vec![],
                closed: vec!["obsolete".into()],
                parents: vec!["requirement".into()],
                template: None,
            }],
            links: vec![],
            coverage_roles: vec![],
            levels: vec![],
            trace_role: "refines".into(),
            link_bare_ids: true,
            jira_url: None,
            jira_sync: None,
            severities: Default::default(),
            confluence: None,
            exports: vec![],
        };
        c.code.paths = vec!["src".into()];
        c
    }

    #[test]
    fn a_new_guide_has_the_rules_then_the_section() {
        let c = config();
        let text = write_agents(&c, None).unwrap();
        assert!(text.starts_with("# Working on Demo's design\n"));
        assert!(text.ends_with(&agents_section(&c)));
        assert!(text.contains(
            "| requirement (Requirement) | `REQ-001` | `design/requirements/` | draft (new), approved, obsolete (closed) | requirement |"
        ));
        assert_eq!(agents_current(&c, &text), Ok(Some(true)));
        assert_eq!(agents_path(&c), "design/AGENTS.md");
        assert!(!text.contains("### Planning"), "no part-of role, no plan");
        assert!(agents_pointer(&c).contains("read `design/AGENTS.md`"));
    }

    #[test]
    // verifies: REQ-020
    fn the_planning_part_names_the_work_and_its_done_statuses() {
        let mut c = config();
        let ty =
            |name: &str, prefix: &str, statuses: &[&str], done: &[&str]| crate::config::TypeDef {
                name: name.into(),
                prefix: prefix.into(),
                title: name.into(),
                plural: format!("{name}s"),
                statuses: statuses.iter().map(|s| s.to_string()).collect(),
                initial: statuses[0].into(),
                done: done.iter().map(|s| s.to_string()).collect(),
                closed: vec![],
                parents: vec![],
                template: None,
            };
        c.types
            .push(ty("milestone", "MS", &["planned", "done"], &["done"]));
        c.types
            .push(ty("task", "TASK", &["todo", "doing", "done"], &["done"]));
        let role = |name: &str, from: &str, to: &str| crate::config::LinkRole {
            name: name.into(),
            from: vec![from.into()],
            to: vec![to.into()],
            inverse: "x".into(),
        };
        c.links.push(role("part-of", "task", "milestone"));
        c.links.push(role("depends-on", "task", "task"));
        let section = agents_section(&c);
        assert!(
            section.contains(
                "### Planning\n\nWork is a task `part-of` a milestone, ordered by `depends-on`."
            ),
            "{section}"
        );
        assert!(
            section.contains("Finished work has a done status (task done);"),
            "{section}"
        );
    }

    #[test]
    // verifies: REQ-019
    fn refreshing_replaces_only_the_section() {
        let c = config();
        let mut changed = c.clone();
        changed.types[0].prefix = "R".into();
        let old = write_agents(&c, None).unwrap();
        let edited = old.replace("## Rules\n", "## Rules\n\n- Our own rule.\n") + "\nAfter.\n";
        assert_eq!(agents_current(&changed, &edited), Ok(Some(false)));
        let new = write_agents(&changed, Some(&edited)).unwrap();
        assert!(new.contains("- Our own rule.\n"));
        assert!(new.ends_with(&format!("{}\nAfter.\n", agents_section(&changed))));
        assert_eq!(agents_current(&changed, &new), Ok(Some(true)));
    }

    #[test]
    fn a_file_without_markers_gains_the_section_and_a_broken_one_is_refused() {
        let c = config();
        let text = write_agents(&c, Some("# Ours\n\nKeep this.\n")).unwrap();
        assert!(text.starts_with("# Ours\n\nKeep this.\n\n<!-- kiln:agents begin"));
        assert_eq!(agents_current(&c, "# Ours\n"), Ok(None));
        let broken = format!("# Ours\n{BEGIN}: x -->\nstuff\n");
        assert_eq!(
            write_agents(&c, Some(&broken)),
            Err(AgentsError::Unclosed { line: 2 })
        );
    }
}
