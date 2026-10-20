# Command line

How to run `kiln`, as it stands (K9). The commands planned for later
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

### `kiln init [--name NAME] [--fenix]`

Writes `kiln.toml` with FORMAT.md's default types and links, and a
`design/` folder with a README. `NAME` defaults to the folder's name.
Never overwrites: with a `kiln.toml` already there it stops (exit 2). Add
a `[code]` section yourself to have source code scanned for code
references.

`--fenix` also registers `kiln lsp` in `.fenix/tools.json` as an
additional language server (`"lsp": {"additional": [{"name": "kiln",
"executable": "kiln", "args": ["lsp"], "languages": [...]}]}`), beside each
language's own server, for Markdown and the language of every code file
`[code] paths` scans. It merges into an existing file, keeps languages
added by hand, and moves the older form (Kiln as the Markdown server) over.
In a project that already has a `kiln.toml`, `kiln init --fenix` does only
that; run it again after adding `[code] paths` to add their languages. It
needs a Fenix with additional servers (`471ed09` or later).

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

### `kiln list [--type TYPE] [--status STATUS] [--tag TAG] [--jira] [--format text|json]`

Items in type order (as `kiln.toml` lists types), then by ID; the filters
combine.

```
$ kiln list --type requirement
REQ-001  approved  The repository is the source of truth
REQ-002  approved  Readable without Kiln
```

JSON gives each item's `id`, `type`, `title`, `status`, `parent`, `tags`,
`path` and `line`.

`--jira` adds each linked ticket's status and assignee from the Jira
mirror (read-only, cached ten minutes; INTEGRATION.md, "Jira"), and in
JSON a `jira` array of `{key, summary, status, resolved, assignee}` (or
`{key, missing: true}` for a key Jira doesn't know). It needs `[jira] url`
and a token: `KILN_JIRA_TOKEN`, or the one `kiln auth jira` stored.

```
$ kiln list --type task --jira
TASK-003  doing  Measure the hotkey  [FLINT-88 In Review · Alice]
TASK-004  done   Ship the loader     [FLINT-90 Done]
```

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

### `kiln diff REV[..REV] [--type TYPE] [--format text|json]`

The items added, changed and removed between two revisions, or between
a revision and the working tree (uncommitted edits included) when only one
is given. A revision is a tag, branch or commit; the project there is read
from git, not checked out. What counts as a change is FORMAT.md,
"Baselines".

```
$ kiln diff v1.0
v1.0 (1a2b3c4) → the working tree
~ REQ-014 Fast start: status approved → implemented; + mitigates RISK-002; body
+ REQ-031 Telemetry export (requirement)
- REQ-007 Old idea (requirement)
1 added, 1 changed, 1 removed
```

`--type` keeps one type; `--format json` gives each item's change and
details. A revision git doesn't know is a usage error (exit 2).

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

K207 (off by default) compares items with their Jira tickets, from the
Jira mirror as `kiln list --jira` reads it. When Jira can't be read (no
token, no network, Jira Cloud), stderr says `kiln: K207 not checked: …`
and the other rules decide the exit code: Kiln works without Jira.

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
| `--yes` | Confluence: publishes without asking (needed when not on a terminal, as in CI) |
| `--force` | Confluence: overwrites pages edited in Confluence (after their edit is in the repository) |
| `--adopt` | Confluence: publishes into a `page =` that already has content, replacing it |
| `--prune-removed DAYS` | Confluence: sends pages retired more than DAYS days ago to the space's trash |
| `--since REV` | marks each item new or changed since REV (a tag, branch or commit) and fills the `changes` report (EXPORT.md, "Reports") |
| `--format json` | the outcome per target as JSON (the Confluence plan included) |

File targets write atomically and only what changed, and remove the files
they wrote earlier and no longer produce. "leaves out" lists open items
no part of the document placed: an omission should be a decision.

A Confluence target reads the space with `KILN_CONFLUENCE_TOKEN` (and
`KILN_CONFLUENCE_USER` on Cloud), or the token `kiln auth confluence`
stored, prints each page's operation and why, and then publishes:

```
$ kiln export --yes
spec (confluence, document) → space FLINT, page 123456789 "Flint"
  update    "Flint — design document": REQ-014 changed; REQ-031 new (42 KiB)
  0 create, 1 update, 0 unchanged, 0 drift, 0 retire
spec: published 0 created, 1 updated, 0 retired, 0 pruned
```

The first line names the page the document goes under (or the adopted
page) by ID and title, so a wrong `parent` shows before anything is
published; JSON has the title as `parentTitle`. `--dry-run` stops after
the plan. Without `--yes`, a run on a terminal
asks before publishing each target; a run that isn't on a terminal stops
after the plan and exits 1. `--snapshot FILE` plans offline and works
only with `--dry-run` or `--check`. A plan that is refused (a page edited
in Confluence, a page too large, an adopted page with content) publishes
nothing of that target and exits 1; a server or network error exits 3,
and the next run finishes what the failed one started. `--format json`
adds what was published under `applied` (publishing with JSON output
needs `--yes`). CONFLUENCE.md has the details: drift, retired pages, the
page cache.

## Editing in an editor

### `kiln lsp`

A language server on stdin and stdout, for Fenix (`kiln init --fenix`
sets it up) or any editor that speaks LSP. It finds the project from the
editor's workspace folder, or from the first file opened, walking up to
`kiln.toml`. In item files:

