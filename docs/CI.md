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

| Job | Runner | Notes |
|---|---|---|
| Windows | `windows-2022` (MSVC) | primary platform; checked out into a path with a space |
| Linux | `ubuntu-24.04` | headless, as in a CI container |

Gates still to come: the benchmark gate (K1, DESIGN.md "Performance"),
`kiln check` on this repository's own `design/` folder (K1), and
`kiln export --check` on its committed design document (K2).

## Run locally

Install Rust stable with Clippy and rustfmt, and PowerShell 7. On
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
