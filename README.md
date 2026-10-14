# Kiln

**Requirements, design decisions and project progress, kept as Markdown
files in the repository they describe.**

Kiln links each requirement to the design, tasks, tests and code that
answer it, and checks those links in CI. It shows what a change affects,
and generates a design document for the people who don't read the
repository: a Markdown file, an HTML or Word file, or a Confluence page.

It keeps no database and needs no server. Delete Kiln and your design is
still there, as plain files anyone can read on GitHub or GitLab.

**[Read the wiki](https://github.com/tpedneault/kiln/wiki)**: install it,
follow the tutorial, and design your own project.

## What it looks like

A requirement is one Markdown file with a small TOML header:

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

A design note says what it implements, and a test says what it verifies:

```toml
# design/designs/DES-003-the-window.md
[links]
implements = ["REQ-014"]
```

```rust
#[test]
// verifies: REQ-014
fn hotkey_to_first_frame_is_under_budget() { … }
```

Kiln checks the whole web of links, with file, line and column:

```console
$ kiln check
design/decisions/ADR-007-one-process.md:11:9: K102 error: reference REQ-099 doesn't exist
design/requirements/REQ-031-export-telemetry.md:2:7: K201 warning: REQ-031 is approved but nothing verifies it
kiln: 1 error, 1 warning
```

It answers questions about the design:

```console
$ kiln show REQ-001
REQ-001 Beacon checks every configured site on a schedule
requirement · approved · design/requirements/REQ-001-beacon-checks-every-configured-site-on-a-schedule.md:2
children        REQ-002 A check finishes within 5 seconds per site
tags            core
implemented by  DES-001 The scheduler, src/scheduler.rs:1
verified by     src/scheduler.rs:5
mentioned by    ADR-001 Checks run concurrently, one task per site
coverage        implements: yes, verifies: yes
```

And it writes the design document from a template you control, as a
file or onto a Confluence page:

```console
$ kiln export
design-md (markdown, document) → docs/DESIGN.md
  update docs/DESIGN.md
```

## What it does

- **Items as files.** Requirements, design notes, decisions,
  milestones, tasks, tests and risks are Markdown files with TOML front
  matter. You can define your own types, statuses and link roles.
- **Traceability.** Links between items, and comments in any
  programming language (`// implements: REQ-007`), feed a traceability
  matrix. `kiln trace` shows the gaps, and `kiln impact` shows everything
  a change reaches.
- **Checks for CI.** Broken references, duplicate IDs, links the
  configuration doesn't allow, approved requirements nothing verifies,
  late milestones. Findings come out as text, JSON, a GitLab Code Quality
  report, or GitHub annotations.
- **Design documents.**
  - **Formats:** Markdown, self-contained HTML (print it to PDF), Word
    with your company's styles, or Confluence (Cloud or Data Center).
  - **Layouts:** one page, a page per section, or a page per item.
  - **Templates:** you write the headings and prose; Kiln places the
    items and reports. Diagrams in Mermaid are drawn.
- **Change review.** `kiln diff v1.0` and `kiln export --since v1.0`
  show what changed in the design since a release.
- **Safe edits.** `kiln new`, `set`, `link` and `renumber` change only
  what they are about, keep your comments and formatting, and never
  overwrite a file your editor just saved.
- **Editors and tools.**
  - **Language server:** completion, hover, go to definition, find
    references, rename, diagnostics and code actions, in
    [Fenix](https://github.com/tpedneault/fenix) or any LSP editor.
  - **Window:** a keyboard-driven view of the tree, trace, board,
    roadmap and decisions.
  - **[Flint](https://github.com/tpedneault/flint) provider:** every item
    a few keystrokes away.
  - **Jira:** the status of linked tickets, read-only.

## Get started

```console
$ kiln init --name MyProject
$ kiln new requirement --title "Users can export their data as CSV"
design/requirements/REQ-001-users-can-export-their-data-as-csv.md
$ kiln check
kiln: no findings
```

Then follow the **[Tutorial](https://github.com/tpedneault/kiln/wiki/Tutorial)**.
It designs a small project from an empty folder to a generated document.

## Installing

Each [release](https://github.com/tpedneault/kiln/releases) has an
archive for each platform, with a `.sha256` file beside it:

- **Windows:** `x86_64-pc-windows-msvc`, a `.zip`.
- **Linux:** `x86_64-unknown-linux-gnu`, a `.tar.gz`, needing glibc 2.35
  or newer.

Unpack it and put the folder on your `PATH`:

- `kiln` is the command line, and opens the window when run on its own.
- `kilnw` opens the window without a console, on Windows.

On Linux, `kiln` runs headless in CI with nothing else installed.

From source, with Rust stable:

```console
$ cargo install --locked --git https://github.com/tpedneault/kiln kiln
```

[Installing](https://github.com/tpedneault/kiln/wiki/Installing) has the
details.

## Documentation

- **[The wiki](https://github.com/tpedneault/kiln/wiki)** is the user
  guide: the tutorial, the design method, and references for item files,
  configuration, checks, exports, Confluence, the command line, editors
  and CI. Its source is [docs/wiki/](docs/wiki/).
- **The specification**, for contributors and the curious:
  - [Design](docs/DESIGN.md): goals, architecture, budgets and
    milestones.
  - [File format](docs/FORMAT.md): the contract.
  - [Exporting](docs/EXPORT.md) and
    [Publishing to Confluence](docs/CONFLUENCE.md).
  - [Fenix, Flint and Jira](docs/INTEGRATION.md).
  - [Architecture](docs/ARCHITECTURE.md), [the GUI](docs/GUI.md),
    [Command line](docs/CLI.md), [Configuration](docs/CONFIGURATION.md)
    and [Continuous integration](docs/CI.md).
- **Kiln's own design** is a Kiln project. See [design/](design/) and the
  document generated from it, [docs/generated/DESIGN.md](docs/generated/DESIGN.md).

## Status

Every planned milestone is built: the file format and checks, the editing
commands, exports to Markdown, HTML, Word and Confluence, the language
server, the Flint provider, baselines and diffs, the window, and the Jira
mirror.

Two parts haven't yet been confirmed against live servers:

- **Confluence publishing** is tested against a faithful fake server of
  both Cloud and Data Center, but not yet against every live instance.
- **The Jira mirror** supports Jira Server and Data Center only.

[CHANGELOG.md](CHANGELOG.md) lists what changed.

## Contributing

Kiln is a Cargo workspace. One script runs every gate, locally and in
CI, on Windows and Linux: format, lint, tests, Kiln checking its own
design, and the performance benchmark.

```powershell
pwsh -NoProfile -File ./scripts/ci.ps1
```

[docs/CI.md](docs/CI.md) explains the gates. [CLAUDE.md](CLAUDE.md) holds
the project's working conventions.

## Licence

Kiln is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
