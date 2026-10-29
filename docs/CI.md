# Continuous integration

The `CI` workflow (`.github/workflows/ci.yml`) runs on pushes, pull
requests and manual dispatch. It needs no secrets and no live services.
Actions are pinned to commit hashes, the token has read-only access, and
dependencies resolve from the committed `Cargo.lock` (`--locked`). The
compiler follows Rust stable; each run records the exact `rustc` and
`cargo` versions so a toolchain change can be diagnosed.

## Gates

Both jobs run the same script, `scripts/ci.ps1`, over the whole
workspace. `kiln-gui` builds headless on both (winit and wgpu load the
display and GPU libraries at run time); no test opens a window: the GUI's
pages and keys are tested as a state machine producing text.

| Gate | Command |
|---|---|
| toolchain | `rustc --version --verbose`, `cargo --version` (recorded, not judged) |
| format | `cargo fmt --all --check` |
| lint | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| tests | `cargo test --locked --workspace --no-fail-fast` |
| dogfood | `kiln check` on this repository's own `kiln.toml` and `design/` (errors fail; warnings are listed) |
| dogfood export | `kiln export --check`: `docs/generated/DESIGN.md` is what `kiln export` would write now |
| wiki links | `scripts/wiki-links.ps1`: every link between pages in `docs/wiki/` names a page that exists, and every `#anchor` a heading on it |
| benchmark | `cargo run --release -p kiln-check --example bench -- --gate`: `kiln check` on a generated 2,000-item, 50,000-file project, failing above 3 s (50% over the 2 s budget); `kiln export --dry-run` of that project as one document, failing above 4.5 s (50% over 3 s); and a hundred keystrokes in one item file, each re-read alone and the project checked again as `kiln lsp` does, failing when the p95 is above 75 ms (50% over 50 ms); and the Flint provider's drill-down answers (`status`, `impact`, `all`), failing when the p95 is above 24 ms (50% over 16 ms) or the 2,000 static items come within 64 KiB of Flint's 4 MiB message limit |

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
a real Confluence. The language server is tested through `lsp-server`'s
in-memory connection, and once as a process over stdio; one test waits
about 3 s for its disk watcher. The Flint provider is tested with the test
as Flint over in-process pipes, using `flint-protocol`'s framing and
types, and once as a process.

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

## The wiki

The [wiki](https://github.com/tpedneault/kiln/wiki) is the user guide. Its
source is `docs/wiki/`, one Markdown file per page, the file name being the
page name (`Item-files.md` is the page "Item files"; `_Sidebar.md` and
`_Footer.md` are GitHub's sidebar and footer). Pages link to each other as
`[text](Page-Name)` or `[text](Page-Name#heading)`, without `.md`, which
is how GitHub wikis link; the wiki-links gate checks every one.

The `Wiki` workflow (`.github/workflows/wiki.yml`) publishes it: on each
push to `master` that changes `docs/wiki/` (or when run by hand), it
replaces the wiki's pages with those files and pushes, as
`github-actions[bot]`, with the workflow's token. Edits made on GitHub's
wiki pages are overwritten, so change the wiki in a pull request. The wiki
must exist before the first run: open the repository's Wiki tab and
create any page once.

## Releasing

The `Release` workflow (`.github/workflows/release.yml`) runs when a tag
`vX.Y.Z` is pushed, or when it is run by hand on `master` with a version
(Actions → Release → Run workflow), and nothing else publishes. `scripts/release.ps1`
holds its steps, so they run the same locally:

1. **prepare**: `release.ps1 -Step check` fails unless the tag's version
   is the `[workspace.package] version` in `Cargo.toml` and
   `CHANGELOG.md` has a dated `## [X.Y.Z] - YYYY-MM-DD` section, so the
   tag, `kiln --version` and the notes agree. It then drafts the release
   with that section as its notes (`-Step notes`, relative links pointed
   at the tag). A version with a suffix (`v1.0.0-rc.1`) is a pre-release.
2. **build**, on `windows-2022` and `ubuntu-22.04`: the same gates as CI
   (`scripts/ci.ps1`), then `-Step package`: `cargo build --release
   --locked -p kiln`, an archive `kiln-<version>-<target>` (`.zip` on
   Windows, `.tar.gz` on Linux) with `kiln`, `kilnw`, the readme,
   changelog and licences, a `.sha256` beside it, and a check that the
   archived `kiln --version` says the tagged version. Both are uploaded
   to the draft. Linux builds on the older runner so the binary needs
   glibc 2.35 at most (Ubuntu 22.04, Debian 12 and newer); it links only
   libc, libm and libgcc, the display and GPU libraries being loaded at
   run time by the GUI alone.
3. **publish**: undrafts the release once every archive is there.

To release:

1. On a branch, set `version` in `Cargo.toml`'s `[workspace.package]`
   (and let `cargo` update `Cargo.lock`), and move everything under
   `## [Unreleased]` into a new `## [X.Y.Z] - <today>` section below it,
   leaving `## [Unreleased]` empty. Merge it.
2. Tag the merge commit and push the tag: `git tag vX.Y.Z && git push
   origin vX.Y.Z`. Or, without pushing a tag (from a session that may
   push only its own branch, say): run the workflow by hand on `master`
   with the version `X.Y.Z`. After the same check, `prepare` creates the
   tag `vX.Y.Z` on `master`'s head through the API, and the run goes on
   to build and publish from that tag. It goes on in the same run because
   a tag pushed with the workflow's own token starts no other workflow;
   and it refuses a tag that already exists, so a version is released
   once.

When a step fails, the release stays a draft: delete it and the tag
(`git push origin :refs/tags/vX.Y.Z`), fix the cause on `master`, and tag
again. The token the workflow uses can write releases (`contents:
write`) only in its three jobs; nothing else in either workflow can.

## Limits

Tests marked `#[ignore]` need a live service or a manual setup and are
documented where they live; CI does not run them. The live Confluence
tests are `crates/kiln/tests/live.rs`, set up as
[dev/confluence/README.md](../dev/confluence/README.md) describes.
Behaviour inside Fenix and Flint, and against real Confluence instances,
is checked by hand (CLAUDE.md, "What only Thomas can verify").
