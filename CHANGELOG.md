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
- K10: Kiln keeps Jira in step with its tasks (ADR-014). With `[jira]
  project`, `kiln jira sync` creates one project issue and a sub-task per
  task, and keeps their summary, due date and status, by the workflow
  transition that reaches the mapped status (`[jira.status]`, else the
  status category); it prints the plan first, asks (or `--yes`), and
  writes the keys it created into `kiln.toml` and the items, to commit.
  Only the project issue's sub-tasks are ever written, nothing is
  deleted, and a move the workflow forbids is reported and skipped.
  `kiln jira pull` takes a ticket's status back into its item; K207 names
  both. In the GUI, the Exports page shows the sync as a `jira` target
  (`P` syncs after a `y`), and `J` pulls an item's status
  ([docs/INTEGRATION.md](docs/INTEGRATION.md), [docs/CLI.md](docs/CLI.md)).
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
- Kiln's mark and icon: a fired block in clay holding Fenix's flame, drawn
  as Fenix's Forged F is, so the suite reads as one. `kiln-brand` draws
  it from one definition: the GUI window's icon, and the icon embedded in
  `kiln.exe` and `kilnw.exe` on every Windows build; `kiln.ico` and the
  logos in `docs/brand/` are checked against it by a test
  ([docs/GUI.md](docs/GUI.md), "Icon").

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

- K11: presets. `kiln preset list` and `kiln preset add NAME` (or `kiln
  init --preset NAME`) copy a ready-made document into the project: its
  template in `design/templates/`, the types and link roles it places
  that `kiln.toml` lacks (inserted with their comments, the rest of the
  file left as written), item templates for those types, and an export
  target, with a Confluence one commented out. `arc42` (twelve sections,
  a page each; quality goals and scenarios, constraints, building blocks,
  decisions, risks, glossary terms), `srs` (a requirements specification
  after ISO/IEC/IEEE 29148) and `design-doc`. Presets stack without
  declaring anything twice: a type already there is kept, a shared role
  gains only what it lacks. Never overwrites; copied, not referenced
  (ADR-015). The wiki's new "Presets" page says what each adds and the
  options to add by hand (tests verifying quality scenarios, code
  implementing building blocks, blocks using blocks, concepts and
  runtime scenarios as items…), its blocks tested in CI. Templates keep
  guidance in HTML comments, which exports now leave out, and
  `kiln:items style="compact"` shows items as a bold line and their body,
  without a heading, for glossaries ([docs/CLI.md](docs/CLI.md),
  [docs/EXPORT.md](docs/EXPORT.md), [docs/DESIGN.md](docs/DESIGN.md),
  "Presets").
- Confluence: `[export.confluence] qualify_titles = "always"` gives
  every section and item page the target's title in parentheses from the
  first publish, so several projects can publish the same template to one
  space without the titles depending on who published first; `children =
  "list"` lists child pages as links in document order instead of the
  `children` macro ([docs/CONFLUENCE.md](docs/CONFLUENCE.md)).
- K12: Home, the projects dashboard (REQ-017). `kiln` outside a project
  opens on the projects opened before, after Fenix's Home: the logo and
  the date, a find field, each project with when it was last opened and a
  dot for its `kiln check`, and a preview (items, types, next milestone,
  findings). `Enter` or `1`–`9` opens one, `a` adds a folder (with `Tab`
  completion), `d` takes one off the list, and `SPC h` or `gh` on any page
  goes back to switch; the list is `projects.toml` in the state folder
  ([docs/GUI.md](docs/GUI.md), "Home").
- K13: requirement levels (REQ-018, ADR-016). `[requirements] levels`
  lists the requirement types highest first and `trace` (default
  `refines`) the role from each level to the one above; `derived = true`
  marks a requirement that deliberately traces to nothing above it. K201
  and K202 check every level their role can point at; K208 reports a
  requirement that traces to nothing above and isn't derived, K209 a
  trace link that skips a level, K210 a requirement nothing below
  refines. `kiln trace --levels` (text, JSON, and CSV as a traceability
  matrix) and the `levels` report follow the chains down, derived and
  untraced requirements apart; exports show "Derived" in an item's
  properties, `kiln show` its level, `kiln diff` a change to `derived`.
  Without `[requirements]` nothing changes
  ([docs/FORMAT.md](docs/FORMAT.md), "Requirement levels").
- K14: the agent guide (REQ-019, ADR-017). `kiln agents` writes
  `design/AGENTS.md` for coding agents: rules written once and then the
  project's (read with Kiln, change front matter only through `kiln
  new`, `set`, `link` and `unlink`, link from the source side, leave
  sign-off to people, add code references, keep `kiln check` clean,
  never edit exports), and a section between markers generated from
  `kiln.toml` (types, folders, statuses, link roles, code references,
  requirement levels, coverage, exports). `kiln init` writes it, `kiln
  preset add` refreshes it, and K113 warns when the section no longer
  matches `kiln.toml`. Kiln's own `design/AGENTS.md`
  ([docs/CLI.md](docs/CLI.md), "`kiln agents`").

### Changed

- Check: K201 and K202 fire only when `verifies` or `implements` can
  point at the requirement's type; a project whose roles never allowed
  it got a warning it couldn't fix.
- Confluence (Data Center): pages below the published root are found by
  walking `child/page`, not `descendant/page`, which a live Data Center
  answers with HTTP 500 once the page exists, so a dry run or publish of
  an already-published document no longer fails until the page is
  deleted ([docs/CONFLUENCE.md](docs/CONFLUENCE.md)).

- Confluence: a page title held by a page the target doesn't own, and
  that Kiln can't make unique (the root's, or a qualified title taken
  too, as when two targets share a `title`), stops the plan before
  anything is written, naming the title, instead of failing half-way
  through a publish; qualified titles are now looked up as well as plain
  ones ([docs/CONFLUENCE.md](docs/CONFLUENCE.md)).
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

### Fixed

- CLI: a reader that stops early (`kiln trace --levels | head -5`) ends
  `kiln` quietly with exit 0, as `git log | head` does, instead of a
  "failed printing to stdout: Broken pipe" panic, on every command and
  platform.
