# Architecture

Where Kiln's code lives and which way its dependencies point.
[DESIGN.md](DESIGN.md) says what Kiln does and why; this document is the
map of the workspace that does it, and is updated whenever a crate gains
a role or a dependency.

Status: K2. The format crates (`kiln-model`, `kiln-parse`, `kiln-index`,
`kiln-check`), the export crates (`kiln-render`, `kiln-export`, and
`kiln-confluence` without writes), `kiln-git`, `kiln-secrets` (environment
variables), `kiln-fenix` (opening a file) and the binary's K1 and K2
commands are built. `kiln-lsp` and `kiln-flint` are shells with their
module header (what each is for, what it deliberately doesn't do, the
milestone that fills it in).

## Layout

```
Cargo.toml            the workspace: members, shared package metadata,
                      every external dependency (and later the Fenix and
                      Flint pins), lints
clippy.toml           test-only lint relaxations
kiln.toml, design/    Kiln's own Kiln project, checked in CI (K1 dogfooding)
docs/generated/       its design document, exported and checked in CI (K2)
crates/kiln-*/        library crates, one role each
crates/kiln/          the binary: src/cmd/ has one module per command
crates/kiln-check/examples/bench.rs   the benchmark gate (check and export)
crates/*/golden/      golden files (KILN_BLESS=1 rewrites them)
scripts/ci.ps1        the gates, locally and in CI (CI.md)
.github/workflows/    CI on Windows and Linux
docs/                 design (DESIGN, FORMAT, EXPORT, CONFLUENCE,
                      INTEGRATION) and user docs (CLI, later CONFIGURATION)
```


## Crates

The same shape as Fenix and Flint: small crates with their own unit
tests; logic in pure crates, I/O only in the crates whose role is I/O.
"Pure" means no file system, network, process, clock or environment
access except through a value or trait the caller passes in.

| Crate | Role | Pure? | Built in |
|---|---|---|---|
| `kiln-model` | `kiln.toml` schema (export targets and `[confluence]` too), `ItemId`, `Item`, `Ref`, `CodeRef`, `Finding`, the rules; Kiln's folders | yes | K1, K2 |
| `kiln-parse` | front matter and `kiln.toml` with spans (`toml_edit`), references in bodies (`pulldown-cmark`) and code (`regex`), comment-preserving edits | yes | K1 |
| `kiln-index` | the loaded project: walk (`ignore`), parse in parallel (`rayon`), graph, queries, renumber plans; incremental update | yes, given a file-source trait | K1 (incremental, K4) |
| `kiln-check` | the FORMAT.md rules and their output formats | yes | K1 |
| `kiln-render` | Markdown and templates → document tree (`pulldown-cmark`); report builders; Mermaid validation (`fenix-diagram`) | yes | K2 |
| `kiln-export` | assembly, numbering, layouts, reference resolution, Markdown and HTML, the file destination, diagram pictures and their cache | yes, except writing files and the cache | K2 |
| `kiln-confluence` | storage emitter, normaliser, desired pages, planner, snapshots, the client (`ureq`); apply | emitter, normaliser and planner yes; client no | K2 (read), K3 (write) |
| `kiln-git` | the current commit; later dirty state, last commit per file, shallow detection, revisions | no (shells out to `git`) | K1, K2, K6 |
| `kiln-lsp` | `kiln lsp` | no | K4 |
| `kiln-flint` | `kiln provider` | no | K5 |
| `kiln-fenix` | Fenix control channel: open at a line, agenda | no | K1 (`new --open`), K5 |
| `kiln-secrets` | tokens from the environment (K2) or the OS credential store (K3); a `Token` that never formats | no | K2, K3 |
| `kiln` | the binary: `clap` CLI, exit codes, project discovery, reading and writing files | no | K0 onward |
| `kiln-gui` | the GUI | no | K7 (not in the workspace until then) |

`kiln-gui` is deliberately absent rather than an empty shell: it is the
one crate that will pull in `wgpu` and `winit`, and nothing before K7
depends on it.

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
| `kiln-flint` | `kiln-model`, `kiln-index`, `kiln-fenix`, `kiln-confluence` and `kiln-secrets` ("Open in Confluence") |
| `kiln` | all of them |

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
| `fenix-storage` | atomic writes (`kiln`: `set`, `new`, `link`, `renumber`; exports from K2) | K1, added |
| `fenix-control` | `kiln-fenix` (`kiln new --open`) | K1, added |
| `fenix-diagram` | `kiln-render` (validation), `kiln-export` (pictures) | K2, added |
| `flint-protocol` | `kiln-flint` | K5 |
| `fenix-keymap` | `kiln-gui` | K7 |
| `fenix-jira` | the Jira mirror | K8 |

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
       confluence    → snapshot (client or --snapshot) →
                       desired pages (storage, renderHash) → plan  kiln-confluence
```

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
on both ([CI.md](CI.md)). Every crate except `kiln-gui` (K7) builds and
runs headless, because Kiln is also a CI tool that runs in Linux
containers: no display, no console window, no desktop session assumed.
