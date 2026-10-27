# Architecture

Where Kiln's code lives and which way its dependencies point.
[DESIGN.md](DESIGN.md) says what Kiln does and why; this document is the
map of the workspace that does it, and is updated whenever a crate gains
a role or a dependency.

Status: K11, every milestone. The format crates (`kiln-model`,
`kiln-parse`, `kiln-index`, `kiln-check`), the export crates
(`kiln-render`, `kiln-export` with Markdown, HTML and Word,
`kiln-confluence` with publishing), `kiln-git`, `kiln-secrets` (environment
variables and the OS credential store through `keyring`), `kiln-fenix`
(opening a file, clocking in, opening a URL), `kiln-lsp`, `kiln-flint`,
`kiln-jira` (the Jira mirror), the GUI (`kiln-gui`) and the binary's
commands are built, and the presets (K11): their data in `kiln-model`,
the merge into `kiln.toml` in `kiln-parse`, the files in the binary.

## Layout

```
Cargo.toml            the workspace: members, shared package metadata,
                      every external dependency (and later the Fenix and
                      Flint pins), lints
clippy.toml           test-only lint relaxations
kiln.toml, design/    Kiln's own Kiln project, checked in CI (K1 dogfooding)
docs/generated/       its design document, exported and checked in CI (K2)
crates/kiln-*/        library crates, one role each
crates/kiln/          the binary: src/cmd/ has one module per command;
                      build.rs embeds the icon in the Windows executables
crates/kiln-brand/tools/outline.py  the wordmark's outlines, from the font
kiln.ico, docs/brand/ the icon and logos, drawn by kiln-brand's
                      write-icon example and checked by its tests
crates/kiln-check/examples/bench.rs   the benchmark gate (check and export)
crates/*/golden/      golden files (KILN_BLESS=1 rewrites them)
scripts/ci.ps1        the gates, locally and in CI (CI.md)
scripts/release.ps1   a release's steps: check, notes, package (CI.md)
scripts/wiki-links.ps1  checks the links between wiki pages (CI.md)
.github/workflows/    CI on Windows and Linux; releases from a tag; the wiki
docs/                 design (DESIGN, FORMAT, EXPORT, CONFLUENCE,
                      INTEGRATION) and user docs (CLI, CONFIGURATION,
                      wiki/: the user guide published to the GitHub wiki)
```


## Crates

The same shape as Fenix and Flint: small crates with their own unit
tests; logic in pure crates, I/O only in the crates whose role is I/O.
"Pure" means no file system, network, process, clock or environment
access except through a value or trait the caller passes in.

| Crate | Role | Pure? | Built in |
|---|---|---|---|
| `kiln-model` | `kiln.toml` schema (export targets and `[confluence]` too), `ItemId`, `Item`, `Ref`, `CodeRef`, `Finding`, the rules; Kiln's folders; the presets as data (`presets/`, embedded with `include_str!`) | yes | K1, K2, K11 |
| `kiln-parse` | front matter and `kiln.toml` with spans (`toml_edit`), references in bodies (`pulldown-cmark`) and code (`regex`), comment-preserving edits and front-matter formatting; adding a preset to `kiln.toml` (`add_preset`, spliced so comments stay, nothing declared twice) | yes | K1, K4, K11 |
| `kiln-index` | the loaded project: walk (`ignore`), parse in parallel (`rayon`), graph, queries (including the facts `kiln show` and hover print), renumber and new-item plans; the editing commands' checks and writes (`plan_set`, `check_link`, `apply_*`, `create_item`, shared by the CLI, the GUI and the Flint provider); the item diff between two projects; the requirement levels as a tree (`level_trace`, K13); re-reading one file (`update_file`) | yes, given a file-source trait | K1 (incremental, K4) |
| `kiln-check` | the FORMAT.md rules and their output formats | yes | K1 |
| `kiln-render` | Markdown and templates → document tree (`pulldown-cmark`); report builders; Mermaid validation (`fenix-diagram`) | yes | K2 |
| `kiln-export` | assembly, numbering, layouts, reference resolution, Markdown, HTML and Word (`zip`, ADR-011), the file destination, diagram pictures and their cache | yes, except writing files and the cache | K2, K8 (Word) |
| `kiln-confluence` | storage emitter, normaliser, desired pages, planner, snapshots, the client (`ureq`); apply; reading and placing footer comments | emitter, normaliser and planner yes; client no | K2 (read), K3 (write) |
| `kiln-git` | the current commit, dirty state, last commit per file, shallow detection; a revision resolved to its commit and the files at it (one `ls-tree`, one `cat-file --batch`) | no (shells out to `git`) | K1, K2, K6 |
| `kiln-lsp` | `kiln lsp`: the protocol (`lsp-server`, `lsp-types`), open documents layered over the disk, features as functions of the project and a position, a thread watching the disk | no | K4 |
| `kiln-flint` | `kiln provider`: items and drill-downs as pure functions of the project (`items`), Confluence page lookup, the protocol loop with project following and polling | no | K5 |
| `kiln-fenix` | Fenix control channel: open at a line, clock in on an agenda task by Jira key; opening a URL with the OS | no | K1 (`new --open`), K5 |
| `kiln-secrets` | tokens from the environment (K2) or the OS credential store (K3); a `Token` that never formats | no | K2, K3 |
| `kiln` | the binary: `clap` CLI, exit codes, project discovery, reading and writing files; and `kilnw`, the GUI without a console on Windows (ADR-010) | no | K0 onward |
| `kiln-jira` | the Jira mirror: linked keys, the cached mirror and its freshness, K207 (pure); one fetch through `fenix-jira`, and the fake Jira for tests (`fake` feature) | mirror and K207 yes; fetch no | K8 |
| `kiln-brand` | the mark, wordmark and app icon (GUI.md, "Icon") drawn from one vector definition by a small rasteriser; the `.ico` with its `ico` feature (`image`) | yes | after K10 |
| `kiln-gui` | the GUI (GUI.md): pages as rows, the prompt, the application and Home (the projects dashboard) as state machines (pure, tested as text); edits through `kiln-index`; the list of projects in the state folder; the window (`winit`), GPU (`wgpu`) and text (`glyphon`) that draw a `Screen`, the logo drawn as one textured quad, and the threads that run the binary's jobs, open projects and summarise them for Home | pages, prompt, app and home yes; window no | K7, K9, K12 |