| Request | Gives |
|---|---|
| completion | IDs after `[[`, in `parent = "` (types allowed as parent) and in `[links]` arrays (types the role allows, not already listed); statuses after `status = "`; types after `type = "`; tags already used; link roles at the start of a `[links]` line |
| hover | on any ID: what `kiln show` prints for it |
| definition | the item's `id` line (every copy, when duplicated) |
| references | every link, `parent`, mention and code reference to the item (and its `id`, when asked) |
| rename | `kiln renumber`: every reference, and the file name when the editor can rename files |
| diagnostics | `kiln check` for the open files and `kiln.toml`, after each edit |
| code actions | set status; add a link to the ID under the cursor; new child; create a test (or design, task…) covering this item; rename the file to match its ID |
| formatting | front-matter keys in FORMAT.md's order, strings in `"…"`; the body untouched |

It never writes a file: every change comes back as an edit the editor
applies. Actions that create or rename a file are offered only to editors
that can apply such edits; others get them disabled, naming the command
to run instead (`kiln new … --open`). Fenix can't yet (INTEGRATION.md,
G7). Files changed outside the editor are noticed within a few seconds.
A Markdown file outside the project gets empty answers. Messages from
the server itself go nowhere: stdout is the protocol.

Run beside a code file's own server (Fenix's additional servers, or any
editor that allows several servers per file), it also answers in the code
files `[code] paths` scans: IDs complete after a role word
(`// implements: REQ-0`), IDs have hover, definition and references, and
code references that name a missing ID or a role code may not use are
diagnostics as you type. It leaves formatting and code actions in code
files to that file's own server.

### `kiln confluence comments [EXPORT…] [--format text|json]`

The unresolved footer comments on each Confluence target's pages (all
targets, or the named ones), each under the item it is about when Kiln can
tell (CONFLUENCE.md, "Comments"), with a link that opens it:

```
$ kiln confluence comments
spec → space FLINT: 2 unresolved comments
  REQ-014   Alice Martin, 2026-10-12: Startup in 50 ms Is that measured cold?
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=456#comment-456
  (page)    Bob Chen, 2026-10-13: Can we get a PDF of this?
            on "Flint — design document"
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=457#comment-457
```

It needs the same token as publishing, reads only, and exits 3 on a
server or network error.

## The window

### `kiln gui [--since REV] [--item ID]`, or `kiln` alone

Opens the GUI ([GUI.md](GUI.md)): Tree, Trace, Impact, Board, Roadmap,
Decisions, Changes, Item, Check, Exports and Comments, with Vim keys and
the `SPC` leader; `Enter` opens the item under the cursor in Fenix at its
title. It also does what `set`, `link`, `unlink`, `new` and `renumber` do,
on the item under the cursor and through the same code, and what `check`,
`show`, `export` (writing file targets, and publishing after a `y`) and
`confluence comments` show; never an item's body. `--since REV` is the
baseline for the Changes page (default: the most recent tag; `b` picks
another); `--item ID` puts the cursor on that item. The window follows
edits to the files within two seconds.

On Windows, `kilnw` (next to `kiln.exe`) does the same without a console
window, for shortcuts and Flint; a failure to open goes to `kilnw.log` in
the temporary folder. Without a display, `kiln gui` fails with one line
and exit 2.

## In Flint

### `kiln provider [--name NAME] [--prefix KEY]`

A Flint provider on stdin and stdout (INTEGRATION.md, "Kiln as a Flint
provider"). Declare it in Flint's `settings.toml`:

```toml
[[providers]]
name = "kiln"
command = ["kiln", "provider"]
prefix = "k"
```

Every item that isn't closed is in Flint as `REQ-014 Title`, under
`requirement · approved · under REQ-002`; `Enter` opens it in Fenix at its
title. The other actions copy the ID or `[[REQ-014]]`, set the status, show
what depends on it, open it in Confluence or Jira, and clock in on its
agenda task in Fenix. The drill-downs: `k status REQ-014`, `k impact
REQ-014`, `k all <text>` (closed items too).

The project is `--project DIR` when given; otherwise the one Fenix shows
when Flint is summoned (remembered for next time), or the folder the
provider started in. `--name` must match the `[[providers]]` name and
`--prefix` the scope keyword when either isn't the default. Changes on
disk reach Flint within about 2 s. Messages go to stderr, which Flint
keeps in `logs/providers/<name>.log`.

### `kiln auth confluence [--forget]`

Asks for the token for `[confluence] url` without echoing it, checks it
against the server and stores it in the OS credential store (Windows
Credential Manager; the Secret Service on Linux) as service
`fenix-suite`, account `confluence:<url>`, where Flint finds it too.
`--forget` removes it. On Cloud, set `[confluence] user` (your account
e-mail) first. `KILN_CONFLUENCE_TOKEN`, when set, is used instead of the
stored token; CI sets that rather than running `kiln auth`.

```
$ kiln auth confluence
Token for https://confluence.example.com:
stored the token for https://confluence.example.com (signed in as Thomas) in the credential store as fenix-suite/confluence:https://confluence.example.com
```

### `kiln auth jira [--forget]`

The same for `[jira] url`: asks for a personal access token, checks it
with one search and stores it as `fenix-suite`/`jira:<url>`, the name
Flint uses, so one stored token serves both. `KILN_JIRA_TOKEN`, when set,
is used instead. Jira Server and Data Center only: a Jira Cloud URL is
refused with a line naming INTEGRATION.md's G6.

```
$ kiln auth jira
Personal access token for https://jira.example.com:
stored the token for https://jira.example.com in the credential store as fenix-suite/jira:https://jira.example.com
```

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

`kiln` with no arguments opens the GUI, as `kiln gui` does.

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
| 3 | network or server error (Confluence, or Jira for `list --jira` and `auth jira`) |
