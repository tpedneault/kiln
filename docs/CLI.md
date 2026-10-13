# Command line

How to run `kiln`, as it stands (K2). The commands planned for later
milestones are in [DESIGN.md](DESIGN.md), "Command line"; each is
documented here when it ships. The file format is
[FORMAT.md](FORMAT.md); `kiln.toml` is summed up in
[CONFIGURATION.md](CONFIGURATION.md).

`kiln` finds the project by walking up from the current folder to the
nearest `kiln.toml`. `--project <DIR>` starts the walk from `DIR`
instead, for every command.

Results go to stdout, messages to stderr, so `kiln list --format json |
…` and `kiln check --format gitlab > report.json` stay clean.

## Starting

### `kiln init [--name NAME]`

Writes `kiln.toml` with FORMAT.md's default types and links, and a
`design/` folder with a README. `NAME` defaults to the folder's name.
Never overwrites: with a `kiln.toml` already there it stops (exit 2). Add
a `[code]` section yourself to have source code scanned for code
references.

### `kiln new TYPE [--parent ID] [--title TEXT] [--open]`

Creates an item with the next free ID for `TYPE` (one above the highest
used, closed items included) and prints its path:

```
$ kiln new requirement --title "Flint appears within 50 ms of the hotkey" --parent REQ-002
design/requirements/REQ-014-flint-appears-within-50-ms-of-the-hotkey.md
```

The file goes in `<first of paths>/<type plural>/`, named by ID and the
title (up to 50 characters of it). The front matter has `id`, `type`,
`title`, the type's `initial` status and `parent`; the body is the type's
`template` file when it has one. Without `--title` the title is
"Untitled <type>". The parent must exist and be of an allowed type.

`--open` opens the file at its title: in the running Fenix (through its
control channel), else by starting `fenix <path>:4`, else, when Fenix
isn't installed, with the operating system's default editor; stderr says
which. The item is created either way.

## Reading

### `kiln list [--type TYPE] [--status STATUS] [--tag TAG] [--format text|json]`

Items in type order (as `kiln.toml` lists types), then by ID; the filters
combine.

```
$ kiln list --type requirement
REQ-001  approved  The repository is the source of truth
REQ-002  approved  Readable without Kiln
```

JSON gives each item's `id`, `type`, `title`, `status`, `parent`, `tags`,
`path` and `line`.

### `kiln show ID [--format text|json]`

One item with everything Kiln computes about it: parent and children,
tags, Jira keys, owner, due date, outgoing links by role, incoming links
under their inverse names ("verified by", including code locations),
items that mention it, coverage for each coverage role, and progress for
a milestone.

```
$ kiln show REQ-013
REQ-013 Edits keep the author's formatting and never fight an editor
requirement · verified · design/requirements/REQ-013-….md:2
parent          REQ-001 The repository is the source of truth
implemented by  crates/kiln-parse/src/edit.rs:58, crates/kiln/src/project.rs:85
verified by     crates/kiln-parse/src/edit.rs:173, crates/kiln-parse/src/edit.rs:195, …
coverage        implements: yes, verifies: yes
```

### `kiln trace [--type TYPE] [--format text|json|csv]`

Coverage of every non-closed item of `TYPE` (default `requirement`): for
each coverage role, the open items and code locations that link to it,
and the gaps. An item covered only because all its open children are says
"(through its children)". The text and JSON forms name the commit
(`HEAD`) when the project is in a git repository. CSV has one row per
item, lists joined with `; `.

### `kiln impact ID [--format text|json]`

Everything that reaches `ID` by a link or a parent edge, transitively,
breadth first, each with its path:

```
$ kiln impact REQ-001
REQ-013                            child of REQ-001 — REQ-013 Edits keep the author's formatting…
crates/kiln-parse/src/edit.rs:58   implements REQ-013, child of REQ-001
crates/kiln-parse/src/edit.rs:173  verifies REQ-013, child of REQ-001
```

## Checking

### `kiln check [--format text|json|gitlab|github]`

