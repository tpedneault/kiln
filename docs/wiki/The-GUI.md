This page describes Kiln's window: how to start it, its seven pages and what each is good for, every key and mouse action, and how to change its looks. Read it when you want to look around a project (the whole tree, the coverage gaps, what a change touches, where milestones stand, what changed since a release) rather than ask one question at a time with the command line.

The GUI is a reader. It never edits anything: you edit items in your editor, or with `kiln set` and `kiln link`, and the window follows the files within two seconds. The one thing it does outward is open an item in the Fenix editor.

## Starting it

| Command | Opens |
|---|---|
| `kiln` | the window on the project around the current folder |
| `kiln gui` | the same |
| `kiln gui --since v1.0` | with `v1.0` as the baseline of the Changes page (a tag, branch or commit) |
| `kiln gui --item REQ-014` | with the cursor on `REQ-014` |
| `kiln gui --project ~/work/flint` | on another project (see [Command reference](Command-reference#project-discovery)) |

The Changes page compares the project with a baseline. Without `--since`, the baseline is the most recent git tag. With no tag and no `--since`, the Changes page says so and tells you how to set one.

**Windows.** `kiln.exe` is a console program, so starting it from a shortcut flashes a console window. Use `kilnw.exe`, installed next to `kiln.exe`: it takes the same arguments (`kilnw gui --item REQ-014`) and opens only the window. Shortcuts and Flint's "Open in Kiln" action use it. On Linux and macOS there is one `kiln`.

**No display.** Without a display (an SSH session, a CI container), `kiln gui` fails with one line and exit 2:

```console
$ kiln gui
kiln: gui: couldn't connect to the display: os error at …: neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set.
```

## The layout

The window is like Fenix's pages: full-window lists driven from the keyboard.

- The **top line** shows the seven pages, the current one marked, and the project's name: `1 [Tree]  2 Trace  3 Impact …`.
- The **body** is a list of rows. A row is one line: an item, a code location, or a heading that groups the rows under it. The cursor skips headings. Columns are aligned in a monospace font.
- The **bottom line** shows the active filter, a message, or the keys that work on this page, for example `14 rows  Enter open · i impact · / filter · SPC pages · q quit · a show closed`.

Every page keeps its own cursor and scroll position. When the project reloads, the cursor stays on the same item (by ID), or on the row where it was if that item went away.

## The pages

The row sketches below are text versions of what each page shows, taken from Kiln's own tests; spacing is simplified.

### Tree (`SPC t`, `1`)

Every open item, under a heading per type, children indented under their parent:

```text
Requirements
REQ-001  Fast start  approved
  REQ-002  Cold start  draft
REQ-004  Small binary  approved
Decisions
ADR-002  TOML  accepted
Milestones
MS-001  First release  active
MS-002  Later  planned
Tasks
TASK-001  Write the loader  done
TASK-002  Measure it  todo
```

Closed items (an `obsolete` requirement, a `superseded` decision) are hidden until you press `a`.

Use it to see the shape of the project and to find an item by browsing or filtering.

### Trace (`SPC x`, `2`)

Coverage: one row per open item of the traced type (default `requirement`), with one column per coverage role showing what covers it, or `gap` in the error colour:

```text
Requirement                               implements     verifies
REQ-001  Fast start                       src/lib.rs:1   through its children
REQ-002  Cold start                       gap            TEST-001
REQ-004  Small binary                     gap            gap
```

`m` keeps only items with a gap. `T` cycles to the next type that some coverage role reaches. "through its children" means the item is covered because all its open children are.

Use it before a review or a release, to find requirements nothing implements or verifies. It is the window's version of `kiln trace`.

### Impact (`SPC i`, `3`)

Everything that reaches one item by links or parents, transitively, nearest first, each with its path. Pick the item with `i` on any page:

```text
What depends on REQ-001 Fast start (3)
REQ-002  Cold start  draft  ← child of REQ-001
src/lib.rs:1  ← implements REQ-001
TEST-001  Timing  ready  ← verifies REQ-002, child of REQ-001
```

Code locations are included; `Enter` on one opens the file at that line.

Use it before you change or drop an item, to see what else needs a look. It is the window's version of `kiln impact`.

### Board (`SPC b`, `4`)

The work items grouped under a heading per status, in the type's status order, with counts:

```text
todo (1)
TASK-002  Measure it  todo
doing (0)
blocked (0)
done (1)
TASK-001  Write the loader  done
```

The work items are the items of type `task`. A project without a `task` type gets every type that has `done` statuses, except milestones. Closed statuses (`dropped`) are hidden until `a`. With several types on the board, the headings name the type too (`tasks · todo (3)`), and child items say `under TASK-001`.

Use it in a stand-up or a planning meeting to see what is in progress.

### Roadmap (`SPC r`, `5`)

Milestones in `due` order (undated last), each with status, due date and progress, and the items `part-of` it under it:

```text
Milestones
MS-001  First release  active  due 2026-11-30  1/2 █████░░░░░
  TASK-001  Write the loader  done
  TASK-002  Measure it  todo
MS-002  Later  planned  undated
```

A milestone's progress also counts the items of milestones it `depends-on`, transitively.

Use it to see whether a release date still holds.

### Decisions (`SPC d`, `6`)

The decision log in ID order: status, and what supersedes what:

```text
Decision log
ADR-001  YAML  superseded  superseded by ADR-002
ADR-002  TOML  accepted  supersedes ADR-001
```

Unlike Tree, this page shows superseded and rejected decisions: the history is the point.

Use it to answer "why did we do it this way?" and to check that a superseded decision names its successor.

### Changes (`SPC c`, `7`)

The items added, changed and removed since the baseline, grouped, each with what changed:

```text
Since v1.0 (1a2b3c4): 1 added, 1 changed, 0 removed
Added (1)
REQ-005  New one  (requirement)
Changed (1)
REQ-004  Small binary  status approved → implemented
```

Without a baseline the page shows:

```text
No baseline: start with `kiln gui --since REV`, or tag a release so the latest tag is used.
```

The baseline's project is read from git once, at start. When you edit files, the comparison is recomputed against the new working tree. [Changes and baselines](Changes-and-baselines) explains what counts as a change.

Use it to prepare release notes or a review of what changed since the last baseline. It is the window's version of `kiln diff`.

## Keys

Normal mode, as in Fenix's pages:

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑` | next / previous row |
| `C-d` `C-u`, `PgDn` `PgUp` | half a page down / up |
| `gg` `G`, `Home` `End` | first / last row |
| `Enter` | open the item (or code location) in Fenix at its title line |
| `i` | Impact of the item under the cursor |
| `/` | filter (see below) |
| `a` | Tree and Board: show closed items too, or hide them again |
| `m` | Trace: only items with a gap, or all again |
| `T` | Trace: the next traced type |
| `R` | reload now |
| `Tab`, `]` | next page |
| `[` | previous page |
| `1` to `7` | Tree, Trace, Impact, Board, Roadmap, Decisions, Changes |
| `SPC` | the leader key; see below |
| `q`, `C-q` | quit |
| `Esc` | clears a pending key, the leader or the filter |

`C-d` means Ctrl+D. Shift is not a separate modifier: `G` and `T` are the capital letters.

### The leader

`SPC` (the space bar) starts a leader sequence and shows a popup of the keys that follow, as Fenix does:

| Keys | Do |
|---|---|
| `SPC t` | Tree |
| `SPC x` | Trace |
| `SPC i` | Impact |
| `SPC b` | Board |
| `SPC r` | Roadmap |
| `SPC d` | Decisions |
| `SPC c` | Changes |
| `SPC R` | reload |
| `SPC q` | quit |

`Esc` cancels a leader sequence. An unknown key says `SPC: no such key` in the footer.

### Mouse

| Action | Does |
|---|---|
| wheel | scrolls |
| click | moves the cursor to that row |
| double click | opens the row, as `Enter` does |

## Opening items

`Enter` (or a double click) opens the item under the cursor in Fenix at its title line, or a code location at its line. It tries the running Fenix first, then starts Fenix, then, when Fenix isn't installed, the operating system's default editor for the file. On a heading, the footer says `Nothing to open here`. See [Editors and Flint](Editors-and-Flint#opening-items-in-fenix).

## Filtering

Press `/` and type. The page narrows to rows whose text contains what you typed, ignoring case. In Tree, the parents of a match stay, so you see where it sits. The footer shows the filter as you type.

- `Enter` keeps the filter and returns to normal keys; the footer keeps showing `/text`.
- `Esc` clears it.
- `Backspace` removes the last character.
- Switching page clears the filter.

## Closed items

An item is closed when its status is one of its type's `closed` statuses (by default `obsolete` requirements, `rejected` and `superseded` decisions, `dropped` tasks and milestones, `closed` risks). Tree and Board hide closed items until you press `a`; press it again to hide them. Trace leaves closed items out. Decisions shows them all.

## Trace options

| Key | Effect |
|---|---|
| `m` | toggles "gaps only": only items missing coverage for at least one role |
| `T` | traces the next type. The types offered are those that some coverage role points at; with the default configuration that is `requirement` only |

The coverage roles are `[coverage] roles` in `kiln.toml`, by default `implements` and `verifies` ([Configuration](Configuration)).

## Reloading and following edits

The window checks the item files and `kiln.toml` every two seconds and reloads when they change, so edits in your editor, `kiln set` in a terminal or a `git pull` appear on their own. `R` or `SPC R` reloads at once. The cursor stays on the same item across a reload.

## The Jira column

When `kiln.toml` has `[jira] url` and items have `jira` keys, every item row ends with its tickets once the Jira mirror has loaded:

```text
TASK-001  Write the loader  done  FLINT-1 Done · Alice
```

A resolved ticket (in Jira's Done category) is green, others muted. A key Jira doesn't know shows as `FLINT-12 ?`.

The mirror loads on a separate thread, from Kiln's cache when it is less than ten minutes old, so a slow or unreachable Jira never holds up the window. It is read once, at start; restart the window to refresh it. When it can't be read (no token, network error, Jira Cloud), the footer says why. The token is `KILN_JIRA_TOKEN` or the one `kiln auth jira` stored. Jira Server and Data Center only; see [Jira](Jira).

## Looks and settings.toml

The window uses the same rendering stack, themes and monospace font as Fenix and Flint, so the three look alike. To change its looks, create `settings.toml` in Kiln's settings folder:

| Platform | Settings folder |
|---|---|
| Windows | `%AppData%\kiln` |
| Linux | `~/.config/kiln` |
| any, with `KILN_HOME` set | `KILN_HOME` itself |

```toml
theme = "Nord"            # one of Fenix's themes; default Orbit Dark
font = "Cascadia Mono"    # a monospace family; default: the one Fenix would pick
font_size = 15            # points, 8 to 40; default 15
```

| Key | Values |
|---|---|
| `theme` | `Orbit Dark` (default), `TempleOS`, `Gruvbox Dark`, `Nord`, `Dracula`, `Solarized Dark`, `One Dark`, `Visual Studio Dark`, `Catppuccin Latte`, `Catppuccin Frappé`, `Catppuccin Macchiato`, `Catppuccin Mocha`. Case doesn't matter |
| `font` | the name of a monospace font family installed on the system |
| `font_size` | a number from 8 to 40 |

Every key is optional. An unknown key, an unknown theme or a size out of range is reported in the footer and otherwise ignored; the window still opens. This file holds your personal looks and is not part of the project, so it is not in `kiln.toml`.

## What it doesn't do

- Edit anything: use your editor, `kiln set`, `kiln link`, the language server or Flint's `k status`.
- Show an item's body: open it in your editor with `Enter`.
- Refresh the Jira mirror while open.
- Remember the last page between runs.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `couldn't connect to the display` | no graphical session. Run it on a desktop, or use the command line (`kiln trace`, `kiln impact`, `kiln diff`) |
| On Windows, nothing happens when you start `kilnw` | `kilnw` has no console to print to. It writes why the window couldn't open to `kilnw.log` in the temporary folder (`%TEMP%\kilnw.log`) |
| Changes page says "No baseline" | the repository has no tag. Tag a release, or start with `kiln gui --since <commit>` |
| `--since` fails | the revision doesn't exist in this clone. Fetch tags (`git fetch --tags`) |
| `Enter` doesn't open Fenix | Fenix isn't installed or not on `PATH`; the default editor opens instead. If several Fenix instances run, set `FENIX_HOME` to choose one |
| Footer says the theme isn't known | check the spelling against the list above |
| Jira tickets missing | no `[jira] url`, no token, Jira Cloud, or Jira unreachable; the footer says which. Try `kiln list --jira` in a terminal for the full error |

See also: [Command reference](Command-reference) · [Editors and Flint](Editors-and-Flint) · [Changes and baselines](Changes-and-baselines) · [Troubleshooting](Troubleshooting)
