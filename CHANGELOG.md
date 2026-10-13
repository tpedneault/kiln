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

### Changed

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
