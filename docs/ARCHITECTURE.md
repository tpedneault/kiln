# Architecture

Where Kiln's code lives and which way its dependencies point.
[DESIGN.md](DESIGN.md) says what Kiln does and why; this document is the
map of the workspace that does it, and is updated whenever a crate gains
a role or a dependency.

Status: K0. Every crate below exists with its module header (what it is
for, what it deliberately doesn't do, the milestone that fills it in)
and no code yet, except the `kiln` binary, which has `--help` and
`--version`.

## Layout

```
Cargo.toml            the workspace: members, shared package metadata,
                      every external dependency (and later the Fenix and
                      Flint pins), lints
clippy.toml           test-only lint relaxations
crates/kiln-*/        library crates, one role each
crates/kiln/          the binary
scripts/ci.ps1        the gates, locally and in CI (CI.md)
.github/workflows/    CI on Windows and Linux
docs/                 design (DESIGN, FORMAT, EXPORT, CONFLUENCE,
                      INTEGRATION) and user docs (CLI, later CONFIGURATION)
```

From K1 the repository also holds its own Kiln project (`kiln.toml`,
`design/`), checked by `kiln check` in CI; from K2 a template under
`design/templates/` and a committed Markdown export kept current by
`kiln export --check`.

## Crates

The same shape as Fenix and Flint: small crates with their own unit
tests; logic in pure crates, I/O only in the crates whose role is I/O.
"Pure" means no file system, network, process, clock or environment
access except through a value or trait the caller passes in.

| Crate | Role | Pure? | Filled in |
|---|---|---|---|
| `kiln-model` | `kiln.toml` schema, `ItemId`, `Item`, `Link`, `Finding`; Kiln's folders | yes | K1 |
| `kiln-parse` | front matter and `kiln.toml` with spans (`toml_edit`), references in bodies and code | yes | K1 |
| `kiln-index` | the loaded project: walk, parse, graph, queries, incremental update | yes, given a file-source trait | K1 |
| `kiln-check` | the FORMAT.md rules and their output formats | yes | K1 |
| `kiln-render` | Markdown → document tree; report builders | yes | K2 |
| `kiln-export` | targets, templates, assembly, layouts, diagrams, Markdown and HTML | yes, except writing files | K2 |
| `kiln-confluence` | storage emitter, normaliser, planner, client, apply | emitter and planner yes; client no | K2 (read), K3 (write) |
| `kiln-git` | commits, dirty state, last commit per file, shallow detection, revisions | no (shells out to `git`) | K1, K2, K6 |
| `kiln-lsp` | `kiln lsp` | no | K4 |
| `kiln-flint` | `kiln provider` | no | K5 |
| `kiln-fenix` | Fenix control channel: open at a line, agenda | no | K1 (`new --open`), K5 |
| `kiln-secrets` | tokens from the environment or the OS credential store | no | K3 |
| `kiln` | the binary: `clap` CLI, exit codes, project discovery | no | K0 onward |
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
| `kiln-check` | `kiln-model`, `kiln-index` |
| `kiln-render` | `kiln-model`, `kiln-index` |
| `kiln-export` | `kiln-model`, `kiln-index`, `kiln-render`, `kiln-check` (the findings report) |
| `kiln-confluence` | `kiln-model`, `kiln-render`, `kiln-export` |
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
| `fenix-storage` | atomic writes (`kiln set`, `new`, exports) | K1 |
| `fenix-control` | `kiln-fenix` | K1 |
| `fenix-diagram` | `kiln-export` | K2 |
| `flint-protocol` | `kiln-flint` | K5 |
| `fenix-keymap` | `kiln-gui` | K7 |
| `fenix-jira` | the Jira mirror | K8 |

Never `fenix-gui`, `fenix-project` or `fenix-notebook`.

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
