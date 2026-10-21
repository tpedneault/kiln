This page lists every `kiln` command and every option, with a real example for each. Use it when you know roughly what you want to do and need the exact spelling, or when you script Kiln and need its exit codes and JSON output. If you are new to Kiln, start with [Tutorial: your first project](Tutorial) and come back here.

All examples on this page come from one small scratch project, made with `kiln init --name Demo` in an empty git repository. Its items are a few requirements, a test, two tasks, a milestone, a decision and a risk. An *item* is one Markdown file with an ID such as `REQ-001` ([Concepts](Concepts) explains items, types, links and coverage).

## Overview

| Command | Does |
|---|---|
| [`kiln init`](#kiln-init) | starts a project: writes `kiln.toml` and a `design/` folder |
| [`kiln preset list`](#kiln-preset-list) | the document templates Kiln ships (arc42, srs, design-doc) |
| [`kiln preset add`](#kiln-preset-add) | copies one into the project, with its types and an export |
| [`kiln new`](#kiln-new) | creates an item with the next free ID |
| [`kiln list`](#kiln-list) | lists items, filtered |
| [`kiln show`](#kiln-show) | one item with its links in both directions, coverage and progress |
| [`kiln trace`](#kiln-trace) | coverage of every item of a type, and the gaps |
| [`kiln impact`](#kiln-impact) | everything that depends on an item, transitively |
| [`kiln diff`](#kiln-diff) | items added, changed and removed between two revisions |
| [`kiln check`](#kiln-check) | checks the project against the rules |
| [`kiln export`](#kiln-export) | writes file exports and publishes Confluence exports |
| [`kiln set`](#kiln-set) | changes front-matter values |
| [`kiln link`](#kiln-link-and-kiln-unlink) | adds a link |
| [`kiln unlink`](#kiln-link-and-kiln-unlink) | removes a link |
| [`kiln renumber`](#kiln-renumber) | moves an item to another ID and rewrites every reference |
| [`kiln gui`](#kiln-gui) | opens the window (also `kiln` alone) |
| [`kiln lsp`](#kiln-lsp) | language server for editors |
| [`kiln provider`](#kiln-provider) | provider for the Flint launcher |
| [`kiln confluence comments`](#kiln-confluence-comments) | unresolved reader comments on published pages |
| [`kiln auth confluence`](#kiln-auth-confluence) | stores a Confluence token in the OS credential store |
| [`kiln auth jira`](#kiln-auth-jira) | stores a Jira token in the OS credential store |
| [`kiln jira sync`](#kiln-jira-sync) | keeps the project's Jira sub-tasks in step with its tasks |
| [`kiln jira pull`](#kiln-jira-pull) | takes tickets' statuses into the tasks |

## Global options

| Option | Does |
|---|---|
| `--project <DIR>` | starts the search for `kiln.toml` from `DIR` instead of the current folder. Accepted by every command, before or after the command name |
| `-h`, `--help` | prints usage. `kiln <command> --help` prints the usage of one command |
| `-V`, `--version` | prints `kiln <version>` |

```console
$ kiln --version
kiln 0.1.0
$ kiln --help
Requirements, design decisions and project progress, kept as Markdown in the repository they describe

Usage: kiln [OPTIONS] [COMMAND]

Commands:
  init        Write a kiln.toml with the default types and a design/ folder; never overwrites
  new         Create an item with the next free ID and print its path
  …
```

`kiln` with no command opens the GUI, as `kiln gui` does.

## Starting

### kiln init

```text
kiln init [--name NAME] [--fenix] [--preset NAME] [--project DIR]
```

Writes `kiln.toml` with the seven default types (requirement, design, decision, milestone, task, test, risk) and six default link roles, plus a `design/` folder with a README. It never overwrites: if a `kiln.toml` is already there, it stops with exit 2.

| Option | Does |
|---|---|
| `--name NAME` | the project's name in reports and exports. Default: the folder's name |
| `--fenix` | also registers `kiln lsp` in `.fenix/tools.json` as an additional language server for the Fenix editor, beside each language's own, for Markdown and the languages of the code `[code] paths` scans. It merges into an existing file. In a folder that already has a `kiln.toml`, `kiln init --fenix` does only this; run it again after adding `[code] paths` |
| `--preset NAME` | also adds a preset, exactly as [`kiln preset add NAME`](#kiln-preset-add) does. An unknown name stops before anything is written |
| `--project DIR` | writes the project in `DIR` instead of the current folder |

```console
$ kiln init --name Demo
kiln.toml
design/
kiln: started project "Demo"; `kiln new requirement --title "…"` adds the first item
$ kiln init
kiln: /home/me/demo/kiln.toml already exists; kiln init never overwrites
$ echo $?
2
$ kiln init --fenix
.fenix/tools.json
kiln: Fenix now runs `kiln lsp` beside each language's own server for markdown in this project (`:lsp-restart` to pick it up)
```

Notes:

- The files written go to stdout, one per line; the message goes to stderr.
- Source code is not scanned for code references until you add a `[code]` section to `kiln.toml`. The generated file has one commented out. See [Configuration](Configuration).
- `--fenix` keeps everything else in `.fenix/tools.json`, including another Markdown server, which stays the primary one. Kiln's older entry (Kiln as the Markdown server) is moved to the additional form. See [Editors and Flint](Editors-and-Flint#setting-it-up-in-fenix).

### kiln preset list

```text
kiln preset list
```

The presets: document templates Kiln ships, each with the item types it
places and an export target. See
[Templates and exports](Templates-and-exports#presets-arc42-and-others).

```console
$ kiln preset list
arc42       arc42 architecture documentation: twelve sections, one page each
srs         a software requirements specification after ISO/IEC/IEEE 29148, one document
design-doc  a design document: requirements, design, decisions, milestones, risks
```

### kiln preset add

```text
kiln preset add NAME [--dry-run] [--project DIR]
```

Copies a preset into the project: its template to
`design/templates/NAME.md`, the types and link roles it needs that
`kiln.toml` doesn't have yet (with item templates for the types it adds),
and an `[[export]]` target named `NAME` at the end of `kiln.toml`, with a
Confluence version of it commented out. The rest of `kiln.toml`, comments
included, is left as it was. From then on the template is your file:
edit it, and `kiln export NAME` writes the document.

| Option | Does |
|---|---|
| `--dry-run` | prints what would change; writes nothing |

```console
$ kiln preset add arc42
kiln.toml
design/templates/arc42.md
design/templates/quality-scenario.md
design/templates/building-block.md
kiln: added the arc42 preset (types quality-goal, quality-scenario, constraint, block, term; kept kiln.toml's own decision, risk; roles addresses; mitigates gains from decision, block; export `arc42`); `kiln export arc42` writes it
$ kiln preset add arc42
kiln: kiln.toml already has an export named `arc42`; the arc42 preset is added already, or rename that export first
```

Notes:

- A type you already have is kept as you defined it; the preset's
  template doesn't depend on status names. A link role you already have
  gains the types the preset links from or to.
- It never overwrites, and when it refuses it changes nothing (exit 2): a
  file it would write already exists, the export name is taken, one of
  its types uses a prefix another type has, or `kiln check` would find an
  export or template error in the result.
- Two presets can sit side by side (`arc42` and `srs` share the
  `constraint` and `term` types).

### kiln new

```text
kiln new TYPE [--title TEXT] [--parent ID] [--open]
```

Creates an item of type `TYPE` with the next free ID and prints its path. The next free ID is one above the highest ID used for that type, closed items included, so an ID is never reused.

| Option | Does |
|---|---|
| `TYPE` | the type, as `kiln.toml` names it: `requirement`, `decision`, … |
| `--title TEXT` | the item's title. Default: `Untitled <type>` |
| `--parent ID` | sets `parent`. The parent must exist and be of a type `TYPE` allows as parent |
| `--open` | opens the new file at its title line in the Fenix editor; see the notes |

```console
$ kiln new requirement --title "Exports are deterministic"
design/requirements/REQ-001-exports-are-deterministic.md
$ kiln new requirement --title "Same bytes on Windows and Linux" --parent REQ-001
design/requirements/REQ-002-same-bytes-on-windows-and-linux.md
$ kiln new test --title "Export twice and compare"
design/tests/TEST-001-export-twice-and-compare.md
$ cat design/requirements/REQ-002-same-bytes-on-windows-and-linux.md
+++
id = "REQ-002"
type = "requirement"
title = "Same bytes on Windows and Linux"
status = "draft"
parent = "REQ-001"
+++
$ kiln new widget
kiln: `widget` isn't a type (requirement, design, decision, milestone, task, test, risk)
$ kiln new requirement --parent TEST-001
kiln: a requirement can't have a test as parent (allowed: requirement)
```

Notes:

- The file goes in `<first folder of paths>/<type plural>/`, named by the ID and up to 50 characters of the title. Retitling an item later does not rename its file; only the ID at the start of the name matters ([Item files](Item-files)).
- The front matter has `id`, `type`, `title`, the type's initial status and `parent`. The body is the type's `template` file when `kiln.toml` sets one.
- `--open` tries, in order: the running Fenix (through its control channel), starting `fenix <path>:4`, and the operating system's default editor. stderr says which one it used. The item is created even when nothing can open it:

  ```console
  $ kiln new decision --title "Try open" --open
  design/decisions/ADR-002-try-open.md
  kiln: created ADR-002 but couldn't open it: Fenix isn't installed and no default editor could be started: No such file or directory (os error 2)
  ```

- `FENIX_HOME` chooses which Fenix `--open` talks to (the one started with the same `FENIX_HOME`).

## Reading

### kiln list

```text
kiln list [--type TYPE] [--status STATUS] [--tag TAG] [--jira] [--format text|json]
```

Lists items in type order (the order of `[types]` in `kiln.toml`), then by ID. The filters combine. Closed items are included.

| Option | Does |
|---|---|
| `--type TYPE` | only this type. An unknown type is an error (exit 2) |
| `--status STATUS` | only this status. Exact match; there is no negation (`!draft` matches nothing) |
| `--tag TAG` | only items with this tag |
| `--jira` | adds each linked Jira ticket's status and assignee; see the notes |
| `--format text\|json` | output format. Default: `text` |

```console
$ kiln list
REQ-001   approved  Exports are deterministic
REQ-002   verified  Same bytes on Windows and Linux
REQ-003   approved  Exports finish within 3 s
ADR-001   proposed  Use TOML front matter
MS-001    planned   First release
TASK-001  doing     Sort every unordered list
TASK-002  done      Write the release notes
TEST-001  passing   Export twice and compare
RISK-001  open      Fonts differ between machines
$ kiln list --type requirement --status approved
REQ-001  approved  Exports are deterministic
REQ-003  approved  Exports finish within 3 s
$ kiln list --type bogus
kiln: `bogus` isn't a type
```

Notes:

- An unknown status or tag is not an error; it just matches nothing.
- `--jira` reads the Jira mirror: a read-only copy of each linked ticket's status and assignee, cached for ten minutes. It needs `[jira] url` in `kiln.toml` and a token (`KILN_JIRA_TOKEN`, or the one `kiln auth jira` stored). It works with Jira Server and Data Center only, and does not use `HTTPS_PROXY` yet. Items without `jira` keys cost no request. A ticket shows after the item, for example `[FLINT-88 In Review · Alice]`. A network error, or no readable token, exits 3. See [Jira](Jira).

### kiln show

```text
kiln show ID [--format text|json]
```

Shows one item with everything Kiln computes about it: parent and children, tags, Jira keys, owner, due date, outgoing links by role, incoming links under their inverse names ("verified by", including code locations), items that mention it, coverage for each coverage role, and progress for a milestone.

| Option | Does |
|---|---|
| `ID` | the item |
| `--format text\|json` | output format. Default: `text` |

```console
$ kiln show REQ-001
REQ-001 Exports are deterministic
requirement · approved · design/requirements/REQ-001-exports-are-deterministic.md:2
children        REQ-002 Same bytes on Windows and Linux
due             2026-11-30
mitigates       RISK-001 Fonts differ between machines
implemented by  src/export.rs:1
verified by     src/export.rs:5
coverage        implements: yes, verifies: yes
$ kiln show REQ-003
REQ-003 Exports finish within 3 s
requirement · approved · design/requirements/REQ-003-untitled-requirement.md:2
mentioned by  REQ-001 Exports are deterministic
coverage      implements: no, verifies: no
$ kiln show MS-001
MS-001 First release
milestone · planned · design/milestones/MS-001-first-release.md:2
due       2026-12-15
includes  TASK-001 Sort every unordered list, TASK-002 Write the release notes
progress  1 of 2 done
$ kiln show REQ-099
kiln: REQ-099 doesn't exist
```

Notes:

- `src/export.rs:1` is a *code reference*: a comment such as `// implements: REQ-001` in a scanned source file ([Concepts](Concepts)).
- A milestone's progress counts the items `part-of` it, and also the items of the milestones it `depends-on`, transitively.

### kiln trace

```text
kiln trace [--type TYPE] [--format text|json|csv]
```

Shows the coverage of every item of `TYPE` that isn't closed: for each coverage role (by default `implements` and `verifies`), the open items and code locations that link to it, and the gaps.

| Option | Does |
|---|---|
| `--type TYPE` | the type to trace. Default: `requirement` |
| `--format text\|json\|csv` | output format. Default: `text` |

```console
$ kiln trace
Requirements coverage at b5550a2aaaeb

REQ-001  Exports are deterministic  (approved)
         implemented by: src/export.rs:1
         verified by: src/export.rs:5

REQ-002  Same bytes on Windows and Linux  (verified)
         implemented by: TASK-001
         verified by: TEST-001

REQ-003  Exports finish within 3 s  (approved)
         implemented by: — gap
         verified by: — gap

3 requirements; covered: implements 2/3, verifies 2/3
$ kiln trace --format csv
id,title,status,implemented by,verified by,gaps
REQ-001,Exports are deterministic,approved,src/export.rs:1,src/export.rs:5,
REQ-002,Same bytes on Windows and Linux,verified,TASK-001,TEST-001,
REQ-003,Exports finish within 3 s,approved,,,implements; verifies
```

Notes:

- The text and JSON forms name the commit (`HEAD`) when the project is in a git repository.
- An item covered only because all its open children are covered says `(through its children)`.
- CSV has one row per item; lists inside a cell are joined with `; `.
- `kiln trace` always exits 0, even with gaps. To fail a build on gaps, raise K201 or K202 to `error` and run `kiln check` ([Checking](Checking)).

### kiln impact

```text
kiln impact ID [--format text|json]
```

Lists everything that reaches `ID` by a link or a parent edge, transitively, nearest first, each with the path that connects it. Use it before you change or drop an item, to see what else is affected.

| Option | Does |
|---|---|
| `ID` | the item |
| `--format text\|json` | output format. Default: `text` |

```console
$ kiln impact REQ-001
REQ-002          child of REQ-001 — REQ-002 Same bytes on Windows and Linux
src/export.rs:1  implements REQ-001
src/export.rs:5  verifies REQ-001
TASK-001         implements REQ-002, child of REQ-001 — TASK-001 Sort every unordered list
TEST-001         verifies REQ-002, child of REQ-001 — TEST-001 Export twice and compare
```

### kiln diff

```text
kiln diff REV[..REV] [--type TYPE] [--format text|json]
```

Lists the items added, changed and removed between two revisions, or between one revision and the working tree (uncommitted edits included) when only one is given. A revision is a tag, branch or commit. The project at a revision is read from git, not checked out. [Changes and baselines](Changes-and-baselines) explains what counts as a change.

| Option | Does |
|---|---|
| `REV` | from `REV` to the working tree |
| `REV..REV` | between two revisions |
| `--type TYPE` | only this type |
| `--format text\|json` | output format. Default: `text` |

```console
$ kiln diff v0.1
v0.1 (1374820) → the working tree
~ REQ-001 Exports are deterministic: status approved → implemented; body
+ REQ-010 Exports finish within 3 s (requirement)
+ TEST-002 Time a 2,000-item export (test)
- REQ-003 Exports finish within 3 s (requirement)
2 added, 1 changed, 1 removed
$ kiln diff v0.1..HEAD --type test
v0.1 (1374820) → HEAD (6b94f71)
+ TEST-002 Time a 2,000-item export (test)
1 added, 0 changed, 0 removed
$ kiln diff nope
kiln: nope: git failed: `nope` isn't a revision in this repository
```

Notes:

- `~` is changed, `+` added, `-` removed.
- An item that was renumbered (here `REQ-003` to `REQ-010`) shows as one removed and one added, because the ID is the item's identity.
- A revision git doesn't know is a usage error (exit 2). In CI, fetch tags and full history (see [Continuous integration](Continuous-integration#full-git-history)).

## Checking

### kiln check

```text
kiln check [--format text|json|gitlab|github]
```

Runs every check rule at the severities the project sets, prints the findings sorted by file, line and column, then a summary on stderr. Exits 1 when any finding is an error, 0 otherwise: warnings don't fail. [Checking](Checking) lists every rule (K001 to K207) and how to fix each.

| Option | Does |
|---|---|
| `--format text` | for people: `path:line:col: CODE severity: message`. The default |
| `--format json` | for tools: an array of findings |
| `--format gitlab` | a GitLab Code Quality report, so findings show on merge requests |
| `--format github` | GitHub Actions workflow commands, so findings annotate the pull request diff |

```console
$ kiln check
design/requirements/REQ-003-untitled-requirement.md:2:7: K201 warning: REQ-003 is approved but nothing verifies it
design/requirements/REQ-003-untitled-requirement.md:2:7: K202 warning: REQ-003 is approved but nothing implements it
kiln: 2 warnings
$ echo $?
0
$ kiln check --format github
::warning file=design/requirements/REQ-003-untitled-requirement.md,line=2,col=7,title=K201::REQ-003 is approved but nothing verifies it
::warning file=design/requirements/REQ-003-untitled-requirement.md,line=2,col=7,title=K202::REQ-003 is approved but nothing implements it
kiln: 2 warnings
```

With `[check] K201 = "error"` in `kiln.toml` and a reference to an item that doesn't exist:

```console
$ kiln check
design/requirements/REQ-003-untitled-requirement.md:2:7: K201 error: REQ-003 is approved but nothing verifies it
design/requirements/REQ-003-untitled-requirement.md:2:7: K202 warning: REQ-003 is approved but nothing implements it
design/tasks/TASK-001-sort-every-unordered-list.md:11:12: K102 error: reference REQ-099 doesn't exist
kiln: 2 errors, 1 warning
$ echo $?
1
```

A clean project:

```console
$ kiln check
kiln: no findings
```

Notes:

- A `kiln.toml` that Kiln can't read at all (K001, or a newer `format`) is reported as a finding in the chosen format, with exit 1, so CI shows it. Other commands stop with exit 2 in that case.
- A bare ID in prose with a declared prefix, such as `REQ-099`, is a reference, and K102 if it doesn't exist. Put foreign IDs that only look like Kiln IDs in `code spans`.
- K207 (off by default) compares items with their Jira tickets. When Jira can't be read, stderr says `kiln: K207 not checked: …` and the other rules decide the exit code.
- In the GitLab report, errors have severity `major` and warnings `minor`.

## Exporting

### kiln export

```text
kiln export [NAME…] [--dry-run | --check] [--require-clean] [--allow-shallow]
            [--snapshot FILE] [--save-snapshot FILE] [--yes] [--force] [--adopt]
            [--prune-removed DAYS] [--since REV] [--format text|json]
```

Renders every export target in `kiln.toml` (or the named ones). File targets (Markdown, HTML, Word) are written when their bytes change. Confluence targets are planned and then published. [Templates and exports](Templates-and-exports) explains targets, layouts and templates; [Publishing to Confluence](Publishing-to-Confluence) explains the Confluence side.

| Option | Does |
|---|---|
| `NAME…` | the targets to export. Default: all. An unknown name is an error (exit 2) |
| `--dry-run` | renders everything and prints what would change; writes nothing |
| `--check` | like `--dry-run`, and exits 1 if anything would change. For committed exports in CI |
| `--require-clean` | refuses (exit 2) when the working tree has uncommitted changes |
| `--allow-shallow` | exports from a shallow clone; files older than its boundary link at the exported commit |
| `--snapshot FILE` | Confluence: plans against a recorded snapshot instead of the live space. Only with `--dry-run` or `--check` |
| `--save-snapshot FILE` | Confluence: writes the snapshot a live dry run read |
| `--yes` | Confluence: publishes without asking. Needed when not on a terminal, as in CI |
| `--force` | Confluence: overwrites pages edited in Confluence (after their edit is in the repository) |
| `--adopt` | Confluence: publishes into a `page =` that already has content, replacing it |
| `--prune-removed DAYS` | Confluence: sends pages retired more than `DAYS` days ago to the space's trash |
| `--since REV` | marks each item new or changed since `REV` and fills the `changes` report |
| `--format text\|json` | output format. Default: `text`. Publishing with JSON output needs `--yes` |

With one Markdown target, `design-md`, writing `docs/DESIGN.md`:

```console
$ kiln export --dry-run
design-md (markdown, document) → docs/DESIGN.md
  would create docs/DESIGN.md
$ kiln export --check
design-md (markdown, document) → docs/DESIGN.md
  would create docs/DESIGN.md
kiln: exports are out of date; run `kiln export` and commit the result
$ echo $?
1
$ kiln export
design-md (markdown, document) → docs/DESIGN.md
  create docs/DESIGN.md
$ kiln export --check
design-md (markdown, document) → docs/DESIGN.md
  unchanged (1 file)
$ kiln export --require-clean
kiln: the working tree has uncommitted changes (--require-clean)
$ kiln export nope
kiln: no export target `nope` (design-md)
```

In a shallow clone, with a target whose output is not committed (here an HTML file under an ignored `target/` folder):

```console
$ kiln export --dry-run
kiln: the clone is shallow: fetch full history (GitLab: GIT_DEPTH: 0; GitHub: fetch-depth: 0), or pass --allow-shallow
```

A Confluence target prints each page's operation and why, then publishes (example from the Kiln documentation, not from the scratch project):

```console
$ kiln export --yes
spec (confluence, document) → space FLINT, page 123456789 "Flint"
  update    "Flint — design document": REQ-014 changed; REQ-031 new (42 KiB)
  0 create, 1 update, 0 unchanged, 0 drift, 0 retire
spec: published 0 created, 1 updated, 0 retired, 0 pruned
```

Notes:

- File targets write atomically and only what changed, and remove the files they wrote earlier and no longer produce.
- `--dry-run` lists `leaves out:` open items no part of the document placed. An omission should be a decision.
- A target `kiln check` calls broken (K005, K112) doesn't export.
- Confluence reads the space with `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER` on Cloud), or the token `kiln auth confluence` stored.
- Without `--yes`, a run on a terminal asks before publishing each Confluence target. A run that isn't on a terminal stops after the plan and exits 1.
- A refused plan (a page edited in Confluence, a page too large, an adopted page with content) publishes nothing of that target and exits 1. A server or network error exits 3; the next run finishes what the failed one started.
- `kiln export` with no names also runs Confluence targets. To check only a committed file in CI, name it: `kiln export --check design-md`.
- Several Confluence endpoints are not yet confirmed against live Cloud and Data Center servers; they are tested against a fake server. See [Publishing to Confluence](Publishing-to-Confluence).

## Editing

The editing commands change only what they are about. They keep comments, layout and the file's line endings, write atomically, and refuse a file that changed on disk since Kiln read it (exit 1; run the command again). They refuse to act on a duplicated ID: renumber one copy first.

### kiln set

```text
kiln set ID KEY=VALUE…
```

Sets `status`, `title`, `parent`, `owner` or `due` (a date, `YYYY-MM-DD`). An empty value removes `parent`, `owner` or `due`. Every value is checked before anything is written: a status of the item's type, an existing parent of an allowed type that doesn't make a cycle, a real date.

```console
$ kiln set REQ-001 status=approved owner=thomas due=2026-11-30
REQ-001: status draft → approved
REQ-001: owner (none) → thomas
REQ-001: due (none) → 2026-11-30
$ kiln set REQ-003 title="Exports finish within 3 s"
REQ-003: title Untitled requirement → Exports finish within 3 s
$ kiln set REQ-001 owner=
REQ-001: owner thomas → (none)
$ kiln set REQ-001 status=done
kiln: `done` isn't a requirement status (draft, proposed, approved, implemented, verified, obsolete)
$ kiln set REQ-001 due=2026-02-30
kiln: `2026-02-30` isn't a date (2026-11-30)
$ kiln set REQ-001 color=red
kiln: kiln set changes status, title, parent, owner or due, not `color`
```

Notes: tags, Jira keys and links are not set with `kiln set`. Edit tags and `jira` in the file; use `kiln link` for links.

### kiln link and kiln unlink

```text
kiln link ID ROLE TARGET
kiln unlink ID ROLE TARGET
```

`link` adds `TARGET` to `[links] ROLE` of `ID`, creating the `[links]` table and the role as needed. `unlink` removes it, dropping the role, and `[links]`, when they become empty. `link` checks that the role exists, allows `ID`'s type as its source and `TARGET`'s type as its target. Links are written on their source side only: a test `verifies` a requirement, never the reverse.

```console
$ kiln link TEST-001 verifies REQ-002
TEST-001 verifies REQ-002
$ kiln link TEST-001 verifies REQ-002
TEST-001 verifies REQ-002 (already there)
$ kiln link TASK-001 part-of MS-001
TASK-001 part-of MS-001
$ kiln link REQ-001 verified-by TEST-001
kiln: `verified-by` isn't a link role; write it from the other side: kiln link TEST-001 verifies REQ-001
$ kiln link REQ-001 verifies REQ-002
kiln: a requirement can't be the source of `verifies` (from: test, code)
$ kiln unlink REQ-001 mitigates RISK-001
REQ-001 no longer mitigates RISK-001
$ cat design/tests/TEST-001-export-twice-and-compare.md
+++
id = "TEST-001"
type = "test"
title = "Export twice and compare"
status = "draft"

[links]
verifies = ["REQ-002"]
+++
```

Linking twice is not an error (exit 0).

### kiln renumber

```text
kiln renumber ID --to NEW [--file PATH]
```

Moves an item to another ID with the same prefix and rewrites every reference Kiln counts: `parent` and `[links]` in front matter, mentions in bodies, code references. Renames the file when its name starts with the old ID. Every file is checked unchanged before any is written.

| Option | Does |
|---|---|
| `ID` | the item to move |
| `--to NEW` | the new ID. It must have the same prefix and be free |
| `--file PATH` | when `ID` is duplicated, which file moves |

```console
$ kiln renumber REQ-003 --to REQ-010
REQ-003 → REQ-010: 1 reference in 1 other file; design/requirements/REQ-003-untitled-requirement.md → design/requirements/REQ-010-untitled-requirement.md
$ kiln renumber REQ-010 --to TASK-010
kiln: TASK-010 has another prefix than REQ-010; an item keeps its type's prefix
$ kiln renumber REQ-010 --to REQ-001
kiln: REQ-001 already exists (design/requirements/REQ-001-exports-are-deterministic.md)
```

When two branches allocated the same number, `kiln check` reports K101 on both copies. `--file` picks the copy to move. Only its own ID and file name change; the references, which may mean either copy, are listed on stderr for you to check by hand:

```console
$ kiln renumber REQ-015 --to REQ-016 --file design/requirements/REQ-015-other.md
REQ-015 → REQ-016: 0 references in 0 other files; design/requirements/REQ-015-other.md → design/requirements/REQ-016-other.md
kiln: left alone: design/tests/TEST-004-x.md:7 (may mean either REQ-015)
```

(This last example is from the Kiln documentation.)

## The window, editors and Flint

### kiln gui

```text
kiln gui [--since REV] [--item ID]
kiln
```

Opens the GUI: eleven pages (Tree, Trace, Impact, Board, Roadmap, Decisions, Changes, Item, Check, Exports, Comments), driven with Vim keys. It also sets statuses and other front-matter values, links and unlinks, creates and renumbers items, writes exports and publishes to Confluence after a confirm, through the same code as the commands. [The GUI](The-GUI) describes every page and key.

| Option | Does |
|---|---|
| `--since REV` | the baseline for the Changes page. Default: the most recent tag |
| `--item ID` | puts the cursor on this item |

```console
$ kiln gui
kiln: gui: couldn't connect to the display: os error at …: neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set.
$ echo $?
2
```

Notes: without a display, `kiln gui` fails with one line and exit 2. On Windows, `kilnw` does the same without a console window.

### kiln lsp

```text
kiln lsp
```

A language server on stdin and stdout, for Fenix or any editor that speaks the Language Server Protocol (LSP). It serves item files, and code files when your editor runs it beside their own server. It finds the project from the editor's workspace folder, or from the first file opened. It never writes a file. You don't run it yourself; your editor starts it. [Editors and Flint](Editors-and-Flint) covers what it offers and how to set it up.

### kiln provider

```text
kiln provider [--name NAME] [--prefix KEY]
```

A provider for the Flint launcher on stdin and stdout. Flint starts it. [Editors and Flint](Editors-and-Flint#the-flint-provider) covers setup, actions and drill-downs.

| Option | Does |
|---|---|
| `--name NAME` | must match the `name` of the `[[providers]]` entry in Flint's `settings.toml`. Default: `kiln` |
| `--prefix KEY` | must match the entry's scope keyword. Default: `k` |
| `--project DIR` | pins one project instead of following Fenix |

## Confluence and credentials

### kiln confluence comments

```text
kiln confluence comments [NAME…] [--format text|json]
```

Lists the unresolved footer comments on each Confluence target's pages (all targets, or the named ones). Each comment sits under the item it is about when Kiln can tell, with a link that opens it. It reads only, needs the same token as publishing, and exits 3 on a server or network error.

| Option | Does |
|---|---|
| `NAME…` | Confluence targets. Default: all |
| `--format text\|json` | output format. Default: `text` |

```console
$ kiln confluence comments
spec → space FLINT: 2 unresolved comments
  REQ-014   Alice Martin, 2026-10-12: Startup in 50 ms Is that measured cold?
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=456#comment-456
  (page)    Bob Chen, 2026-10-13: Can we get a PDF of this?
            on "Flint — design document"
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=457#comment-457
```

(Example from the Kiln documentation.) A target not published yet prints `not published yet`. Without a `[confluence]` section:

```console
$ kiln confluence comments
kiln: kiln.toml has no [confluence]
```

The comment endpoints are among those not yet confirmed against live servers.

### kiln auth confluence

```text
kiln auth confluence [--forget]
```

Asks for the token for `[confluence] url` without echoing it, checks it against the server, and stores it in the OS credential store (Windows Credential Manager; the Secret Service on Linux) as service `fenix-suite`, account `confluence:<url>`. Flint finds it there too. On Cloud, set `[confluence] user` (your account e-mail) first.

| Option | Does |
|---|---|
| `--forget` | removes the stored token instead |

```console
$ kiln auth confluence
Token for https://confluence.example.com:
stored the token for https://confluence.example.com (signed in as Thomas) in the credential store as fenix-suite/confluence:https://confluence.example.com
```

Notes: `KILN_CONFLUENCE_TOKEN`, when set, is used instead of the stored token. CI sets that variable rather than running `kiln auth`. On a headless Linux machine without a Secret Service, the store can't be read and the error says so.

### kiln jira sync

```text
kiln jira sync [--dry-run] [--yes]
```

Creates, updates and moves the Jira sub-tasks of the project issue to
match the synced items (`[jira] project`; see [Jira](Jira#keeping-jira-in-step-kiln-jira-sync)), after printing the plan. Without `--yes` it asks on a terminal; not on a terminal, it stops after the plan with exit 1. The keys it creates are written into `kiln.toml` (`parent`) and the items (`jira`): commit them.

| Option | Does |
|---|---|
| `--dry-run` | prints the plan; changes nothing in Jira or the files |
| `--yes` | syncs without asking |

Exit codes: 0 synced (or nothing to do), 1 a ticket's move was refused by the workflow, or the sync wasn't confirmed, 2 no `[jira] project` or no token, 3 Jira couldn't be reached (the keys created before the error are still written).

### kiln jira pull

```text
kiln jira pull [ID…]
```

Takes the status of each item's ticket (a sub-task of the project issue) into the item, mapped back through `[jira.status]` or the status category, for tickets someone moved in Jira. Without IDs, every synced item. Prints each change as `kiln set` does.

### kiln auth jira

```text
kiln auth jira [--forget]
```

The same for `[jira] url`: asks for a personal access token, checks it with one search, and stores it as `fenix-suite` / `jira:<url>`, the name Flint uses, so one stored token serves both. `KILN_JIRA_TOKEN`, when set, is used instead. Jira Server and Data Center only:

```console
$ kiln auth jira
kiln: https://acme.atlassian.net is Jira Cloud, which fenix-jira can't sign in to yet (INTEGRATION.md, G6); the mirror works with Jira Server and Data Center
```

`G6` in that message is the recorded gap "Jira Cloud support in the Jira client Kiln uses". See [Jira](Jira).

## Project discovery

Every command except `kiln init` needs a project. Kiln finds it by walking up from the current folder to the nearest `kiln.toml`. `--project DIR` starts that walk from `DIR` instead, so `--project design` from the project root still finds the root.

```console
$ cd /tmp
$ kiln check
kiln: no kiln.toml in /tmp or any folder above it (run `kiln init` to start a project)
$ kiln --project ~/demo/design list --type test
TEST-001  passing  Export twice and compare
```

Paths Kiln prints are relative to the project root.

`kiln lsp` finds the project from the editor's workspace folder or the first file opened. `kiln provider` follows the project the Fenix editor shows (see [Editors and Flint](Editors-and-Flint#which-project-it-follows)).

## stdout and stderr

Results go to stdout; messages, summaries and errors go to stderr. So you can pipe or redirect results safely:

```console
$ kiln list --format json > items.json
$ kiln check --format gitlab > gl-code-quality.json
kiln: 2 warnings
```

An error is one line on stderr, starting `kiln: `, naming what failed and why, with the file and line when there is one. `kiln lsp` and `kiln provider` use stdout for their protocol; the provider writes its messages to stderr, which Flint keeps in a log.

## Exit codes

| Code | Means |
|---|---|
| 0 | fine. Warnings from `kiln check` don't change it |
| 1 | `kiln check` found an error; `kiln export --check` found changes; a Confluence plan was refused, or a publish was needed but not confirmed (not on a terminal and no `--yes`); an edit was refused because a file changed on disk |
| 2 | usage or configuration error: unknown command, type, status or ID; no `kiln.toml`; a `kiln.toml` Kiln can't read (except for `kiln check`, which reports it as a finding with exit 1); a value the configuration doesn't allow; a git revision that doesn't exist; a shallow clone without `--allow-shallow`; `--require-clean` with uncommitted changes; no display for `kiln gui` |
| 3 | network or server error: Confluence, or Jira for `list --jira` and `auth jira` (including a Jira token that can't be read) |

## JSON output

Commands with `--format json` print pretty-printed JSON on stdout. Keys are sorted alphabetically.

### kiln list

An array, one object per item:

| Field | Type | Meaning |
|---|---|---|
| `id` | string | `REQ-001` |
| `type` | string | `requirement` |
| `title` | string | the title |
| `status` | string | the status |
| `parent` | string or null | the parent's ID |
| `tags` | array of strings | tags |
| `path` | string | the file, relative to the project |
| `line` | number | the line of `id` in the file |
| `jira` | array | only with `--jira`: `{key, summary, status, resolved, assignee}` per ticket, or `{key, missing: true}` for a key Jira doesn't know |

```json
[
  {
    "id": "REQ-001",
    "line": 2,
    "parent": null,
    "path": "design/requirements/REQ-001-exports-are-deterministic.md",
    "status": "approved",
    "tags": [],
    "title": "Exports are deterministic",
    "type": "requirement"
  }
]
```

### kiln show

One object. `id`, `type`, `title`, `status`, `path` and `line` are always there. Every other row of the text output becomes a key named like its label, holding an array of strings: `parent`, `children`, `tags`, `due`, outgoing roles (`mitigates`), incoming roles under their inverse names with spaces (`"implemented by"`, `"verified by"`, `includes`), `"mentioned by"`, `coverage`, `progress`. Keys appear only when they have a value.

```json
{
  "children": ["REQ-002 Same bytes on Windows and Linux"],
  "coverage": ["implements: yes", "verifies: yes"],
  "due": ["2026-11-30"],
  "id": "REQ-001",
  "implemented by": ["src/export.rs:1"],
  "line": 2,
  "mitigates": ["RISK-001 Fonts differ between machines"],
  "path": "design/requirements/REQ-001-exports-are-deterministic.md",
  "status": "approved",
  "title": "Exports are deterministic",
  "type": "requirement",
  "verified by": ["src/export.rs:5"]
}
```

### kiln trace

| Field | Meaning |
|---|---|
| `type` | the traced type |
| `commit` | the full `HEAD` commit, when in a git repository |
| `roles` | the coverage roles, in order |
| `items` | per item: `id`, `title`, `status`, `coverage` (an object from role to the IDs and code locations covering it), `gaps` (the roles with no coverage) |

```json
{
  "commit": "b5550a2aaaeb26aff88a6a694bde9373da9ad8f6",
  "items": [
    {
      "coverage": { "implements": [], "verifies": [] },
      "gaps": ["implements", "verifies"],
      "id": "REQ-003",
      "status": "approved",
      "title": "Exports finish within 3 s"
    }
  ],
  "roles": ["implements", "verifies"],
  "type": "requirement"
}
```

### kiln impact

An array, one object per thing that reaches the item: `source` (an ID or a code location `path:line`) and `path`, the chain of edges from it to the item, each `{from, relation, to}` where `relation` is a link role or `parent`.

```json
[
  {
    "path": [
      { "from": "TASK-001", "relation": "implements", "to": "REQ-002" },
      { "from": "REQ-002", "relation": "parent", "to": "REQ-001" }
    ],
    "source": "TASK-001"
  }
]
```

### kiln diff

| Field | Meaning |
|---|---|
| `from` | the first revision as given |
| `fromCommit` | its full commit |
| `to` | the second revision as given, or `working tree` |
| `items` | per item: `id`, `type`, `title`, `path`, `change` (`added`, `changed`, `removed`), `details` (what changed, as in the text form, for example `status approved → implemented`, `body`) |

### kiln check

An array of findings: `code` (`K201`), `severity` (`error` or `warning`), `path`, `line`, `column`, `message`.

```json
[
  {
    "code": "K201",
    "column": 7,
    "line": 2,
    "message": "REQ-003 is approved but nothing verifies it",
    "path": "design/requirements/REQ-003-untitled-requirement.md",
    "severity": "warning"
  }
]
```

`--format gitlab` instead prints GitLab Code Quality objects: `check_name`, `description`, `fingerprint`, `location` (`path`, `lines.begin`) and `severity` (`major` for errors, `minor` for warnings).

### kiln export

An array, one object per target.

A file target: `name`, `format`, `layout`, `write` (files that would be or were written, each `{path, new}`), `unchanged` (paths), `remove` (paths of files it wrote earlier and no longer produces), `leftOut` (open items the document doesn't place).

```json
[
  {
    "format": "markdown",
    "layout": "document",
    "leftOut": [],
    "name": "design-md",
    "remove": [],
    "unchanged": ["docs/DESIGN.md"],
    "write": []
  }
]
```

A Confluence target: `name`, `format` (`confluence`), `layout`, `space`, `parent`, `leftOut`, `plan` and `applied`.

- `plan` has `ops`, `foreign`, `warnings` and `refusals`. Each op has `kind` (`create`, `update`, `unchanged`, `drift`, `retire`, `prune`), `key`, `title`, `page_id` (when the page exists), `reasons`, `size` and, when there is one, `message`.
- `applied` is null for a dry run; after a publish it has `created`, `updated`, `retired`, `pruned` and `skipped`.

### kiln confluence comments

An array, one object per published target: `name`, `space`, `comments`. Each comment has `item` (the ID it is about, or null for the page), `page` (page id), `page_title`, `author`, `created`, `text` (one line of plain text) and `url`.

See also: [Concepts](Concepts) · [Checking](Checking) · [Templates and exports](Templates-and-exports) · [Troubleshooting](Troubleshooting)
