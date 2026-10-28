# The GUI

`kiln` with no arguments (or `kiln gui`) opens a window onto the project:
eleven pages, each a full window like Fenix's pages, driven from the
keyboard with Vim keys and the `SPC` leader. It reads the project, edits
its front matter (statuses, titles, parents, owners, due dates, links, new
items, IDs), shows `kiln check`'s findings, and plans, writes and publishes
exports. It never edits prose: an item's body is written in Fenix, which
`Enter` opens at the item (ADR-012). Started outside a project, it opens
on Home, a projects dashboard after Fenix's, which adds, removes and
switches between projects.

Designed after K6, as DESIGN.md planned; built in K7; editing and the
Item, Check, Exports and Comments pages in K9; Home in K12 (REQ-017).

## Home

Which project the window opens:

| Started | Opens |
|---|---|
| `kiln`, `kiln gui` in a project (its folder or one below) | that project's pages, as before |
| `--project PATH` | that project's pages; no `kiln.toml` there or above is an error, as for every command |
| anywhere else (a shortcut, the Start menu, a terminal in `~`) | Home |

Home is Fenix's Home cut to what Kiln has: the lockup and the date, a
find field, then two columns, the projects and a preview of the one under
the cursor, stacked into one when the window is narrower than 76 cells:

```text
[Home]  2 projects

 ▣ kiln                                         Wednesday 21 October 2026

  ›  Find a project                                                    /

projects 2 ─────────────────── a add   Kiln ─────────────────── Enter open
 1  Kiln                       2 h ●   ~/src/kiln
    ~/src/kiln
 2  Flint                      3 d ○   items     42 · 30 open
    ~/src/flint                        types     requirement 12 · decision 14
 +  add a project                  a   next      MS-003 Beta · due 2026-11-30 · 3/5
                                       check     2 errors · 1 warning
                                       opened    2 h ago
```

Each project shows its name (`kiln.toml`'s, its folder's until read),
its path (the home folder as `~`), how long since it was last opened,
and a dot for how `kiln check` finds it: green clean, the warning colour
with warnings, red with errors or when it can't be read (no `kiln.toml`
there any more), hollow while it is being read. The preview shows its
items (how many, how many open, by type), its next open milestone with
its due date and progress, and the findings. Projects and their findings
are read on a thread when Home opens, so a slow disk never holds it up.

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑`, `C-n` `C-p` | next / previous project (the last entry is *add a project*) |
| `gg` `G`, `Home` `End` | first / last |
| `Enter`, a double click | open the project (on *add a project*: add one) |
| `1`–`9` | open the first nine directly |
| `/` | the find field: typing narrows the list by name or path; `↓` `↑` move, `Enter` opens, `Esc` clears |
| `a` | add a project: type its folder (`Tab` completes folder names, `~` is the home folder); a folder inside a project adds the project. A folder without a `kiln.toml` in it or above is refused with the reason, the prompt staying open |
| `d` | take the project under the cursor off the list, after a `y`; its folder is untouched |
| `R` | read the projects again |
| `q`, `C-q` | quit |

From any page, `SPC h` or `gh` goes back to Home, with the project just
left first and under the cursor. Not while a job runs (planning,
writing or publishing exports, reading comments): its answer would have
no page to land on, so the footer says to wait.

The list is `projects.toml` in Kiln's state folder (CONFIGURATION.md),
most recently opened first. Every project the window opens joins it,
from the command line or from Home. It is in the state folder rather
than beside `settings.toml` because its paths are this machine's: on
Windows the settings roam, and a list of another machine's folders would
only be noise. A list Kiln can't read starts Home empty with the reason
in the footer; Kiln rewrites the file whole.

## What it is for

Kiln's command line answers one question at a time (`kiln trace`, `kiln
impact REQ-014`, `kiln diff v1.0`) and makes one change at a time (`kiln
set REQ-014 status=approved`). The GUI is for looking around and for the
small changes that looking around turns up, without retyping IDs: the
whole tree, the gaps, what a change touches, where the milestones stand,
what changed since a release, what `kiln check` finds, what an export
would change; moving an item along the board, linking a test to the
requirement it verifies, adding a child, publishing the design document.
What it leaves to an editor is prose: the GUI has one-line inputs and no
text area. The files are the source of truth; the GUI follows them within
two seconds, whoever edits them.

## Pages

Every page is a list of rows. A row is one line: an item, a code
location, a finding, an export's line, a comment, or a heading that groups
the rows under it (headings are skipped by the cursor). Columns are
aligned by padding, in the monospace font.

| Key | Page | Rows |
|---|---|---|
| `SPC t` | **Tree** | every open item, nested by `parent` under a heading per type: `REQ-014  Title  approved`; children indented under their parent. Closed items are hidden until `a` |
| `SPC x` | **Trace** | per open item of the traced type (default `requirement`, `T` cycles through types some coverage role reaches): its ID and title, then one column per coverage role: what covers it, or `gap` in the error colour. `m` keeps only items with a gap |
| `SPC i` | **Impact** | for the item picked with `i` on any page: everything that reaches it by links or parents, transitively, nearest first, each with its path (`verifies REQ-014`, `child of REQ-002`); code locations included |
| `SPC b` | **Board** | the work items (type `task`, else every type with `done` statuses except milestones) grouped under a heading per status, in the type's status order, with counts. Closed statuses are hidden until `a`; `H` and `L` move an item to the previous or next status |
| `SPC r` | **Roadmap** | milestones in `due` order (undated last), each with status, due date and progress (`3/5 ███░░`), and the items `part-of` it under it |
| `SPC d` | **Decisions** | the decision log in ID order: status, then `supersedes ADR-001` / `superseded by ADR-007` |
| `SPC c` | **Changes** | the items added, changed and removed since the baseline (`kiln gui --since REV`, else the most recent tag; `b` picks another), grouped under Added / Changed / Removed, each with what changed (`status draft → approved; + verifies REQ-002`) |
| `SPC v` | **Item** | for the item picked with `v` on any page: what `kiln show` prints, one row per related item or code location (parent, children, links both ways, mentions, coverage, progress, Jira tickets). `v` on a row views that item; `C-o` and `C-i` go back and forward |
| `SPC k` | **Check** | `kiln check`'s findings (K207 too, once the Jira mirror is here), grouped by file: `12:3  K102 error  REQ-099 doesn't exist`. Recomputed on a thread after every reload; the top line shows the error count on every page |
| `SPC p` | **Exports** | `kiln export --dry-run`, target by target, as the command line prints it: the target's line as the heading, then what would change, "leaves out", and for Confluence the plan's operations; with `[jira] project`, `kiln jira sync --dry-run` as one more target, `jira`. `w` writes a file target, `W` all of them, `P` publishes a Confluence target or syncs Jira |
| `SPC m` | **Comments** | `kiln confluence comments`: the unresolved comments under each Confluence target, by the item they are about; `Enter` opens one in the browser |

