This page describes Kiln's window: how to start it, its eleven pages and what each is good for, how to edit from it, every key and mouse action, and how to change its looks. Read it when you want to look around a project (the whole tree, the coverage gaps, what a change touches, where milestones stand, what changed since a release, what `kiln check` finds) and make the small changes that turns up, without typing one command at a time.

The GUI edits front matter, never prose. It does what `kiln set`, `kiln link`, `kiln unlink`, `kiln new` and `kiln renumber` do, through the same code, and it writes and publishes exports. It has no text area: an item's body is written in your editor, which `Enter` opens at the item. Whoever edits the files, the window follows them within two seconds.

## Starting it

| Command | Opens |
|---|---|
| `kiln` | the window on the project around the current folder; outside a project, [Home](#home-switching-projects), the projects dashboard |
| `kiln gui` | the same |
| `kiln gui --since v1.0` | with `v1.0` as the baseline of the Changes page (a tag, branch or commit) |
| `kiln gui --item REQ-014` | with the cursor on `REQ-014` |
| `kiln gui --project ~/work/flint` | on another project (see [Command reference](Command-reference#project-discovery)) |

The Changes page compares the project with a baseline. Without `--since`, the baseline is the most recent git tag. With no tag and no `--since`, the Changes page says so and tells you how to set one.

**Windows.** `kiln.exe` is a console program, so starting it from a shortcut flashes a console window. Use `kilnw.exe`, installed next to `kiln.exe`: it takes the same arguments (`kilnw gui --item REQ-014`) and opens only the window. Shortcuts and Flint's "Open in Kiln" action use it. A shortcut without `--project` opens on Home, where you pick the project. On Linux and macOS there is one `kiln`.

## Home: switching projects

Started outside a project, the window opens on **Home**, a dashboard modelled on Fenix's: Kiln's logo and the date, a find field, the projects you have opened before (most recent first), and a preview of the one under the cursor.

```text
projects 2 ─────────────────── a add   Kiln ─────────────────── Enter open
 1  Kiln                       2 h ●   ~/src/kiln
    ~/src/kiln
 2  Flint                      3 d ○   items     42 · 30 open
    ~/src/flint                        types     requirement 12 · decision 14
 +  add a project                  a   next      MS-003 Beta · due 2026-11-30 · 3/5
                                       check     2 errors · 1 warning
                                       opened    2 h ago
```

The dot after each project says how `kiln check` finds it: green when clean, the warning colour with warnings, red with errors or when its `kiln.toml` has gone.

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑` | move between projects; the last entry is *add a project* |
| `Enter`, double click | open the project |
| `1`–`9` | open one of the first nine directly |
| `/` | find a project by name or path; `Enter` opens the match, `Esc` clears |
| `a` | add a project: type its folder; `Tab` completes folder names, `~` is your home folder. A folder inside a project adds that project; one with no `kiln.toml` is refused (run `kiln init` there first) |
| `d` | take the project off the list, after `y`; its folder isn't touched |
| `R` | read the projects again |
| `q` | quit |

On a project's pages, `SPC h` (or `gh`) goes back to Home, with that project first; open another to switch. While an export or a publish is running, Home waits until it finishes.

Every project you open, from Home or with `kiln` in its folder, joins the list. The list is `projects.toml` in Kiln's state folder (see [Configuration](Configuration)); it belongs to this machine and is never written into a repository.

**No display.** Without a display (an SSH session, a CI container), `kiln gui` fails with one line and exit 2:

```console
$ kiln gui
kiln: gui: couldn't connect to the display: os error at …: neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set.
```

## The layout

The window is like Fenix's pages: full-window lists driven from the keyboard.

- The **top line** shows the pages, the current one marked, the project's name and, when `kiln check` finds errors, how many: `1 [Tree]  2 Trace  3 Impact …  Kiln  2 errors`.
- The **body** is a list of rows. A row is one line: an item, a code location, or a heading that groups the rows under it. The cursor skips headings. Columns are aligned in a monospace font.
- The **bottom line** shows a question being asked, the active filter, a message, what is running, or the keys that work on this page, for example `14 rows  Enter open · i impact · / filter · SPC pages · q quit · a show closed`.

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
No baseline: press b to pick one, start with `kiln gui --since REV`, or tag a release.
```

`b` picks another baseline: the tags are offered newest first, and you can type any revision (`main`, `HEAD~10`, a commit). The baseline's project is read from git once, when it is picked. When you edit files, the comparison is recomputed against the new working tree. [Changes and baselines](Changes-and-baselines) explains what counts as a change.

Use it to prepare release notes or a review of what changed since the last baseline. It is the window's version of `kiln diff`.

### Item (`SPC v`, `8`)

One item, as `kiln show` prints it, for the item you pressed `v` on (on any page):

```text
REQ-002 Cold start
REQ-002  Cold start  draft  requirement · design/REQ-002.md:2
  parent       REQ-001 Fast start
  verified by  TEST-001 Timing
  coverage     implements: no
               verifies: yes
```

Every related item and code location is a row: `Enter` opens it in Fenix, `v` views that item instead. `C-o` goes back to the item viewed before, `C-i` forward again, as in Vim's jump list.

Use it to follow the links of one item without retyping IDs. It is the window's version of `kiln show`.

### Check (`SPC k`, `9`)

`kiln check`'s findings, grouped by file, with line, column, rule and severity:

```text
kiln check: 1 error, 0 warnings
design/TEST-002.md
     7:13  K102 error    REQ-099 doesn't exist
```

The findings are recomputed every time the project reloads, so fixing one in your editor makes it disappear within two seconds. `Enter` opens the file at the finding. On a duplicate ID (K101), `SPC e r` renumbers that file's copy. K207 (Jira) joins in once the Jira mirror has loaded, when `[check]` turns it on.

Use it to clean up before you push, instead of reading `kiln check` in a terminal.

### Exports (`SPC p`)

Each export target's dry run, as `kiln export --dry-run` prints it:

```text
design-md (markdown, document) → docs/generated/DESIGN.md
  would update docs/generated/DESIGN.md
spec (confluence, document) → space FLINT, page 123456789 "Flint"
  update    "Flint — design document": REQ-014 changed; REQ-031 new (42 KiB)
  0 create, 1 update, 0 unchanged, 0 drift, 0 retire
```

The plans are made when you first open the page, on a separate thread (a Confluence target reads the space, which can take a few seconds); `R` makes them again. `w` writes the file target under the cursor, `W` every file target. `P` on a Confluence target publishes it after you answer `y`; see [Publishing from the window](#publishing-from-the-window).

Use it to check what an export would change, and to publish without leaving the window.

### Comments (`SPC m`)

The unresolved comments on each Confluence target's pages, as `kiln confluence comments` lists them:

```text
spec → space FLINT: 1 unresolved comment
REQ-001   Alice Martin, 2026-10-12: Is that measured cold?
```

`Enter` opens the comment in your browser; `v` shows the item it is about. Read once when the page is first opened; `R` reads them again.

## Editing

The keys below work on the item under the cursor, on every page (an Item row, a finding and a comment count too):

| Keys | Does | Same as |
|---|---|---|
| `s`, `SPC e s` | sets the status: pick one of its type's statuses | `kiln set ID status=…` |
| `H` / `L` (Board) | moves the item to the previous / next status | `kiln set ID status=…` |
| `SPC e t` | sets the title | `kiln set ID title=…` |
| `SPC e p` | sets the parent: pick an allowed item, or "(no parent)" | `kiln set ID parent=…` |
| `SPC e o`, `SPC e d` | sets the owner, the due date (`2026-11-30`); empty removes it | `kiln set ID owner=… due=…` |
| `l`, `SPC e l` | adds a link: pick the role, then the target | `kiln link ID ROLE TARGET` |
| `U`, `SPC e u` | removes a link: pick it, then press `y` | `kiln unlink ID ROLE TARGET` |
| `n`, `SPC e n` | creates an item: pick the type, type the title; it opens in Fenix | `kiln new TYPE --title … --open` |
| `N`, `SPC e N` | creates a child of the item | `kiln new TYPE --parent ID --open` |
| `SPC e r` | renumbers: type the new ID (the next free one is filled in), then press `y` | `kiln renumber ID --to …` |

The question appears on the bottom line. For a pick, the choices show above it and narrow as you type: `↓` `↑` (or `Tab`, `C-n`, `C-p`) move, `Enter` answers, `C-u` clears what you typed, `Esc` cancels. Picks only offer what the configuration allows: the statuses of the item's type, the link roles its type may start, the targets those roles accept and that aren't linked yet.

Before anything is written, the answer is checked exactly as the command checks it. A value that isn't allowed keeps the question open with the reason:

```text
Due date of REQ-002 (YYYY-MM-DD, empty for none): soon▏  `soon` isn't a date (2026-11-30)
```

Then the edit is made by the same code as the command: only the key or link it is about changes, comments, layout and line endings are kept, and a file that changed on disk since it was read (because your editor just saved it) is refused rather than overwritten. The bottom line shows what the command would have printed, such as `REQ-002: status draft → approved`, and the cursor follows the item.

There is no undo. The files are in git: `git diff` shows what changed, and `git checkout -- <file>` takes it back.

### Publishing from the window

`P` on a Confluence target asks first, naming the space and the page the document goes under:

```text
Publish spec (confluence, document) → space FLINT, page 123456789 "Flint"? y/n
```

`y` publishes the plan shown, and only that plan: if someone edited the space, or you edited the files, since the plan was made, nothing is published and the bottom line asks you to look at the new plan. `n` or `Esc` publishes nothing.

A plan Kiln refuses (a page edited in Confluence, an adopted page that has content) can't be published from the window. The bottom line names the command that overrides it, such as `kiln export spec --force`, so that decision is made deliberately in a terminal. Pruning retired pages (`--prune-removed`) is also left to the command line. See [Publishing to Confluence](Publishing-to-Confluence).

Publishing uses the token `kiln auth confluence` stored, or `KILN_CONFLUENCE_TOKEN`. The window never asks for one: run `kiln auth confluence` in a terminal first.

## Keys

Normal mode, as in Fenix's pages:

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑` | next / previous row |
| `C-d` `C-u`, `PgDn` `PgUp` | half a page down / up |
| `gg` `G`, `Home` `End` | first / last row |
| `Enter` | open the item (or code location, or finding) in Fenix at its line; on a comment, open it in the browser |
| `i` | Impact of the item under the cursor |
| `v` | Item: the item under the cursor |
| `C-o`, `C-i` | Item: the item viewed before / after |
| `/` | filter (see below) |
| `a` | Tree and Board: show closed items too, or hide them again |
| `m` | Trace: only items with a gap, or all again |
| `T` | Trace: the next traced type |
| `b` | Changes: pick another baseline |
| `w`, `W`, `P` | Exports: write this file target, write every file target, publish this Confluence target |
| `s`, `l`, `U`, `n`, `N`, `H`, `L` | the edits in [Editing](#editing) |
| `R` | reload now; on Exports and Comments, read them again too |
| `Tab`, `]` | next page |
| `[` | previous page |
| `1` to `9` | Tree, Trace, Impact, Board, Roadmap, Decisions, Changes, Item, Check |
| `SPC` | the leader key; see below |
| `q`, `C-q` | quit |
| `Esc` | clears a pending key, the leader, the filter or a question |

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
| `SPC v` | Item |
| `SPC k` | Check |
| `SPC p` | Exports |
| `SPC m` | Comments |
| `SPC e` | the edits: `s` status, `t` title, `p` parent, `o` owner, `d` due date, `l` link, `u` unlink, `n` new item, `N` new child, `r` renumber |
| `SPC R` | reload |
| `SPC h` | Home: the projects (also `gh`) |
| `SPC q` | quit |

`Esc` cancels a leader sequence. An unknown key says `SPC: no such key` in the footer.

### Mouse

| Action | Does |
|---|---|
| wheel | scrolls |
| click | moves the cursor to that row |
| double click | opens the row, as `Enter` does |

## Opening items

`Enter` (or a double click) opens the item under the cursor in Fenix at its title line, a code location or a finding at its line, and a comment in the browser. It uses `KILN_EDITOR` when it is set; otherwise it tries the running Fenix, then starts Fenix, then, when Fenix isn't installed, `VISUAL` or `EDITOR`, then the operating system's default editor for the file ([Editors and Flint](Editors-and-Flint#opening-items-in-fenix)). On a heading, the footer says `Nothing to open here`. See [Editors and Flint](Editors-and-Flint#opening-items-in-fenix).

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

Statuses show as coloured pills: green when done, red for `blocked`, grey when closed, blue for a type's first status (such as `draft` or `todo`), and purple for anything in between (such as `approved` or `doing`). The colours come from the theme.

Every key is optional. An unknown key, an unknown theme or a size out of range is reported in the footer and otherwise ignored; the window still opens. This file holds your personal looks and is not part of the project, so it is not in `kiln.toml`.

## What it doesn't do

- Edit an item's body, or any text longer than one line: open it in your editor with `Enter`.
- Show an item's body (the Item page shows its front matter and links).
- Undo an edit: use git.
- Override a refused Confluence plan, adopt a page, or prune retired pages: use `kiln export` with `--force`, `--adopt` or `--prune-removed`.
- Store a token: run `kiln auth confluence` or `kiln auth jira`.
- Create a project: run `kiln init` in its folder, then add it on Home with `a`.
- Delete a project's folder: `d` on Home only takes it off the list.
- Refresh the Jira mirror while open.
- Remember the last page between runs.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `couldn't connect to the display` | no graphical session. Run it on a desktop, or use the command line (`kiln trace`, `kiln impact`, `kiln diff`) |
| On Windows, nothing happens when you start `kilnw` | `kilnw` has no console to print to. It writes why the window couldn't open to `kilnw.log` in the temporary folder (`%TEMP%\kilnw.log`) |
| Home says `no kiln.toml in … any more` | the project moved or its `kiln.toml` was removed. `d` takes it off the list; `a` adds it where it is now |
| Changes page says "No baseline" | the repository has no tag. Tag a release, or start with `kiln gui --since <commit>` |
| `--since` fails | the revision doesn't exist in this clone. Fetch tags (`git fetch --tags`) |
| `Enter` doesn't open Fenix | `KILN_EDITOR` is set, which comes first; or Fenix isn't installed or not on `PATH`, and `VISUAL`, `EDITOR` or the default editor opens instead. If several Fenix instances run, set `FENIX_HOME` to choose one |
| Footer says the theme isn't known | check the spelling against the list above |
| An edit says the file "changed on disk since Kiln read it" | your editor saved the file at the same moment. Press the key again: the window has reloaded |
| Exports says "no token for …" | run `kiln auth confluence` in a terminal, or set `KILN_CONFLUENCE_TOKEN`, then press `R` |
| Jira tickets missing | no `[jira] url`, no token, Jira Cloud, or Jira unreachable; the footer says which. Try `kiln list --jira` in a terminal for the full error |

See also: [Command reference](Command-reference) · [Editors and Flint](Editors-and-Flint) · [Changes and baselines](Changes-and-baselines) · [Troubleshooting](Troubleshooting)
