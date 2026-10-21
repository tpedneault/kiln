# Kiln — design

Kiln keeps a project's requirements, design decisions, milestones, work
items, tests and risks as plain Markdown files in the project's own
repository, links them to each other and to the code, and shows what
follows from those links: what is unverified, what a change affects, how
far a milestone has got. It exports all of it, by default as one design document on one
Confluence page, for the people who don't use Kiln.

Status: K0–K12 done (the format, `kiln check`, the editing commands, exports to Markdown, HTML and Word, publishing to Confluence on Cloud and Data Center with `kiln auth confluence`, `kiln lsp` for Fenix and other editors, `kiln provider` for Flint, `kiln diff` and `export --since` against a baseline, `kiln confluence comments`, the GUI (editing front matter and running exports since K9), the Jira mirror with K207 and `kiln auth jira`, `kiln jira sync` and `pull` keeping a sub-task per task in step (K10), presets such as arc42 with `kiln preset add` (K11), Home, the GUI's projects dashboard, to open, add, remove and switch projects (K12), this repository's own `design/` and generated design document); every milestone is built. Every endpoint marked **verify** in CONFLUENCE.md still awaits a live instance. Pinned against Fenix
`ce118a2af1a0b70733d0a2dd426f94365cf5eb1a` and Flint
`eb23547eb5939a52b8de06c298271ce4c8604927` (2026-10-12).

| Document | What it holds |
|---|---|
| this one | goals, architecture, crates, CLI, milestones |
| [FORMAT.md](FORMAT.md) | the item file format, `kiln.toml`, links, code references, the `kiln check` rules |
| [EXPORT.md](EXPORT.md) | export targets, layouts (one document, sections, a page tree), templates, formats (Confluence, Markdown, HTML, Word) |
| [CONFLUENCE.md](CONFLUENCE.md) | the Confluence format: pages, planner, drift, the API |
| [INTEGRATION.md](INTEGRATION.md) | Fenix, Flint and Jira: what Kiln uses, and what they need to add |
| [ARCHITECTURE.md](ARCHITECTURE.md) | the workspace as built: crates, dependency direction, lints |
| [GUI.md](GUI.md) | the GUI: pages, keys, looks, how it is built |

## Why the name

A kiln is where shaped clay becomes something that holds. Requirements and
decisions are shaped here before the work is fired. It sits beside Fenix
(the fire) and Flint (the spark).

## Where Kiln stops and the others start