`Tab` and `]` (`[` back), or `1`–`9` for the first nine, step through
the pages in that order. The top line shows the pages, the current one marked, the
project's name and, when `kiln check` finds errors, how many; the bottom
line shows the prompt, the filter, a message, what is running, or the keys
that work here.

Exports and Comments are read on a thread when their page is first opened
(they read git, render the document, and for Confluence ask the server),
and again with `R`; the page says what it is waiting for, and the window
stays responsive meanwhile. One such job runs at a time.

With `[jira] url` and items that link tickets, every item row ends with
its tickets once the Jira mirror has arrived: `FLINT-12 In Review ·
Alice`, green when resolved, muted otherwise (`FLINT-12 ?` when Jira
doesn't know the key). The mirror loads on a thread of its own, from the
cache when it is fresh (INTEGRATION.md, "Jira"), so a slow or absent Jira
never holds up the window; when it can't be read, the footer says why.

Every page keeps its own cursor and scroll position. When the project
reloads, the cursor stays on the same item (by ID), or on the row where
it was when that item went away.

## Editing

The edits are those of the editing commands (CLI.md, "Editing"), on the
item under the cursor, on any page:

| Keys | Do | Same as |
|---|---|---|
| `s`, `SPC e s` | set the status: pick one of its type's statuses | `kiln set ID status=…` |
| `H` / `L` (Board) | move to the previous / next status in the type's order | `kiln set ID status=…` |
| `SPC e t` | the title | `kiln set ID title=…` |
| `SPC e p` | the parent: pick an item of an allowed type, or none | `kiln set ID parent=…` |
| `SPC e o` / `SPC e d` | the owner / the due date (`YYYY-MM-DD`); empty removes it | `kiln set ID owner=… due=…` |
| `l`, `SPC e l` | link: pick a role the item's type may start, then a target the role allows that isn't linked yet | `kiln link ID ROLE TARGET` |
| `U`, `SPC e u` | unlink: pick one of the item's links, then `y` | `kiln unlink ID ROLE TARGET` |
| `n`, `SPC e n` | a new item: pick its type, type its title; it opens in Fenix | `kiln new TYPE --title … --open` |
| `N`, `SPC e N` | a new child of the item: pick a type that may have it as parent | `kiln new TYPE --parent ID --open` |
| `SPC e r` | renumber: type the new ID (the next free one is offered), then `y`. On a duplicate-ID finding on Check, it renumbers that file's copy | `kiln renumber ID --to … [--file …]` |

A question is asked on the footer line, and a pick's choices show above it
narrowing as you type: `↓` `↑` (or `C-n` `C-p`, `Tab`) move, `Enter`
answers, `C-u` clears, `Esc` cancels. The answer is checked as the command
would check it before anything is written; a value the configuration
doesn't allow (`soon` as a date, a parent that would make a cycle) leaves
the prompt open with the reason. The edit then goes through the same code
as the command: it changes only what it is about, keeps comments, layout
and line endings, and refuses a file that changed on disk since it was
read (an editor's save), naming it. The footer shows what the command
would have printed (`REQ-014: status draft → approved`), the project
reloads at once, and the cursor follows the item.

There is no undo: the files are in git, and the footer names what changed.

### Exports and publishing

On Exports, `w` writes the file target under the cursor (Markdown, HTML,
Word, wiki markup) and `W` every file target, as `kiln export NAME…` does;
the page plans again afterwards. `P` on a Confluence target asks `Publish
spec (confluence, document) → space FLINT, page 123 "Flint"? y/n`, naming
the space and the parent page, and on `y` publishes exactly the plan
shown: if the space or the files moved on since it was read, nothing is
published and the footer says to look at the new plan. A plan Kiln refuses
(a page edited in Confluence, an adopted page with content) is not
published from the GUI; the footer names the command that overrides it
(`kiln export spec --force`), which stays on the command line where the
decision is written down. Publishing needs the token `kiln auth
confluence` stored (or `KILN_CONFLUENCE_TOKEN`); the binary reads it, the
GUI never sees it.

## Keys

Normal mode, as in Fenix's pages:

| Keys | Do |
|---|---|
| `j` `k`, `↓` `↑` | next / previous row |
| `C-d` `C-u`, `PgDn` `PgUp` | half a page down / up |
| `gg` `G`, `Home` `End` | first / last row |
| `Enter` | open the item (or code location, or finding) in Fenix at its line (`kiln new --open`'s path: `KILN_EDITOR`, else the running Fenix, else a new one, else `VISUAL` or `EDITOR`, else the default editor); on a comment, open it in the browser |
| `i` | Impact of the item under the cursor |
| `v` | Item: the item under the cursor, as `kiln show` shows it |
| `C-o` `C-i` (Item) | the item viewed before / after |
| `/` | filter: typing narrows the page to rows whose text contains it (case-insensitive; in Tree, ancestors of a match stay); `Enter` keeps the filter, `Esc` clears it |
| `a` | show closed items too (Tree, Board), or hide them again |
| `m` (Trace) | only items with a gap (missing coverage) |
| `T` (Trace) | the next traced type |
| `b` (Changes) | pick another baseline: a tag, or any revision typed |
| `w` `W` `P` (Exports) | write the file target / every file target; publish the Confluence target, or sync Jira |
| `J` | take the item's status from its Jira ticket (`kiln jira pull ID`) |
| `s` `l` `U` `n` `N`, `H` `L` | the edits above |
| `R` | reload now (and, on Exports and Comments, read them again) |
| `SPC h`, `gh` | Home: the projects (see "Home") |
| `SPC` | the leader: the pages above, `SPC e` the edits, `SPC R` reload, `SPC h` Home, `SPC q` quit, with the which-key popup Fenix shows |
| `q`, `C-q` | quit |
| `Esc` | clears a pending key, the leader, the filter or the prompt |

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

An item's status is drawn as a pill, a rounded band in its own colour,
coloured by what its type says the status means rather than by its name,
so it reads the same in every project: done statuses green, `blocked`
red, closed statuses muted, the type's initial status (a draft, a to-do)
in the headings' blue, and anything between (approved, doing, active) in
the theme's purple. The green and purple are each theme's own (Fenix's
staged and picker colours; Catppuccin's green and mauve), so the pills
stay legible on light themes too.

## How it is built

`kiln-gui` keeps everything that can be tested without a display in plain
modules: each page is a function from the project (and what has been
gathered about it, and its own state) to rows (`pages`); the prompt is a
small state machine of its own (`prompt`); the application is a state
machine from key presses to a `Screen` of lines of styled spans and an
effect (open this file at this line, open this address, make this edit,
reload, quit) (`app`). The window, the GPU and the text (`window`, `gpu`,
`render`) only draw a `Screen` and turn winit's events into key presses.
Tests drive `app` with keys and compare screens as text.

Home is the same shape: `home` is keys in, a `Screen` and an effect
(open this project, save the list, quit) out, laid out in cells so tests
read it as text; `projects` reads and writes the list and summarises a
project. A `Screen` from Home also carries bands (the find field, the
key cap, the entry under the cursor with Fenix's focus rail) and rules
placed in cells, which the renderer draws at the font's measured
advance, and where the lockup goes: `kiln-brand` draws it at the pixel
height of three rows in the theme's text colour, and `image` draws it as
one textured quad (Fenix's `pdf_texture` pipeline, cut down). The window
shows either Home or a project with its threads; opening a project from
Home runs on a thread through a function the binary hands over (the
same one `kiln gui` opens with: the project, its baseline, its Jira
mirror and its host), so the window never blocks on git. Switching drops
the project shown with its threads' answers.

Edits are decided in `app`, checked with `kiln-index`'s `plan_set` and
`check_link` against the project shown, and made by `edit::apply`, which
reads the project again and calls `kiln-index`'s `apply_set`,
`apply_link`, `apply_renumber` or `create_item`: the functions `kiln set`,
`link`, `unlink`, `renumber` and `new` call, and the Flint provider's
status change too.

The binary loads the project (and the baseline's project, from git, once)
and hands them over with a `Host`: what the GUI can't do itself because it
needs git, the export pipeline, Confluence or a token. The app queues
`Job`s (plan the exports, write some, publish one, read the comments, list
the tags, read the project at a revision); the window runs one at a time
on a thread through the host, which runs the commands' own code
(`export::run_with` with its output captured, `confluence::read`) and
wakes the window with the answer. The window polls the item files and
`kiln.toml` every two seconds and reloads the project when they change,
recomputing the baseline's diff. `kiln check` then runs on a thread of
its own, which reads the project from disk itself, since on a large
project it takes seconds the window shouldn't freeze for; until its
findings arrive, Check shows the previous ones and says it is checking
again, and a newer check replaces one still running. The Jira
mirror arrives the same way, as a function the binary hands over and the
window runs once on a thread, so the GUI never handles a token.

## Windows: two executables

`kiln.exe` stays a console program: Kiln is first a command-line and CI
tool, and a GUI-subsystem program is not waited for by `cmd.exe`, which
would break exit codes in scripts. `kilnw.exe`, built from the same crate,
is a GUI-subsystem program that only opens the window: what a shortcut or
Flint's "Open in Kiln" starts, with no console window. A shortcut usually
starts in a folder that isn't a project, so it opens on Home. On Linux and macOS
there is one `kiln`. (ADR-010 records the tradeoff; DESIGN.md's earlier
"`--console` on Windows" was Fenix's and Flint's answer, for programs
that are GUIs first.)

## Icon

Kiln's mark is a fired block in clay, its three faces standing a little
apart, with Fenix's Flame burning in its mouth: drawn the way Fenix's
Forged F is (a 48-unit grid, every slanted edge rising 1 : 2, flat
colour, the same graphite tile and Martian Mono wordmark), so the two
read as one suite. `kiln-brand` draws it from one vector definition: the
window's icon, and the `.ico` that `crates/kiln/build.rs` embeds in
`kiln.exe` and `kilnw.exe` on every Windows build, which Explorer, a
taskbar pin and the Start menu show. `kiln.ico` and the logos in
`docs/brand/` are copies for use outside a build, rewritten by `cargo run
-p kiln-brand --example write-icon` and checked against the geometry by a
test. On Linux the window icon shows under X11; Wayland takes a desktop
file's icon, and Kiln installs no desktop file.

![Kiln](brand/kiln-icon-256.png)

## Not built

Editing an item's body, or any multi-line text (Fenix does that: `Enter`
opens it); undo (git is the undo); overriding a refused Confluence plan,
adopting a page or pruning retired ones (`kiln export --force`, `--adopt`,
`--prune-removed`); `kiln auth` (the GUI never handles a token); `kiln
init` (Home adds folders that have a `kiln.toml`; it doesn't make one);
deleting a project's folder (Home only takes it off the list); pinning or
grouping projects on Home, which Fenix's project hub does; two projects
in one window at once; refreshing the Jira mirror while the window is
open (it is read once, when the project opens); Kiln-specific themes;
remembering the last page between runs.
