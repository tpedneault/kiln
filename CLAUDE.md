# Working on Kiln

Kiln keeps a project's requirements, design decisions, milestones, work
items, tests and risks as Markdown files in the repository, checks the
links between them in CI, and exports them (by default as one design
document on one Confluence page) for people who don't use Kiln. It is
part of a suite with Fenix (editor, https://github.com/tpedneault/fenix)
and Flint (launcher, https://github.com/tpedneault/flint). The design is settled; K0 (the
workspace and CI), K1 (the format, `kiln check`, the editing commands,
this repository's own `design/`), K2 (exports; Confluence planning
without writes; the committed design document) and K3 (Confluence
publishing, `kiln auth confluence`; every **verify** still unconfirmed
live), K4 (`kiln lsp`; Fenix gaps G7 and G8 worked around), K5
(`kiln provider` for Flint), K6 (`kiln diff`, `export --since`,
`kiln confluence comments`), K7 (the GUI, `kiln gui`, `kilnw` on
Windows) and K8 (the `docx` format, the Jira mirror with K207 and `kiln
auth jira`; Fenix gaps G6 and G9 worked around) are done.

## Read first, in this order

1. `docs/DESIGN.md` — goals, non-goals, architecture, crates, CLI,
   budgets and milestones K0–K8.
2. `docs/FORMAT.md` — the item file format and `kiln.toml`; the contract.
   `kiln-model`/`kiln-parse` implement exactly this; change both together.
3. `docs/EXPORT.md` — export targets, the three layouts (one document is
   the default), templates, reports, formats (Confluence, Markdown, HTML,
   Word), determinism, diagrams.
4. `docs/CONFLUENCE.md` — the Confluence format: pages and their
   identity, planner, drift, storage format, the API by flavour, retries,
   CI, testing.
5. `docs/INTEGRATION.md` — Fenix, Flint and Jira: what Kiln uses and the
   gaps G1–G9.

## How to work

- **Milestone by milestone, in order.** Each milestone is one branch and
  one pull request that leaves Kiln usable, with tests, docs and a
  `CHANGELOG.md` line. Don't start K(n+1) inside K(n)'s pull request.
- **K1 dogfoods the format.** By the end of K1, this repository has a
  `kiln.toml` and a `design/` folder describing Kiln's own requirements
  (taken from DESIGN.md's goals) and decisions (TOML front matter,
  sequential IDs, one-way publishing…), and `kiln check` runs on it in CI.
  If the format feels wrong while doing that, change FORMAT.md *then*,
  before anything else depends on it.
- **K2 makes no writes to Confluence.** K2 delivers exports end to end
  for the file formats (Markdown, HTML) and only read-only calls and
  `--dry-run` for Confluence; K3 adds Confluence writes. Build the
  `document` layout first in each step: it is the default and the
  simplest, and `sections` and `tree` are splits of the same document.
  K2 dogfoods exports the way K1 dogfoods the format: this repository gets
  a template under `design/templates/` and a committed Markdown export of
  its own design document, kept current by `kiln export --check` in CI.
- **Endpoints marked "verify" in CONFLUENCE.md** must be confirmed
  against live instances in K2/K3 (see "What only Thomas can verify"), and
  the document updated with what was found. Don't build on an unverified
  endpoint without a test against the fake server *and* a note in the pull
  request.
- **Never modify Fenix or Flint from a Kiln session**, and never write
  their files at runtime (INTEGRATION.md, principle 2). If Kiln needs
  something from them, record it as a G-item in INTEGRATION.md and work
  around it visibly.
- **Depend on Fenix and Flint crates by git and pinned `rev`**, only the
  ones DESIGN.md lists, only in the workspace `Cargo.toml`. Never
  `fenix-gui`, `fenix-project` or `fenix-notebook`.
- **Windows is the primary platform**, Linux second; CI runs both. Kiln is
  also a CI tool: it must work headless in a Linux container.
- **Budgets are requirements**: the benchmark gate arrives in K1
  (DESIGN.md, "Performance").

## Conventions (the same as Fenix and Flint)

- A Cargo workspace of small crates; logic in pure crates with unit tests,
  I/O only in the crates whose role is I/O (DESIGN.md, "Crates").
- Doc comments explain *why*, including the tradeoff taken. A module's
  header says what it is for and what it deliberately doesn't do.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` pass. One entry point, `scripts/ci.ps1`
  (PowerShell 7), runs the same gates locally and in CI.
- Commit subjects name the area, then what changed for the user:
  `Check: a duplicate ID is an error with both files named`,
  `Confluence: unchanged pages cost no API call`.
- No panics on bad input from files, config, servers or peers. Errors are
  one line naming what failed and why, with file and line when known.
- Secrets never in files, logs, plan output, errors or command lines.
- Rendering is deterministic; golden files change only deliberately.
- User docs (`docs/CONFIGURATION.md`, `docs/CLI.md`, the wiki pages in
  `docs/wiki/`, and the five design documents where behaviour changes) are
  updated in the same pull request.
- Licence: MIT OR Apache-2.0.

## What only Thomas can verify

Some things can't be checked from CI or a cloud session. When a milestone
depends on one, list it in the pull request under **"Needs checking"**
with exact steps and the expected result, rather than reporting it done:

- every **verify** in CONFLUENCE.md, against a Confluence Cloud site and a
  Data Center instance (a free Cloud site and the `atlassian/confluence`
  image with a timebomb licence; `dev/confluence/README.md` from K3);
- how the corporate Confluence at work behaves: proxy, TLS interception,
  personal access tokens, whether page restrictions and the Jira macro
  are allowed, rate limits;
- the language server inside Fenix (K4): completion, hover, `gd`, `gr`,
  rename preview, diagnostics, code actions;
- the Flint provider (K5): drill-downs, opening in Fenix, clock-in;
- diagrams in Confluence: legibility at the chosen scale, fonts in the CI
  image used at work;
- the one-page document (K2/K3): item anchors reachable from a URL
  (`…#REQ-014`) and from links on other pages, the table of contents, how a
  large document opens and edits (find the size where it gets slow and
  record it in CONFLUENCE.md, "Size"), "compare versions" between two
  publishes;
- the Markdown export rendered on the GitLab used at work (anchors, Mermaid)
  and the HTML export printed to PDF.

## Where things are

```
docs/DESIGN.md        the design
docs/FORMAT.md        the file format (the contract)
docs/EXPORT.md        exports: targets, layouts, templates, formats
docs/CONFLUENCE.md    the Confluence format
docs/INTEGRATION.md   Fenix, Flint, Jira (G1–G9)
docs/ARCHITECTURE.md  the crates as built and which way they depend
docs/GUI.md           the GUI: pages, keys, looks, how it is built
docs/CI.md            the gates and how to run them
docs/CLI.md           the command line as it stands
docs/CONFIGURATION.md kiln.toml and environment, the overview
docs/wiki/            the user guide, published to the GitHub wiki (CI.md)
kiln.toml, design/    Kiln's own Kiln project (dogfooding), checked in CI
docs/generated/       its exported design document, `kiln export --check` in CI
crates/               the workspace (kiln-* libraries, kiln binary)
scripts/ci.ps1        the one entry point for the gates
scripts/release.ps1   a release's steps; .github/workflows/release.yml runs them on a tag
CHANGELOG.md          Keep a Changelog; add under Unreleased
```
