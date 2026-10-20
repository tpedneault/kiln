# Changelog

All notable changes to Kiln are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Kiln uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The design: [docs/DESIGN.md](docs/DESIGN.md),
  [docs/FORMAT.md](docs/FORMAT.md),
  [docs/EXPORT.md](docs/EXPORT.md),
  [docs/CONFLUENCE.md](docs/CONFLUENCE.md) and
  [docs/INTEGRATION.md](docs/INTEGRATION.md).
- K0: the Cargo workspace with every crate as a documented shell, a
  `kiln` binary with `--help` and `--version`, one-line errors and the
  documented exit codes, and CI on Windows and Linux through
  `scripts/ci.ps1` ([docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
  [docs/CI.md](docs/CI.md), [docs/CLI.md](docs/CLI.md)).
- K1: the file format and the commands that read, check and edit it:
  `kiln init`, `new` (with `--open` in Fenix), `list`, `show`, `check`
  (text, JSON, GitLab Code Quality and GitHub output), `set`, `link`,
  `unlink`, `renumber`, `trace` and `impact`; the check rules K001–K004,
  K100–K110 and K201–K206; Kiln's own requirements, decisions, milestones
  and risks in [design/](design/), checked in CI; and a benchmark gate for
  `kiln check` on 2,000 items and 50,000 source files
  ([docs/CLI.md](docs/CLI.md), [docs/CONFIGURATION.md](docs/CONFIGURATION.md)).

- K2: exports. `kiln export` with `--dry-run`, `--check`,
  `--require-clean`, `--allow-shallow` and `--format json`; templates with
  `<!-- kiln:… -->` directives; the `document`, `sections` and `tree`
  layouts; the Markdown (GitLab, GitHub and CommonMark anchors) and HTML
  (self-contained, printable) formats; traceability, findings, roadmap and
  risks reports; Mermaid diagrams drawn with `fenix-diagram` and cached.
  Confluence targets plan without writing: the storage-format emitter,
  drift-detecting normaliser, planner and read-only client for Cloud and
  Data Center, against the live space or a `--snapshot` file. Check rules
  K005, K111 and K112. Kiln's own design document in
  [docs/generated/DESIGN.md](docs/generated/DESIGN.md), kept current in
  CI; the benchmark gate covers `kiln export --dry-run`.
- K3: publishing to Confluence on Cloud and Data Center. `kiln export`
  creates, updates, moves and retires pages (`--yes`, or a prompt on a
  terminal), with labels, attachments, the `kiln` property, version
  messages naming the commit, drift detection (`--force`), adopting a
  page (`--adopt`), `--prune-removed`, `restrict_edits`, size limits,
  retries on 409/429/503, and a page cache; an interrupted publish
  finishes on the next run. `kiln auth confluence` stores a token in the
  OS credential store, shared with Flint. CI recipes in
  [contrib/](contrib/), live tests and
  [dev/confluence/README.md](dev/confluence/README.md) for both flavours
  ([docs/CONFLUENCE.md](docs/CONFLUENCE.md), [docs/CLI.md](docs/CLI.md)).
- K4: `kiln lsp`, a language server for item files: completion of IDs,
  statuses, types, tags and link roles; hover with what `kiln show`
  prints; definition; references; rename as `kiln renumber`; `kiln check`
  diagnostics after each edit; code actions (set status, add a link, new
  child, create a covering test, rename the file to its ID) and
  front-matter formatting. `kiln init --fenix` registers it in
  `.fenix/tools.json`. Files changed outside the editor are noticed by
  polling; the benchmark gate covers a keystroke (50 ms p95)
  ([docs/CLI.md](docs/CLI.md), [docs/INTEGRATION.md](docs/INTEGRATION.md),
  which records Fenix gaps G7 and G8).
- K5: `kiln provider`, a Flint provider: every open item as a Flint item
  (open in Fenix at its title, copy the ID or `[[ID]]`, set the status,
  show what depends on it, open in Confluence or Jira, clock in on its
  agenda task) and the drill-downs `k status ID`, `k impact ID` and
  `k all text`. It follows Fenix to the project in front and notices
  changes on disk; the benchmark gate covers its drill-downs (16 ms) and
  checks its items fit one Flint message ([docs/CLI.md](docs/CLI.md),
  [docs/INTEGRATION.md](docs/INTEGRATION.md)).
- K6: baselines. `kiln diff REV[..REV]` lists the items added, changed and
  removed since a tag, branch or commit (status, title, links, body…),
  reading the old project from git; `kiln export --since REV` marks each
  new or changed item next to its heading (Markdown, HTML, Confluence)
  and fills the `changes` report; `<!-- kiln:value since -->`. `kiln
  confluence comments` lists the unresolved footer comments on the
  published pages, placed on the item they quote
  ([docs/CLI.md](docs/CLI.md), [docs/FORMAT.md](docs/FORMAT.md),
  "Baselines").
- K7: the GUI. `kiln` (or `kiln gui`) opens a window onto the project:
  Tree, Trace, Impact, Board, Roadmap, Decisions and Changes (against
  the latest tag or `--since`), Vim-style keys with a Space leader,
  Enter opening the item in Fenix, live reload, Fenix's themes; `kilnw`
  opens it on Windows without a console (ADR-010); Flint's item actions
  gain "Open in Kiln" ([docs/GUI.md](docs/GUI.md),
  [docs/CLI.md](docs/CLI.md)).
- K8: Word and Jira. `format = "docx"` exports to Word, in every layout:
  Word's own heading styles, item bookmarks and links, a filled-in table
  of contents, tables, embedded images and diagrams, deterministic bytes,
  and `[export.docx] reference` for a house style (written directly,
  ADR-011). The Jira mirror reads linked tickets read-only through
  `fenix-jira`, cached ten minutes: `kiln list --jira`, ticket status in
  the GUI, `kiln check` rule K207, and `kiln auth jira`; Jira Cloud (G6)
  and a proxy for Jira (G9) wait on Fenix
  ([docs/EXPORT.md](docs/EXPORT.md), [docs/INTEGRATION.md](docs/INTEGRATION.md),
  [docs/CLI.md](docs/CLI.md)).
- K9: the GUI does what the editing commands do, never prose (ADR-012):
  set a status (`s`, or `H`/`L` on the Board), title, parent, owner or
  due date, link and unlink, create an item or a child (opened in Fenix),
  renumber, each picked or typed on the footer line, checked as the
  command checks it, and written through the same `kiln-index` functions
  as `kiln set`, `link`, `new` and `renumber` (and the Flint provider's
  status change). New pages: Item (`kiln show`, walking from item to
  item), Check (the findings, and an error count on every page), Exports
  (each target's dry run; `w`/`W` write file targets, `P` publishes a
  Confluence target after a `y`, only the plan shown) and Comments
  (Confluence's unresolved comments, opened in the browser); `b` on
  Changes picks another baseline; `kiln check` runs on a thread, so a
  large project doesn't freeze the window; statuses are drawn as pills
  coloured by what they mean (done, in progress, initial, blocked,
  closed) ([docs/GUI.md](docs/GUI.md)).
- Exports read more easily: the items of a `kiln:items` are a flat list
  of headings at one level, each parent followed by its children, so a
  table of contents (Confluence's stops at three levels) lists every one;
  `nest="parent"` keeps the old nesting, which stays the default in
  `tree`. Properties default to the new `item_style = "list"`: the status,
  then one line per relationship with its label in bold, instead of one
  long line (`line` is still there); ADR-013 ([docs/EXPORT.md](docs/EXPORT.md)).
- The wiki: a user guide for designing a project with Kiln, from
  installing and a tutorial to the design method and references for item
  files, configuration, checks, exports, Confluence, Jira, the command
  line, editors, the GUI and CI. Kept in `docs/wiki/`, published by the
  `Wiki` workflow, its links checked by `scripts/ci.ps1`; the README
  rewritten around it ([docs/CI.md](docs/CI.md), "The wiki").
- Releases: a tag `vX.Y.Z` builds `kiln` and `kilnw` for Windows and
  Linux after the CI gates pass on the building runner, and publishes
  them with checksums and the changelog section as a GitHub release
  ([docs/CI.md](docs/CI.md), "Releasing").
- Catppuccin themes in the GUI: `theme` takes `Catppuccin Latte`,
  `Catppuccin Frappé`, `Catppuccin Macchiato` and `Catppuccin Mocha`,
  Fenix's colours for them.

- Code files in the editor: run beside a language's own server (Fenix's
  additional servers, which close INTEGRATION.md's G3), `kiln lsp`
  completes IDs after `implements:` and `verifies:`, shows hover,
  definition and references on IDs, and reports broken code references as
  you type. `kiln init --fenix` registers Kiln as an additional server for
  Markdown and the project's code languages, moving the older
  Markdown-only form over ([docs/CLI.md](docs/CLI.md),
  [docs/INTEGRATION.md](docs/INTEGRATION.md)).

- Exports: `format = "confluence-wiki"` writes Confluence wiki markup to
  a file, to paste into a page by hand where Kiln can't publish; headings
  carry no section numbers ([docs/EXPORT.md](docs/EXPORT.md),
  "`confluence-wiki`").

### Changed

- Confluence: the plan names the parent page by title as well as ID
  (`→ space DEMO, page 123456789 "Demo"`, `parentTitle` in JSON), so a
  wrong `parent` is caught before publishing. It costs no extra call.
- Exports written into the repository link files relatively and name no
  commit, so a committed export can be kept current
  ([docs/EXPORT.md](docs/EXPORT.md), "Exports kept in the repository").
- Confluence calls honour `NO_PROXY`.
- `kiln check` is faster: reference patterns are compiled once, not per
  item.
- The file format: IDs are written with at least three digits and one
  spelling per number (`REQ-007`, not `REQ-7`); `[code] exclude`; the
  default code pattern; which rules apply when
  ([docs/FORMAT.md](docs/FORMAT.md)).
