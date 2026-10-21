**Kiln** keeps a project's requirements, design notes, decisions,
milestones, tasks, tests and risks as Markdown files in the repository
they describe. It links each requirement to the design, tests and code
that answer it, checks those links in CI, shows what a change affects,
and generates a design document for the people who don't read the
repository: a Markdown file, an HTML or Word file, or a Confluence page.

It keeps no database and needs no server: the repository is the source of
truth. It works on its own, and fits in a suite with
[Fenix](https://github.com/tpedneault/fenix) (the editor) and
[Flint](https://github.com/tpedneault/flint) (the launcher).

This wiki is everything you need to design a project with Kiln.

## Start here

1. **[Installing](Installing)**: get `kiln` on your machine.
2. **[Tutorial: your first project](Tutorial)**: design a small project
   from an empty folder to a generated document, in twenty minutes.
3. **[Designing a project with Kiln](Designing-with-Kiln)**: the method.
   Which items to write and how, linking, the document, and keeping it
   true as a team.
4. **[Concepts](Concepts)**: the vocabulary, each idea explained once.

## What it looks like

An item is one Markdown file:

```markdown
+++
id = "REQ-014"
type = "requirement"
title = "The window appears within 50 ms of the hotkey"
status = "approved"
parent = "REQ-002"
tags = ["performance"]
+++

Measured from the hotkey to the first frame, at the 95th percentile, on
the reference laptop. Anything slower stops being a reflex; see ADR-004.
```

A test names the requirement it verifies, and so does the code:

```rust
#[test]
// verifies: REQ-014
fn hotkey_to_first_frame_is_under_budget() { … }
```

Kiln checks the whole web of links:

```console
$ kiln check
design/decisions/ADR-007-one-process.md:11:9: K102 error: reference REQ-099 doesn't exist
design/requirements/REQ-031-export-telemetry.md:2:7: K201 warning: REQ-031 is approved but nothing verifies it
kiln: 1 error, 1 warning
```

And it writes the document:

```console
$ kiln export
design-md (markdown, document) → docs/DESIGN.md
  update docs/DESIGN.md
```

## Reference

| Page | Covers |
|---|---|
| [Item files](Item-files) | front matter, body syntax, IDs, file names |
| [Configuration](Configuration) | every `kiln.toml` setting, custom types and links, environment variables |
| [Checking](Checking) | `kiln check` and every rule, with fixes |
| [Templates and exports](Templates-and-exports) | targets, layouts, templates, reports; Markdown, HTML and Word |
| [Presets](Presets) | arc42, a requirements specification and a design document: the templates Kiln ships, what each adds to kiln.toml, and the options |
| [Publishing to Confluence](Publishing-to-Confluence) | one page or a page tree, drift, comments, CI |
| [Changes and baselines](Changes-and-baselines) | `kiln diff`, `export --since`, release reviews |
| [Jira](Jira) | Jira keys, the read-only mirror, rule K207 |
| [Command reference](Command-reference) | every command and option, exit codes |
| [Editors and Flint](Editors-and-Flint) | the language server in Fenix and other editors; Kiln in Flint |
| [The GUI](The-GUI) | the window: tree, trace, impact, board, roadmap, decisions, changes, item, check, exports, comments; editing front matter and publishing from it |
| [Continuous integration](Continuous-integration) | GitHub Actions and GitLab, ready to copy |
| [Troubleshooting](Troubleshooting) | common problems, and frequently asked questions |

## Examples

Kiln's own design is a Kiln project. Browse its
[`design/`](https://github.com/tpedneault/kiln/tree/master/design) folder
and the [document generated from
it](https://github.com/tpedneault/kiln/blob/master/docs/generated/DESIGN.md).

The full specification (the file format, export and Confluence details,
the architecture) is in the repository's
[`docs/`](https://github.com/tpedneault/kiln/tree/master/docs) folder,
for when this wiki isn't deep enough.