| Belongs to | Is |
|---|---|
| **Kiln** | the project's model: what it must do, why it is built this way, how far along it is, and the links between those. Owned by the project; lives in the repo; reviewed in merge requests. |
| **Fenix** | editing text (including Kiln's files), rendering diagrams, *your* agenda, notebook and Jira view. Owned by you. |
| **Flint** | getting to any of it from anywhere. |
| **Jira** | tickets, when the organisation uses Jira. Kiln links to them, never replaces them. |

## Goals

1. **The repository is the source of truth.** Every fact Kiln shows comes
   from files in the repo (plus git history and, read-only, Jira). No
   database, no server. Deleting Kiln loses no information.
2. **Readable without Kiln.** The files are Markdown a person can read on
   GitLab or GitHub; exports turn them into a design document where the
   rest of the organisation already reads: a Confluence page, a file, a
   printable HTML page.
3. **Checkable in CI.** `kiln check` fails a merge request that breaks a
   link, duplicates an ID or leaves an approved requirement unverified.
4. **Fast and keyboard-first**, with the same grammar and themes as Fenix
   and Flint.
5. **Works on a locked-down corporate machine and in a CI container.** No
   admin rights, single binary, system proxy and corporate CA store
   honoured, tokens in the OS credential store or CI variables.
6. **No tool requires another.** Kiln works without Fenix, Flint, Jira or
   Confluence; each integration degrades and says so.

## Non-goals

- **A ticket tracker.** No assignees, sprints, estimates or boards of
  other people's work. Work items exist for projects without Jira and for
  linking; with Jira, they point at tickets, and (K10) Kiln keeps the
  sub-tasks of its own project issue in step with them, one way
  (ADR-014).
- **Editing in Confluence.** Confluence is a published view. Edits made
  there are detected and reported, never merged back (CONFLUENCE.md,
  "Drift").
- **A rich-text editor.** Kiln edits front matter (status, links) and
  creates files; prose is written in Fenix or any editor.
- **Formal tool qualification** (DO-178C, ISO 26262 tool confidence). Kiln
  can make that evidence easier to gather; it does not claim to be
  qualified.
- **A server or multi-user web app.** Collaboration happens through git.

## Concepts

- **Item**: one Markdown file with a TOML front matter block. Has an ID
  (`REQ-014`), a type, a title, a status, optional parent, links, tags and
  a body.
- **Type**: declared per project in `kiln.toml`: ID prefix, allowed
  statuses, allowed parent types. The default set is requirement, design,
  decision, milestone, task, test and risk.
- **Link**: a named, typed, directed relation (`verifies`, `implements`,
  `part-of`, `supersedes`, `mitigates`). Written **once**, on its source
  side; the inverse ("verified by") is computed. One fact, one place.
- **Code reference**: a comment in source code such as
  `// implements: REQ-014`, which makes that code location a link source.
- **Baseline**: a git tag (or any revision). Kiln compares the model at
  two revisions item by item.
- **Finding**: a problem `kiln check` reports: an error fails the check, a
  warning doesn't.
- **Preset**: a document template Kiln ships (arc42, a requirements
  specification, a design document) with the types it places and an
  export target, copied into a project by `kiln preset add` and the
  project's own from then on.

FORMAT.md defines all of these precisely.

## Architecture

```
                 repo files (design/**/*.md, kiln.toml, source code)
                                   │ read (and front-matter edits)
                                   ▼
  ┌────────────────────────────────────────────────────────────────┐
  │ kiln-parse → kiln-model → kiln-index ──► kiln-check            │
  │                              │                                 │
  │                              ├──► kiln-render ──► kiln-export ──┬─► Markdown / HTML / Word files
  │                              │                                  └─► kiln-confluence ──► Confluence (REST)
  │                              ├──► kiln-lsp ◄────── Fenix (LSP client, stdio)
  │                              ├──► kiln-flint ◄──── Flint (provider protocol, stdio)
  │                              ├──► kiln-gui
  │                              ├──► kiln-jira ──► Jira (REST, read-only, fenix-jira)
  │                              └──► kiln-git (baselines, commit ids; shells out to git)
  └────────────────────────────────────────────────────────────────┘
          kiln-fenix ──► Fenix control channel (open at a line, agenda)
```

### Crates

The same shape as Fenix and Flint: small host-agnostic crates with their
own unit tests; I/O only where the crate's role is I/O.

| Crate | Role | Pure? |
|---|---|---|
| `kiln-model` | `kiln.toml` schema (types, statuses, links, paths, code-reference patterns), `ItemId`, `Item`, `Link`, `Finding`; the presets, as data. | yes |
| `kiln-parse` | Splits front matter from body, parses front matter with `toml_edit` (keeping spans for diagnostics and comment-preserving edits), finds references in bodies and in source comments. Every value keeps its byte span. Adding a preset to `kiln.toml`, spliced so comments survive. | yes |
| `kiln-index` | Loads a project: walks configured paths (`ignore` crate, so `.gitignore` is honoured), parses in parallel, builds the graph (forward and inverse links, parent tree), answers queries (children, ancestors, transitive impact, coverage). Incremental update of one file for the LSP. | yes, given a file source trait |
| `kiln-check` | The rules in FORMAT.md, each with a code, a severity (overridable in `kiln.toml`) and a span. Output as text, JSON, GitLab Code Quality and GitHub annotations. | yes |
| `kiln-render` | Markdown body → a small intermediate document tree (`pulldown-cmark`), with item references, callouts, tables, code blocks and Mermaid diagrams as distinct nodes. Report builders (traceability, findings, roadmap, risks, changes) produce the same tree. | yes |
| `kiln-export` | Export targets: selection, templates and directives, assembly into one numbered document with anchors, the three layouts (splitting into pages and resolving links across them), diagram rasterising and its cache, and the Markdown, HTML and Word back ends with their file destination. See EXPORT.md. | yes, except writing files |
| `kiln-confluence` | The Confluence back end: storage-format emitter, normaliser, planner (pure), the client trait with Cloud and Data Center implementations (`ureq`), and apply. See CONFLUENCE.md. | emitter and planner yes; client no |
| `kiln-git` | Current commit, dirty state, file contents at a revision, tags. Shells out to `git`, as `fenix-git` does. | no |
| `kiln-lsp` | Language server over stdio (`lsp-server` + `lsp-types`, synchronous, as rust-analyzer). | no |
| `kiln-flint` | Flint provider over stdio, built on `flint-protocol`. | no |
| `kiln-fenix` | Fenix control-channel client use (`fenix-control`): open at a line, agenda lookups. | no |
| `kiln-jira` | The Jira mirror: linked tickets' status through `fenix-jira`, read-only, cached ten minutes; K207. | mirror and K207 yes; fetch no |
| `kiln-secrets` | Tokens from environment variables or the OS credential store (`keyring`, the suite's names), and prompting without echo. | no |
| `kiln-brand` | Kiln's mark, wordmark and app icon from one vector definition, drawn the way `fenix-brand` draws Fenix's (GUI.md, "Icon"). | yes |
| `kiln-gui` | The GUI (K7, GUI.md). wgpu + winit + glyphon, like Fenix and Flint. | pages and app yes; window no |
| `kiln` | The binary: `clap` CLI; and `kilnw`, which opens the GUI without a console on Windows (ADR-010). | no |

### Dependencies on Fenix and Flint crates

Neither publishes to crates.io. Depend by git, pinned to a revision, in
the workspace `Cargo.toml` only:

```toml
[workspace.dependencies]
fenix-diagram  = { git = "https://github.com/tpedneault/fenix", rev = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a" }
fenix-control  = { git = "https://github.com/tpedneault/fenix", rev = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a" }
fenix-storage  = { git = "https://github.com/tpedneault/fenix", rev = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a" }
flint-protocol = { git = "https://github.com/tpedneault/flint", rev = "eb23547eb5939a52b8de06c298271ce4c8604927" }
# K8:
fenix-jira     = { git = "https://github.com/tpedneault/fenix", rev = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a" }
# K7 (GUI):
fenix-keymap   = { git = "https://github.com/tpedneault/fenix", rev = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a" }
```

This set was built together on 2026-10-12, including rendering a Mermaid
flowchart to PNG through `fenix_diagram::render` and `fenix_diagram::png`.
`flint-protocol` brings its own pin of `fenix-rpc`; two copies of that
small crate in the build are harmless (only framing, no shared types).
Move the pins together when either side changes.

Never depend on `fenix-gui`, `fenix-project` or `fenix-notebook`.

### Kiln's own folders

The same four kinds as Fenix and Flint, under `kiln`: settings in
`%AppData%\kiln` / `~/.config/kiln` (a user `settings.toml` for things that
aren't the project's: theme, GUI keys), state in
`%LocalAppData%\kiln\state` / `~/.local/state/kiln/state` (the Confluence
page cache, the Flint provider's last project, the GUI's list of projects),
logs beside the state.
`KILN_HOME` puts all of it in one folder. `fenix_storage::paths` hard-codes
`fenix` (Flint's F14), so mirror its rules in `kiln-model` as Flint does
in `flint-config`.

Project configuration (`kiln.toml`) lives in the repo and is committed. It
never holds secrets; `kiln check` reports a finding if it sees a key named
like one.

## Command line

`kiln` finds the project by walking up from the current directory to the
nearest `kiln.toml` (`--project <dir>` overrides).

| Command | Does | Milestone |
|---|---|---|
| `kiln init [--preset NAME]` | writes a `kiln.toml` with the default types and a `design/` folder; never overwrites; `--preset` adds a preset too | K1, K11 |
| `kiln preset list` / `preset add NAME [--dry-run]` | the presets; copies one into the project: its template, the types and roles it lacks, an export target; presets stack without declaring anything twice; never overwrites (see "Presets") | K11 |
| `kiln new <type> [--parent ID] [--title T]` | allocates the next ID, writes the file from the type's template, prints its path; `--open` opens it in Fenix at the title | K1 |
| `kiln list [--type T] [--status S] [--tag X] [--jira] [--format text\|json]` | items, filtered; `--jira` adds each linked ticket's status from the Jira mirror | K1, K8 |
| `kiln show <ID>` | one item with its computed links | K1 |
| `kiln check [--format text\|json\|gitlab\|github]` | the rules; exit 1 on any error | K1 |
| `kiln set <ID> status=<S>` (also `title`, `parent`, `owner`, `due`) / `link <ID> <role> <ID>` / `unlink …` | comment-preserving front-matter edits | K1 |
| `kiln renumber <ID> --to <ID> [--file PATH]` | moves an ID and rewrites every reference (the cure for two branches allocating the same number; `--file` picks which copy of a duplicate moves) | K1 |
| `kiln trace [--type requirement] [--format text\|json\|csv]` | coverage: what implements and verifies each item, and the gaps | K1 |
| `kiln impact <ID>` | everything that links to it, transitively, by path | K1 |
| `kiln export [NAME…] --dry-run` / `--check` | renders every (or the named) export target and shows what would change; `--check` exits 1 if anything would; Confluence targets plan against the live space or `--snapshot FILE` | K2 |
| `kiln export [NAME…]` | writes file targets (K2) and publishes Confluence targets (K3); all flags in EXPORT.md and CONFLUENCE.md | K2 / K3 |
| `kiln confluence comments [EXPORT…]` | unresolved footer comments on published pages, per item, with links | K6 |
| `kiln auth confluence` / `auth jira` | stores a token in the OS credential store, prompting without echo | K3 / K8 |
| `kiln lsp` | language server on stdio | K4 |
| `kiln provider` | Flint provider on stdio | K5 |
| `kiln diff <REV>[..<REV>]` | item-level changes between revisions, or a revision and the working tree (FORMAT.md, "Baselines") | K6 |
| `kiln` (no arguments), `kiln gui [--since REV] [--item ID]` | the GUI (GUI.md); in a project its pages, elsewhere Home, the projects dashboard | K7, K12 |

Errors are one line on stderr naming what failed and why, with the file
and line when there is one. Exit codes: 0 fine, 1 findings or a refused
plan, 2 usage or configuration error, 3 network or server error.

## Presets

A preset is a document template Kiln ships, with what it needs to work:
the item types and link roles it places, item body templates for those
types, and an export target. `kiln preset add arc42` copies one into the
project; `kiln init --preset arc42` starts a project with one.

| Preset | Document | Layout | Types it brings |
|---|---|---|---|
| `arc42` | arc42 architecture documentation, its twelve sections | `sections`: a main page and a page per section | quality goal (QG), quality scenario (QS), constraint (CON), building block (BB), decision (ADR), risk (RISK), term (TERM); the role `addresses` |
| `srs` | a software requirements specification after the outline of ISO/IEC/IEEE 29148 | `document` | requirement, constraint, test, term; `verifies` |
| `design-doc` | a design document: goals, requirements, design, decisions, milestones, risks, traceability | `document` | the default types |

How adding works, and why (ADR-015):

- **Copied, not referenced.** The template, the types and the target
  become the project's own, changed like any other file and reviewed in
  its merge requests. A newer Kiln never rewrites them: a document whose
  shape changed under its authors after an upgrade would be a surprise in
  the one place surprises cost most, and a team adapting arc42 to itself
  is the normal case.
- **Added to, never replaced, nothing twice.** A type the project
  already declares, by hand or from another preset, is kept as it is;
  the templates use no status names, so they work with the project's
  own. A link role the project has gains only the `from` and `to` types
  it lacks. Each preset's export has its own name. So presets stack:
  `arc42` and `srs` share `constraint` and `term`. A prefix another type
  already uses, an export of the same name or a file already there stops
  it before anything is written; so does a result `kiln check` would
  fail. `kiln.toml` is spliced, not re-serialised, so its comments and
  layout survive.
- **The minimum, with the options documented.** A preset adds what its
  template needs and nothing more. The links that make arc42 traceable
  beyond its document (tests verifying quality scenarios, code
  implementing building blocks, blocks using blocks, concepts and runtime
  scenarios as items) are choices with real alternatives, so the wiki's
  "Presets" page lists them for a project to add by hand, every block of
  it tested against the templates.
- **The template guides in comments.** Guidance for the author is in HTML
  comments, which GitLab and GitHub hide and exports leave out, so the
  file reads as an outline and the export shows only what the project
  wrote (EXPORT.md, "Templates").
- **The structure is the standard's, the words are Kiln's.** arc42's own
  help text is CC BY-SA, which would bring its share-alike terms into a
  binary licensed MIT OR Apache-2.0; the presets follow arc42's (and
  ISO/IEC/IEEE 29148's) section structure, link to the source and are
  written fresh.
- **Ready for a shared space.** Each target's title carries the project's
  name (`Shop — architecture`), and the commented-out Confluence target
  sets `qualify_titles = "always"` (CONFLUENCE.md, "Pages and their
  identity"), so two projects with the same preset can publish to one
  Confluence space without their section pages fighting for a title.

## GUI (K7)

Pages, each a full window like Fenix's pages, with Vim keys and the `SPC`
leader: **Tree** (items by parent, filtered), **Trace** (coverage table,
gaps highlighted), **Impact** (pick an item, see what depends on it),
**Board** (work items by status), **Roadmap** (milestones with progress
computed from `part-of` items), **Decisions** (the ADR log with
supersession chains), **Changes** (baseline diff); from K9 also **Item**
(`kiln show`), **Check** (the findings), **Exports** (dry runs, writing,
publishing after a confirm) and **Comments** (Confluence's unresolved
comments). `Enter` on an item opens it in Fenix at its title line. From
K9 the GUI edits front matter as `kiln set`, `link`, `unlink`, `new` and
`renumber` do, through the same `kiln-index` functions; it never edits
prose (ADR-012). From K12 it starts anywhere: outside a project it opens
on **Home**, a projects dashboard after Fenix's (the projects opened
before, a preview of each, adding, removing and switching), and `SPC h`
goes back there from any page. Designed in detail in [GUI.md](GUI.md).

## Performance

| Budget | Target |
|---|---|
| `kiln check` on 2,000 items + 50k source files | ≤ 2 s cold on a laptop |
| LSP response after a keystroke in an item file | ≤ 50 ms p95 (incremental re-index of that file only) |
| Flint provider query | ≤ 16 ms for 2,000 items (static items; Flint matches them) |
| `kiln export --dry-run` of a 2,000-item document | ≤ 3 s, excluding diagram rasterisation, which is cached by source hash |

A benchmark on a generated project runs in CI from K1, as Flint's index
benchmark does: `crates/kiln-check/examples/bench.rs`, gating at 50% over
the `kiln check` budget, (from K2) the `kiln export --dry-run` budget and
(from K4) the keystroke budget: a hundred edits to one item file, each
re-read alone and the whole project checked again, gated on the p95; and
(from K5) the Flint provider's drill-down answers, gated on the p95, with
its 2,000 static items checked to fit one Flint message (CI.md). It measures with a warm file cache; the
cold figure is for a person to check on a laptop.

## Security

- `kiln.toml` is code: it is read from a repo the user cloned. Nothing in
  it is executed. Code-reference patterns are regexes with a size limit
  (`regex` crate, linear time); paths are confined to the project root.
- Tokens: environment variables (`KILN_CONFLUENCE_TOKEN`,
  `KILN_JIRA_TOKEN`) for CI, the OS credential store otherwise. Never in
  any file Kiln writes, never in logs, never on the command line.
- HTTP: `ureq` with `native-certs`, as `fenix-jira`, so a TLS-intercepting
  corporate proxy works; the system proxy via `HTTPS_PROXY`/`HTTP_PROXY`
  and, on Windows, the WinINET setting (verify `ureq`'s behaviour in K3
  and document it).
- Markdown bodies never pass raw HTML through to Confluence or HTML export.

## Testing

- Pure crates carry the logic and most tests: parsing with spans, every
  check rule (one fixture project per rule), graph queries, renumbering,
  rendering (golden files), the Confluence emitter (golden storage XHTML,
  checked well-formed), the planner (table-driven: remote state × desired
  state → plan).
- `kiln-confluence` client: an in-process fake Confluence (Cloud and Data
  Center flavours, the subset Kiln uses, with version numbers, properties,
  labels, attachments, 409 and 429 behaviour) drives apply tests. Live
  tests against real instances are `#[ignore]` and documented (see
  CONFLUENCE.md, "Testing").
- `kiln-lsp`: tests that drive the server through `lsp-server`'s in-memory
  connection.
- `kiln-flint`: tests that spawn `kiln provider` and speak the protocol
  with `flint-protocol`'s types.
- Determinism: rendering the same project twice, and on Windows and Linux,
  produces byte-identical output (line endings normalised on read).
- CI on Windows and Linux through one `scripts/ci.ps1`, like Fenix and
  Flint: fmt, clippy `-D warnings`, tests, the benchmark gate.

## Milestones

Each milestone is one pull request, usable at its end, with docs and a
changelog line.

| # | Delivers | Needs from others |
|---|---|---|
| K0 | Workspace, crates as shells with their doc comments, licences, CI on both OSes, `docs/ARCHITECTURE.md` | — |
| K1 | The format: `kiln-model`, `kiln-parse`, `kiln-index`, `kiln-check`, `kiln-git` (commit id only); `init`, `new`, `list`, `show`, `check`, `set`/`link`/`unlink`, `renumber`, `trace`, `impact`; this repository's own `design/` folder describing Kiln (dogfooding); benchmark gate | — |
| K2 | Exports without writes to Confluence: `kiln-render`, `kiln-export` (targets, templates, the three layouts, reports, diagrams), the Markdown and HTML formats end to end, and the Confluence emitter and planner with `--dry-run` against a recorded snapshot and a live space (read-only calls) | — |
| K3 | Confluence publishing on Cloud and Data Center: create, update, move, adopt a page, labels, attachments, properties, drift detection, retired pages, size limits, rate limiting, `auth confluence`, CI recipes | — (G2 for fonts in slim CI images) |
| K4 | `kiln lsp`: completion, hover, definition, references, rename, diagnostics, code actions, formatting, for item files | — (G3 for code files, since resolved in Fenix; G7 for file renames and creation, G8 for file notifications, both worked around) |
| K5 | `kiln provider` for Flint: items, open in Fenix at a line, copy, set status, open in Confluence | — (G1 for clock-in without an existing agenda task) |
| K6 | `kiln diff` and baselines; `export --since` (the changes report and the in-place markers); `kiln confluence comments` | — |
| K7 | GUI: Tree, Trace, Impact first; Board, Roadmap, Decisions, Changes after | — |
| K8 | The `docx` format (EXPORT.md); the Jira mirror (status of linked tickets, read-only, through `fenix-jira`) | — (G6 for Jira Cloud, G9 for Jira behind a proxy) |
| K9 | The GUI edits front matter (status, title, parent, owner, due, links, new items, renumbering) through the editing commands' shared functions, never prose (ADR-012); Item, Check, Exports (write, publish after a confirm) and Comments pages; picking another baseline | — |
| K10 | `kiln jira sync`: a project issue and a sub-task per task, created and kept in step (summary, due date, status by transition), keys written back into the files; `kiln jira pull`; the Jira target in the GUI (ADR-014) | — (G6 for Jira Cloud, G9 for Jira behind a proxy) |
| K11 | Presets: `kiln preset list`/`add`, `kiln init --preset`, with `arc42`, `srs` and `design-doc`, each adding its types, roles and export to `kiln.toml` without duplicates, options on the wiki (ADR-015); `style="compact"` items; a template's own HTML comments left out of exports; Confluence titles that don't depend on who published first (`qualify_titles`), a refused plan instead of a failed create when a title is taken, and `children = "list"` | — |
| K12 | Home, the projects dashboard (REQ-017): `kiln` outside a project opens on the projects opened before, with a preview of each (items, types, next milestone, `kiln check`'s findings), adding a folder, removing one from the list, switching with `SPC h`; the list in the state folder | — |

Everything through K12 needs nothing new from Fenix or Flint to work. INTEGRATION.md
lists the changes (G1–G9) that make the integration better and which
milestone notices their absence.

## Risks

| Risk | Mitigation |
|---|---|
| The Confluence API differs between Cloud and Data Center, and Cloud is mid-migration from v1 to v2 | one client trait, two implementations; every endpoint used is listed in CONFLUENCE.md with its flavour; K2 starts with read-only calls against both |
| People edit published pages in Confluence | drift detection with the author named; a banner on every page; optional edit restriction |
| Two branches allocate the same ID | `kiln check` finds duplicates; `kiln renumber` and LSP rename fix them |
| The model becomes a second Jira | non-goals above; work items have no assignees or sprints |
| The format is wrong and costly to change | K1 dogfoods it on Kiln's own design before anything depends on it; `format = 1` in `kiln.toml` allows a migration later |
| Diagram text missing in CI images without fonts | exports refuse to rasterise when no fonts are found, naming the fix; G2 |
| A one-page document outgrows what Confluence handles well | size measured on every export, warning and refusal thresholds, `sections` layout as the remedy (CONFLUENCE.md, "Size") |