Runs the rules of FORMAT.md, "Check rules", at the project's severities,
and prints the findings sorted by file, line and column, then a summary
on stderr (`kiln: 2 errors, 1 warning`). Exit 1 when any finding is an
error, 0 otherwise (warnings don't fail).

| Format | For |
|---|---|
| `text` | people: `path:line:col: K102 error: REQ-099 doesn't exist` |
| `json` | tools: an array of `{code, severity, path, line, column, message}` |
| `gitlab` | a GitLab Code Quality report, so findings show on merge requests |
| `github` | GitHub Actions workflow commands, so findings annotate the diff |

A `kiln.toml` Kiln can't read at all (K001, or a newer `format`) is
reported as a finding in the chosen format, with exit 1, so CI shows it.

## Exporting

### `kiln export [NAME…] [--dry-run | --check] [--format text|json]`

Renders every export target in `kiln.toml` (or the named ones) and writes
the file targets whose bytes changed; see [EXPORT.md](EXPORT.md) for
targets, templates, layouts and formats.

```
$ kiln export --dry-run
design-md (markdown, document) → docs/generated/DESIGN.md
  would update docs/generated/DESIGN.md
  leaves out: TEST-004
```

| Option | Does |
|---|---|
| `--dry-run` | renders everything and prints what would change; writes nothing |
| `--check` | like `--dry-run`, and exits 1 if anything would change (for committed exports in CI) |
| `--require-clean` | refuses when the working tree has uncommitted changes |
| `--allow-shallow` | exports from a shallow clone; files older than its boundary link at the exported commit |
| `--snapshot FILE` | Confluence: plans against a recorded snapshot instead of the live space |
| `--save-snapshot FILE` | Confluence: writes the snapshot a live dry run read |
| `--format json` | the outcome per target as JSON (the Confluence plan included) |

File targets write atomically and only what changed, and remove the files
they wrote earlier and no longer produce. "leaves out" lists open items
no part of the document placed: an omission should be a decision.

Confluence targets only plan until K3: `kiln export --dry-run` reads the
space with `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER` on Cloud)
and prints each page's operation and why:

```
spec (confluence, document) → space FLINT, page 123456789
  update    "Flint — design document": REQ-014 changed; REQ-031 new (42 KiB)
  0 create, 1 update, 0 unchanged, 0 drift, 0 retire
```

Without `--dry-run` or `--check`, a Confluence target is refused (exit 2).
A plan that would be refused (a page edited in Confluence, a page too
large) exits 1; a server or network error exits 3.

## Editing

The editing commands change only what they are about, keep comments,
layout and the file's line endings, write atomically, and refuse a file
that changed on disk since Kiln read it (exit 1; run the command again).
They refuse a duplicated ID: renumber it first.

### `kiln set ID KEY=VALUE…`

Sets `status`, `title`, `parent`, `owner` or `due` (`YYYY-MM-DD`). An
empty value removes `parent`, `owner` or `due`. Values are checked first:
a status of the item's type, an existing parent of an allowed type that
doesn't make a cycle, a real date.

```
$ kiln set REQ-014 status=approved due=2026-11-30
REQ-014: status draft → approved
REQ-014: due (none) → 2026-11-30
```

### `kiln link ID ROLE TARGET` and `kiln unlink ID ROLE TARGET`

Adds `TARGET` to `[links] ROLE` of `ID` (creating either as needed), or
removes it (dropping the role, and `[links]`, when they empty). `link`
checks the role exists, allows `ID`'s type as its source and `TARGET`'s
as its target; written on the inverse side (`verified-by`), it says which
command to run instead. Linking twice is not an error.

### `kiln renumber ID --to NEW [--file PATH]`

Moves an item to another ID of the same prefix and rewrites every
reference Kiln counts: `parent` and `[links]` in front matter, mentions in
bodies, code references. Renames the file when its name starts with the
old ID. Every file is checked unchanged before any is written.

When `ID` is duplicated (two branches allocated the same number),
`--file` picks which item moves; only its own ID and file name change,
and the references, which may mean either, are listed on stderr to check
by hand.

```
$ kiln renumber REQ-015 --to REQ-016 --file design/requirements/REQ-015-other.md
REQ-015 → REQ-016: 0 references in 0 other files; design/requirements/REQ-015-other.md → design/requirements/REQ-016-other.md
kiln: left alone: design/tests/TEST-004-x.md:7 (may mean either REQ-015)
```

## Other

| Command | Does |
|---|---|
| `kiln --help`, `kiln <command> --help` | usage |
| `kiln --version` | `kiln <version>` |

`kiln` with no arguments prints usage and exits 2 (from K7 it opens the
GUI).

## Errors and exit codes

An error is one line on stderr, starting `kiln: `, naming what failed and
why (with the file and line when there is one):

```
$ kiln link REQ-001 verifies REQ-002
kiln: a requirement can't be the source of `verifies` (from: test, code)
```

| Exit code | Means |
|---|---|
| 0 | fine |
| 1 | `kiln check` found an error, `kiln export --check` found changes, a Confluence plan was refused, or a change was refused (a file changed on disk) |
| 2 | usage or configuration error: unknown command or ID, no `kiln.toml`, a `kiln.toml` Kiln can't read, a value the configuration doesn't allow |
| 3 | network or server error (Confluence) |