`kiln-gui` is the one crate that pulls in `wgpu` and `winit`, which is
why it waited for K7. It loads display libraries at run time (`dlopen`),
so it builds in a CI container with none.

## Dependency direction

The intended edges between Kiln crates (each added when the code that
needs it lands):

| Crate | Uses |
|---|---|
| `kiln-model` | no Kiln crate |
| `kiln-parse` | `kiln-model` |
| `kiln-index` | `kiln-model`, `kiln-parse` |
| `kiln-check` | `kiln-model`, `kiln-parse`, `kiln-index`, `kiln-render` (K111, K112) |
| `kiln-render` | `kiln-model`, `kiln-index` |
| `kiln-export` | `kiln-model`, `kiln-index`, `kiln-render` (findings arrive as data, so it doesn't need `kiln-check`) |
| `kiln-confluence` | `kiln-model`, `kiln-index`, `kiln-render`, `kiln-export`, `kiln-secrets` |
| `kiln-git`, `kiln-secrets`, `kiln-fenix` | `kiln-model` at most |
| `kiln-lsp` | `kiln-model`, `kiln-parse`, `kiln-index`, `kiln-check` |
| `kiln-flint` | `kiln-model`, `kiln-parse` (the status edit), `kiln-index`, `kiln-fenix`, `kiln-confluence` and `kiln-secrets` ("Open in Confluence") |
| `kiln-jira` | `kiln-model`, `kiln-index` |
| `kiln-brand` | no Kiln crate |
| `kiln-gui` | `kiln-model`, `kiln-parse` (the title limit), `kiln-index` (the project, and the editing commands' `plan_set`/`apply_*`), `kiln-check` (the Check page), `kiln-fenix` (opening an item or an address), `kiln-brand` (the window icon and Home's lockup), `kiln-jira` (the mirror's type and K207; the binary hands over the function that loads it, so the GUI never sees a token); exports, Confluence and git are the binary's, behind the `Host` trait, and opening a project from Home (its baseline from git) behind a function the binary hands over, so it needs no `kiln-export`, `kiln-confluence` or `kiln-git` |
| `kiln` | all of them; `kiln-brand` with `ico` in its build script too |

The rules:

- **Pure crates depend only on pure crates.** `kiln-model` depends on no
  Kiln crate. Nothing pure depends on `kiln-git`, `kiln-secrets` or
  `kiln-fenix`.
- **Facts from the outside come in as data.** The current commit, the
  last commit per file, a token: the binary (or the LSP, or the provider)
  gets them from the I/O crates and passes them to the pure ones, so
  exports, plans and checks are tested without a repository, a server or
  a credential store.
- **The front ends are thin.** `kiln`, `kiln-lsp` and `kiln-flint` own
  their protocol (arguments and exit codes, LSP, Flint's provider
  protocol) and call the same library functions, so the command line, the
  editor and the launcher never disagree.
- **Confluence is a back end, not a layer.** `kiln-export` knows nothing
  about it; `kiln-confluence` takes `kiln-export`'s split pages.

When a crate needs an edge this table doesn't have, change the table
here in the same pull request, with the reason.

## Dependencies

Every external dependency is declared once, in the workspace
`Cargo.toml`'s `[workspace.dependencies]`, and crates take it with
`workspace = true`, so versions are chosen in one place. `Cargo.lock` is
committed and CI builds with `--locked`.

Fenix and Flint crates are depended on by git and pinned `rev`, in the
workspace `Cargo.toml` only, and only those DESIGN.md lists. Each is
added by the milestone that first uses it, so the build never fetches a
crate nothing compiles:

| Crate | Used by | From |
|---|---|---|
| `fenix-storage` | atomic writes (`kiln-index`'s `write_back` for `set`, `link`, `renumber` and the Flint provider; `kiln new`; exports from K2) | K1, added |
| `fenix-control` | `kiln-fenix` (`kiln new --open`) | K1, added |
| `fenix-diagram` | `kiln-render` (validation), `kiln-export` (pictures) | K2, added |
| `flint-protocol` | `kiln-flint`; the benchmark (`kiln-check`'s example, for the message limit) | K5, added |
| `fenix-keymap` | `kiln-gui` (the `SPC` leader's key trie) | K7, added |
| `fenix-jira` | `kiln-jira` (the mirror's searches) | K8, added |

Never `fenix-gui`, `fenix-project` or `fenix-notebook`.

## How a command runs

```
kiln <command>
  └─ project::load ── find_root (walk up to kiln.toml)
       └─ Project::load(DiskSource)           kiln-index
            ├─ parse_config                   kiln-parse   → K001–K004
            ├─ list + parse_item, in parallel kiln-parse   → K100, K104
            └─ list + scan_code, in parallel  kiln-parse
  └─ query or check                           kiln-index / kiln-check
  └─ edit: plan (pure) → read, check stamp, splice/toml_edit, write atomically
```

Every producer emits findings at its rule's default severity;
`kiln_check::check` applies `[check]` once and sorts. Edits are pure
text-to-text functions in `kiln-parse` (or plans in `kiln-index`); only
the binary touches the disk, through `project::write_back`, which refuses
a file whose length or modification time changed since it was read and
restores the file's line ending and byte-order mark.

## How an export runs

```
kiln export
  ├─ check (findings for the report; K005/K112 stop the target)  kiln-check
  ├─ git facts: commit, date, last commit per file, shallow?, ignored?  kiln-git
  ├─ prepare: assemble (template or default) → split by layout  kiln-export
  ├─ resolve references across pages and link_to targets         kiln-export
  └─ per target:
       markdown/html → render_files → plan_files → apply         kiln-export
       confluence    → snapshot (client or --snapshot; page cache) →
                       desired pages (storage, renderHash) → plan →
                       confirm (--yes or a prompt) → apply      kiln-confluence
```

`apply` takes the client as a `&dyn Confluence` and the attachment bytes
as a function, so the binary hands it `FenixPictures` and the disk while
the tests hand it the fake server and `FakePictures`. The fake server is
`kiln_confluence::fake`, compiled for the crate's own tests and, behind
the `fake` feature, for the binary's end-to-end tests; nothing else
enables it.

Assembly gets the revision as data (`Revision`: commit, date, whether
the output is in-tree), and back ends get repository links as data
(`RepoLinks`: relative, web with each file's last commit, or none), so
none of it touches git. Diagram pictures come through a `Pictures` trait:
`FenixPictures` draws and caches, `FakePictures` keeps golden files
independent of a machine's fonts.

## Lints and conventions in code

Set in the workspace `Cargo.toml` and inherited by every crate with
`[lints] workspace = true`:

- `unsafe_code` is denied. A platform call that truly needs it allows it
  on that item, next to a comment saying why.
- `clippy::unwrap_used` warns (and CI denies warnings) outside tests:
  bad input from files, config, servers or peers is an error with a
  one-line message, never a panic. Tests may unwrap (`clippy.toml`).
- `cargo fmt` with default settings.

Text files are checked out with LF on every platform (`.gitattributes`),
because fixtures and golden files are compared byte for byte on Windows
and Linux. Test fixtures that need CRLF mark themselves `-text`.

## Platforms

Windows is the primary platform and Linux the second; CI runs every gate
on both ([CI.md](CI.md)). Every crate builds headless, and every command
but `kiln gui` runs headless, because Kiln is also a CI tool that runs in
Linux containers: no display, no console window, no desktop session
assumed. Without a display `kiln gui` fails with one line, like any other
error.
