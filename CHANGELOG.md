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

### Changed

- The file format: IDs are written with at least three digits and one
  spelling per number (`REQ-007`, not `REQ-7`); `[code] exclude`; the
  default code pattern; which rules apply when
  ([docs/FORMAT.md](docs/FORMAT.md)).
