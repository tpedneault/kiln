# The GUI

`kiln` with no arguments (or `kiln gui`) opens a window onto the project:
seven pages, each a full window like Fenix's pages, driven from the
keyboard with Vim keys and the `SPC` leader. It reads; the one thing it
does outward is open an item in Fenix. It never edits prose, and it edits
no front matter either: that is `kiln set`, the language server, and
Flint's `k status` (DESIGN.md, "GUI").

Designed after K6, as DESIGN.md planned; built in K7.

## What it is for

Kiln's command line answers one question at a time (`kiln trace`, `kiln
impact REQ-014`, `kiln diff v1.0`). The GUI is for looking around: the
whole tree, the gaps, what a change touches, where the milestones stand,
what changed since a release, moving between them without retyping, and
jumping into Fenix at the item being looked at. It is a reader's tool,
which is why it has no editing: the files are edited in Fenix, and the
GUI follows them within two seconds.

## Pages

Every page is a list of rows. A row is one line: an item, a code
location, or a heading that groups the rows under it (headings are skipped
by the cursor). Columns are aligned by padding, in the monospace font.

| Key | Page | Rows |
|---|---|---|
| `SPC t` | **Tree** | every open item, nested by `parent` under a heading per type: `REQ-014  Title  approved`; children indented under their parent. Closed items are hidden until `a` |
| `SPC x` | **Trace** | per open item of the traced type (default `requirement`, `T` cycles through types some coverage role reaches): its ID and title, then one column per coverage role: what covers it, or `gap` in the error colour. `m` keeps only items with a gap |
| `SPC i` | **Impact** | for the item picked with `i` on any page: everything that reaches it by links or parents, transitively, nearest first, each with its path (`verifies REQ-014`, `child of REQ-002`); code locations included |
| `SPC b` | **Board** | the work items (type `task`, else every type with `done` statuses except milestones) grouped under a heading per status, in the type's status order, with counts. Closed statuses are hidden until `a` |
| `SPC r` | **Roadmap** | milestones in `due` order (undated last), each with status, due date and progress (`3/5 ███░░`), and the items `part-of` it under it |
| `SPC d` | **Decisions** | the decision log in ID order: status, then `supersedes ADR-001` / `superseded by ADR-007` |
| `SPC c` | **Changes** | the items added, changed and removed since the baseline (`kiln gui --since REV`, else the most recent tag), grouped under Added / Changed / Removed, each with what changed (`status draft → approved; + verifies REQ-002`) |

`Tab` and `S-Tab` (or `1`–`7`) step through the pages in that order. The
top line shows the pages, the current one marked, and the project's name;
the bottom line shows the filter, a message, or the keys that work here.

With `[jira] url` and items that link tickets, every item row ends with
its tickets once the Jira mirror has arrived: `FLINT-12 In Review ·
Alice`, green when resolved, muted otherwise (`FLINT-12 ?` when Jira
doesn't know the key). The mirror loads on a thread of its own, from the
cache when it is fresh (INTEGRATION.md, "Jira"), so a slow or absent Jira
never holds up the window; when it can't be read, the footer says why.

Every page keeps its own cursor and scroll position. When the project
reloads, the cursor stays on the same item (by ID), or on the row where
it was when that item went away.

## Keys

Normal mode, as in Fenix's pages:

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑` | next / previous row |
| `C-d` `C-u`, `PgDn` `PgUp` | half a page down / up |
| `gg` `G`, `Home` `End` | first / last row |
| `Enter` | open the item (or code location) in Fenix at its title line (`kiln new --open`'s path: the running Fenix, else a new one, else the default editor) |
| `i` | Impact of the item under the cursor |
| `/` | filter: typing narrows the page to rows whose text contains it (case-insensitive; in Tree, ancestors of a match stay); `Enter` keeps the filter, `Esc` clears it |
| `a` | show closed items too (Tree, Board), or hide them again |
| `m` (Trace) | only items with a gap (missing coverage) |
| `T` (Trace) | the next traced type |
| `R` | reload now |
| `SPC` | the leader: the pages above, `SPC R` reload, `SPC q` quit, with the which-key popup Fenix shows |
| `q`, `C-q` | quit |
| `Esc` | clears a pending key, the leader or the filter |

Mouse: the wheel scrolls, a click moves the cursor, a double click opens.

## Looks

The same stack as Fenix and Flint (winit 0.30, wgpu 30, glyphon 0.12), so
text and colours look alike: Fenix's themes, copied as Flint copies them,
and the monospace font Fenix picks. `settings.toml` in Kiln's settings
folder (CONFIGURATION.md) may say:

```toml
theme = "Nord"            # one of Fenix's themes; default Orbit Dark
font = "Cascadia Mono"    # default: the monospace font Fenix would pick
font_size = 15            # points, 8–40
```

A theme, font or size Kiln doesn't know is ignored with a line in the
footer, not an error.

## How it is built

`kiln-gui` keeps everything that can be tested without a display in plain
modules: each page is a function from the project (and its own state) to
rows (`pages`); the application is a state machine from key presses to a
`Screen` of lines of styled spans and an effect (open this file at this
line, quit, reload) (`app`). The window, the GPU and the text (`window`,
`gpu`, `render`) only draw a `Screen` and turn winit's events into key
presses. Tests drive `app` with keys and compare screens as text.

The binary loads the project (and the baseline's project, from git, once)
and hands them over; the window polls the item files and `kiln.toml` every
two seconds and reloads the project when they change, recomputing the
baseline's diff against the new project. The Jira mirror arrives the same
way, as a function the binary hands over and the window runs once on a
thread, so the GUI never handles a token.

## Windows: two executables

`kiln.exe` stays a console program: Kiln is first a command-line and CI
tool, and a GUI-subsystem program is not waited for by `cmd.exe`, which
would break exit codes in scripts. `kilnw.exe`, built from the same crate,
is a GUI-subsystem program that only opens the window: what a shortcut or
Flint's "Open in Kiln" starts, with no console window. On Linux and macOS
there is one `kiln`. (ADR-010 records the tradeoff; DESIGN.md's earlier
"`--console` on Windows" was Fenix's and Flint's answer, for programs
that are GUIs first.)

## Not built

Editing anything; a detail pane with the item's body (Fenix shows it);
refreshing the Jira mirror while the window is open (it is read once, at
start); Kiln-specific themes; remembering the last page between runs.
