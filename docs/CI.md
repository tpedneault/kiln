# Continuous integration

The `CI` workflow (`.github/workflows/ci.yml`) runs on pushes, pull
requests and manual dispatch. It needs no secrets and no live services.
Actions are pinned to commit hashes, the token has read-only access, and
dependencies resolve from the committed `Cargo.lock` (`--locked`). The
compiler follows Rust stable; each run records the exact `rustc` and
`cargo` versions so a toolchain change can be diagnosed.

## Gates

Both jobs run the same script, `scripts/ci.ps1`, over the whole
workspace; Kiln has no crate that needs a display before K7.

| Gate | Command |
|---|---|
| toolchain | `rustc --version --verbose`, `cargo --version` (recorded, not judged) |
| format | `cargo fmt --all --check` |
| lint | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| tests | `cargo test --locked --workspace --no-fail-fast` |
| dogfood | `kiln check` on this repository's own `kiln.toml` and `design/` (errors fail; warnings are listed) |
| dogfood export | `kiln export --check`: `docs/generated/DESIGN.md` is what `kiln export` would write now |
| benchmark | `cargo run --release -p kiln-check --example bench -- --gate`: `kiln check` on a generated 2,000-item, 50,000-file project, failing above 3 s (50% over the 2 s budget); and `kiln export --dry-run` of that project as one document, failing above 4.5 s (50% over 3 s) |

| Job | Runner | Notes |
|---|---|---|
| Windows | `windows-2022` (MSVC) | primary platform; checked out into a path with a space |
| Linux | `ubuntu-24.04` | headless, as in a CI container |

When the export gate fails, run `kiln export` and commit
`docs/generated/DESIGN.md`. It shows code locations with line numbers, so
editing code above a `// implements:` or `// verifies:` comment makes it
stale too; that is the gate doing its job. The benchmark generates its project once into `target/kiln-bench`
(about 3 s on Linux, longer on Windows), then measures four runs; the gate
is the median of the last three. Kiln's own tests need `git` on the PATH
(`kiln-git`'s test makes a repository). Golden files (`crates/*/golden/`)
are compared byte for byte; after a deliberate change run the tests with
`KILN_BLESS=1` and review the diff. The Confluence client is tested
against an in-process fake server on a local port; nothing in CI talks to
a real Confluence.

## Run locally

Install Rust stable with Clippy and rustfmt, git, and PowerShell 7. On
Windows, also the Visual Studio C++ Build Tools. From the repository
root:

```powershell
pwsh -NoProfile -File ./scripts/ci.ps1
```

The script uses the active toolchain and fails if any gate fails. Each
run writes its logs to a new folder under `target/ci/`, with a
`summary.json` of exit codes and timings; CI uploads that folder even
when a gate fails.

## Limits

Tests marked `#[ignore]` need a live service (Confluence from K3) or a
manual setup and are documented where they live; CI does not run them.
Behaviour inside Fenix and Flint, and against real Confluence instances,
is checked by hand (CLAUDE.md, "What only Thomas can verify").
