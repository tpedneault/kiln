# Kiln with Fenix, Flint and Jira

What Kiln uses from each, how it degrades without them, and the changes
that would make the integration better (G1–G9). Statements about Fenix
were checked against its source at
`ce118a2af1a0b70733d0a2dd426f94365cf5eb1a`, about Flint at
`eb23547eb5939a52b8de06c298271ce4c8604927` (both 2026-10-12). File
references are relative to those repositories.

## Principles

The same as between Flint and Fenix:

1. **No tool requires another.** Kiln is complete on its own; Fenix and
   Flint gain from Kiln and never depend on it.
2. **Kiln never writes another tool's files.** Fenix's agenda, notebook
   and settings, Flint's config and state are read through their tools'
   own interfaces or not at all.
3. **Capabilities, not versions.** Ask the other side what it can do
   (`fenix/hello` capabilities, Flint's protocol version) and disable an
   action with its reason when something is missing.
4. **One framing.** Every channel is `Content-Length` + JSON-RPC 2.0: LSP,
   Fenix's control channel and Flint's provider protocol.

## Fenix

### Editing items: a language server (K4)

Fenix runs each language's own server per project root, chosen from the
project's `.fenix/tools.json` first, and since Fenix `471ed09` (its PR
#61, closing G3) any number of **additional** servers beside it: one
process per server and project, serving every language it lists, with
diagnostics, completion, hover, definition, references and code actions
merged and rename and formatting left to the language's own server. Its
client uses `completion`, `hover`, `definition`, `references`, `rename`
(with a multi-file preview), `publishDiagnostics`, `codeAction`,
`formatting` and `workspace/applyEdit`. So a Kiln project adds Kiln as an
additional server for Markdown and the languages of its code:

```json
{
  "lsp": {
    "rust": { "executable": "rust-analyzer" },
    "additional": [
      { "name": "kiln", "executable": "kiln", "args": ["lsp"],
        "languages": ["markdown", "rust"] }
    ]
  }
}
```

`kiln init --fenix` writes that: Markdown plus the language of every code
file `[code] paths` scans, merged into an existing `tools.json` (never
replacing it, keeping languages added by hand), and moving the older
form (`"markdown": {"executable": "kiln", …}`, from before G3) over so
Kiln doesn't run twice. Running it again after adding `[code] paths` adds
the new languages. A Fenix older than `471ed09` rejects `"additional"` as
an unknown field: update Fenix. What `kiln lsp` provides in item files:

| Request | Gives |
|---|---|
| completion | IDs (with title and status) after `[[`, in `parent = "`, in `[links]` arrays filtered by what the role allows; statuses in `status = "`; link roles in `[links]`; tags already used |
| hover | on any ID: title, type, status, parent, links both ways, coverage |
| definition (`gd`) | the item's file, at its `id` line; on a code location in an inverse list, that file and line |
| references (`gr`) | every link, mention and code reference to the item |
| rename | renames an ID everywhere (front matter, bodies, code references, the file name), the same as `kiln renumber` |
| diagnostics | `kiln check` findings for the open files, recomputed incrementally, plus project-wide ones on the `kiln.toml` |
| code actions | "Set status → …"; on an ID in the file, "Add `verifies` link to REQ-014" for each role allowed and not yet there; "New child requirement"; "Create test that verifies REQ-014" for each coverage role; "Rename file to …" when the name doesn't start with the ID (K105) |
| formatting | normalises front-matter key order and quoting without touching the body |

Markdown files that aren't items (a README) get only ID completion and
hover, so the server is harmless across a whole repository; outside a
project (no `kiln.toml` above the file) every request gets an empty answer.

In **code files** that `[code] paths` scans, beside the language's own
server: completion of IDs after a code reference's role word
(`// implements: REQ-0`, only the types that role may point at, without
those already on the line), hover, definition and references on any ID,
and diagnostics on the code references (an ID that doesn't exist, a role
code may not use), from the editor's text as it is typed. Formatting and
code actions stay with item files; a code file's formatting is its own
server's. A code file outside `[code] paths` gets hover and definition
only.

Every change is a workspace edit computed by the same code as the
command line (`set_field`, `add_link`, `plan_renumber`, `plan_new_item`);
the server never writes a file itself, so the editor's undo covers it.

**What Fenix's client does, at the pinned revision** (checked in K4,
`crates/fenix-lsp`, `crates/fenix-gui/src/lsp.rs`; nothing in Fenix
changed for it):

- It sends `didOpen` and `didChange` (whole or ranged, as the server asks;
  Kiln asks for whole documents), but not `didSave`, `didClose` or
  `workspace/didChangeWatchedFiles`. Kiln therefore watches the disk
  itself for files changed outside the editor (G8).
- It applies workspace edits made of text edits only:
  `resourceOperations` is empty and an edit carrying a file create or
  rename is refused whole. So rename renumbers every reference but leaves
  the file name (K105 then says so), and the code actions that create or
  rename a file come back disabled with the command that does it from a
  terminal (G7). Kiln offers them for real to a client that declares
  `create`/`rename`.
- It shows only code actions that carry an edit, no command and no
  `disabled`, so in Fenix the disabled ones above are simply absent.
- Hover is flattened to plain text, completion items' `textEdit`,
  `filterText` and `detail` are used, snippets are not: Kiln sends plain
  text and no snippets.

### Opening items in Fenix (K1 `new --open`, K5, K7)

Through Fenix's control channel, `crates/fenix-control` (landed in Fenix
#58/#59; documented in Fenix's `docs/CONTROL.md`). Kiln uses
`fenix_control::Client`: `connect(&Endpoint)`, `hello`, then `open` with a
path, line and column. Fenix not running: start `fenix <path>:<line>` (the
same `file:line` handling); Fenix not installed: `VISUAL` or `EDITOR`,
else the OS default editor via the file, and the message says so. Since
K16, `KILN_EDITOR` comes before all of that: a person who names an editor
for Kiln gets it, Fenix or not (ADR-019).

### Diagrams: `fenix-diagram` (K2)

`fenix_diagram::render(source, &Themes, theme)` → SVG, then
`fenix_diagram::png(svg, scale, Background::White)` for the Confluence
and HTML formats (EXPORT.md, "Diagrams"). The same renderer Fenix shows in its notebook and diagram
pages, so a diagram looks the same everywhere. Checked: builds with the
pins in DESIGN.md and renders a flowchart.

### Jira: `fenix-jira` (K8)

`fenix_jira::JiraClient::new(base_url, token)` with `search_issues` and
`build_keys_jql` for the read-only mirror of linked tickets (`kiln-jira`),
and one search to check a token in `kiln auth jira`. It speaks Jira
**Server/Data Center** with a personal access token only
(`crates/fenix-jira/src/client.rs`): a Jira Cloud URL (`*.atlassian.net`)
is refused by name, naming G6. Built and tested in K8 against a fake Jira
over HTTP, so the real client is what runs.

Two things it doesn't give, worked around:

- **Resolution.** `IssueSummary` has the status and its category, not the
  `resolution` field. Kiln counts a ticket in Jira's **Done** category as
  resolved, which is what every workflow agrees on whatever its statuses
  are called; a workflow that resolves tickets outside the Done category
  would read differently (not seen so far).
- **Proxy.** It builds no agent of its own, so requests go out directly:
  `HTTPS_PROXY` is not used (G9). A Jira inside the network, the usual Data
  Center case, needs none.

### Fenix's agenda: clock in on an item (K5)

`fenix/agenda.tasks` returns tasks with their `jiraKey`, and
`fenix/agenda.clockIn` takes an agenda task id. So "Start working on this"
works today for an item with a `jira` key that already has an agenda task
for that key. Otherwise the action is shown disabled: "no agenda task for
FLINT-88 (needs Fenix G1)".

### Gaps in Fenix

#### G1. Create an agenda task over the control channel <a id="g1"></a>

**Noticed by K5.** The control channel can list tasks and clock in, but
not create one. Add `fenix/agenda.createTask { title, jiraKey?, link?:
{ label, url } }` → `{ taskId }`, so Kiln (and Flint) can start tracking
time on an item that has no agenda task yet. The agenda stays Fenix's to
write (principle 2).

#### G2. A fallback font in `fenix-diagram` <a id="g2"></a>

**Noticed by K2/K3 in CI.** `fenix-diagram` builds its font database with
`load_system_fonts()` only (`crates/fenix-diagram/src/lib.rs`, `FONTS`).
In a slim CI container with no fonts, diagrams rasterise without text and
no error. Either bundle one small open font (e.g. DejaVu Sans, behind a
default feature) loaded when the system has none, or expose
`fn font_count() -> usize` and an API to add font data. Kiln guards
against this meanwhile (EXPORT.md, "Diagrams"). Fenix itself would
benefit on minimal Linux installs.

#### G3. More than one language server per file <a id="g3"></a>

**Noticed by K4; resolved in Fenix `471ed09` (PR #61).** Fenix keyed
sessions by (language, root), one server each, so `kiln lsp` could serve
Markdown but not also attach to `.rs` or `.tcl` files beside
rust-analyzer. Fenix now runs **additional** servers per buffer
(`"additional": [...]` under `"lsp"` in `tools.json`), with requests fanned
out and results merged. Kiln uses it: `kiln init --fenix` registers `kiln
lsp` as an additional server for Markdown and the project's code
languages, and the server answers in code files ("Editing items" above).

#### G4. `[[ID]]` in Fenix's Markdown reading view <a id="g4"></a>

**Cosmetic, any milestone.** Fenix's notebook treats `[[name]]` as a
notebook link; in a Kiln item opened in the reading view (`SPC m p`),
`[[REQ-014]]` would look like a missing note. When a language server
answers `definition` for the position, following the link should use it.

#### G5. Reload of files changed by another writer <a id="g5"></a>

**Already handled, recorded for completeness.** Kiln edits front matter
(`kiln set`, provider actions) of files that may be open in Fenix. Fenix
records disk state per buffer (`note_disk_state`) and has `:e!`; Kiln also
refuses to write a file changed since it read it (FORMAT.md, "Edits Kiln
makes"). If Fenix ever auto-reloads clean buffers silently, nothing more is
needed; if it prompts, that is the right behaviour too.

#### G6. Jira Cloud in `fenix-jira` <a id="g6"></a>

**Noticed by K8, only for Cloud users.** Basic auth with e-mail and API
token and the v3 search endpoint. Not needed for Data Center.

#### G7. File operations in workspace edits <a id="g7"></a>

**Noticed by K4.** Fenix's client declares no `resourceOperations` and
refuses a workspace edit containing `create`, `rename` or `delete`
(`fenix_lsp::workspace_edit::prepare`). Supporting `create` and `rename`
(with the rename shown in the multi-file preview, and an open buffer
following its file) would let `kiln lsp` rename an item's file with its
ID and create items from code actions ("New child requirement", "Create
test that verifies REQ-014", "Rename file to …"). Until then Kiln
offers those actions disabled, naming the command that does the same
(`kiln new … --open`), and rename leaves the file name for K105 to
report.

#### G8. File notifications to the language server <a id="g8"></a>

**Noticed by K4, small.** Fenix sends no `didSave`, `didClose` or
`workspace/didChangeWatchedFiles`, so a server doesn't learn of files
created, deleted or changed outside the editor (`kiln new` in a
terminal, a `git pull`). Kiln works around it by polling: item files and
`kiln.toml` every 2 s, code files every 10 s, on a thread of its own.
Sending `didChangeWatchedFiles` would make that immediate and free (Kiln
handles it already), and `didClose` would let a closed buffer's file
count from disk again.

#### G9. A proxy for `fenix-jira` <a id="g9"></a>

**Noticed by K8, only behind a proxy.** `fenix-jira` sends every request
through `ureq::get`/`ureq::request` with ureq's default agent, which (without
ureq's `proxy-from-env` feature) uses no proxy, so a Jira reachable only
through a corporate proxy can't be read. Kiln can't turn the feature on
for it: features unify across the build, and `kiln-confluence` would then
lose its own `NO_PROXY` handling. Either let a caller pass a
`ureq::Agent` to `JiraClient` (the simplest, and Kiln's Confluence agent
would serve), or read `HTTPS_PROXY`/`NO_PROXY` itself as Kiln's
`proxy_for` does.

## Flint

### Kiln as a Flint provider (K5), no Flint change

Flint spawns declared providers and speaks its protocol over stdio
(Flint's `docs/PROTOCOL.md`, `docs/PROVIDERS.md`). Kiln provides
`kiln provider`, built on `flint-protocol`'s types:

```toml
# Flint's settings.toml
[[providers]]
name = "kiln"
command = ["kiln", "provider"]
prefix = "k"
```

(`kiln provider --name … --prefix …` when the entry uses another name or
scope keyword: `hello` must repeat the name, and the drill-down actions
type the keyword.)

- **Which project**: `--project <dir>` pins one. Otherwise the one in the
  query's `context.fenix.project` (or `file`) when a `kiln.toml` is at or
  above it, remembered in Kiln's state folder (`flint-project`) for the
  next start; otherwise the last one remembered; otherwise the folder the
  provider started in. One project is loaded at a time; following Fenix to
  another loads it and replaces the items. Item files and `kiln.toml` are
  checked for changes every 2 s, so items stay current (Flint sends no
  file events; the provider polls, as `kiln lsp` does).
- **Items**: registered as **static items** (Flint matches and ranks them,
  so queries cost no round trip), replaced with `provider/setItems` when
  the index changes, at most every 250 ms. The provider is also `dynamic`,
  but answers only the drill-down queries below (`status `, `impact `,
  `all `) and returns nothing for anything else, so ordinary typing costs
  Flint nothing beyond its own matching. One item per non-closed item
  (closed ones through `k all `): title `REQ-014 Flint appears within 50
  ms…`, subtitle `requirement · approved · under REQ-002`, keywords the
  tags and Jira keys, icon `task` for tasks and milestones, `note` for
  decisions and designs, `ticket` for anything else until Flint has better
  icons (Flint's icon set is fixed; a `requirement` icon would be a Flint
  addition). Every item is in one `provider/setItems` message, under
  Flint's 4 MiB limit: 2,000 items are about 1.6 MiB; past roughly 4,500
  open items the provider registers the first ones and logs that the rest
  are behind `all`.
- **Item ids**: `kiln:<project name>:<ID>`, stable, for Flint's frecency.
- **Actions**, first is `Enter`:

| Action | Kind (Flint's protocol) |
|---|---|
| Open in Fenix | `fenix`, `method: "fenix/open"`, path + line (Flint disables it with the reason when Fenix lacks the capability) |
| Copy ID | `copy` |
| Copy reference `[[REQ-014]]` | `copy` |
| Set status… | `query` drill-down to `k status REQ-014 ` listing the statuses, each `provider` → `execute` → front-matter edit; `message` "REQ-014 → verified" |
| Show what depends on it | `query` drill-down to `k impact REQ-014 ` listing the impact set as items |
| Open in Confluence | `provider`, offered when a Confluence target exists: the page from the local page cache `kiln export` keeps (the item's own page, else the page holding it), else looked up by title in the space with the stored or environment token; opened in the browser at `#<ID>`; an error in the footer names what's missing (no token, not published) |
| Open in Jira | `open` with `<[jira] url>/browse/<KEY>`, when the item has a `jira` key |
| Start working on it | `provider` → `fenix/agenda.clockIn` on the open agenda task with the item's Jira key (see "Fenix's agenda"); offered only for items with a `jira` key; without such a task the footer says "no agenda task for FLINT-88 (needs Fenix G1 to create one)" |
| Open in Kiln | `run` `["kilnw", "gui", "--project", ROOT, "--item", ID]` (`kiln` instead of `kilnw` off Windows), key `g`: Kiln's window on that item (GUI.md) |

Drill-down results: `k status REQ-014` lists the type's statuses as
`status REQ-014 → verified` (the current one marked), each a `provider`
action that makes the same stamp-checked front-matter edit as `kiln set`
and answers "REQ-014: approved → verified"; `k impact REQ-014` lists what
reaches it (items with their usual actions, code locations opening in
Fenix at the line), as `impact REQ-014 ← TEST-004 …`; `k all text` lists
every item, closed ones included, whose ID and title contain the text, as
`all REQ-005 …`. Any other query gets an empty answer at once.

- **Context boost**: Flint already ranks items from the current Fenix
  project first.

### Gaps in Flint

None blocking. Five small ones worth raising in Flint's own repository
(recorded with K5; Flint not modified):

- **Drill-down verbs.** Flint matches a provider's static items against
  every query, including a drill-down like `k status REQ-014`, so the
  static `REQ-014` item ranks above the dynamic status choices (Flint keeps
  results that match the text first, others after; its
  `docs/PROVIDERS.md`). A registration field such as `verbs: ["status",
  "impact", "all"]`, meaning "when a scoped query starts with one of these
  words, hide my static items", fixes it. Until then, Kiln titles its
  drill-down results so they contain the query text (`status REQ-014 →
  verified`) and therefore match and rank first.
- **Disabled actions.** An `Action` has no `disabled` reason, so an action
  that can't work for an item (clock-in without an agenda task, Open in
  Confluence before a publish) is either left out or fails in the footer
  when chosen. Kiln offers them where they can work and explains in the
  footer otherwise. A `disabled: { reason }` field, as LSP code actions
  have, would let the menu show why up front.
- **The scope keyword in drill-down text.** A `query` action's text must
  carry the provider's scope keyword (`k status REQ-014`), but settings may
  override the keyword. Kiln's `--prefix` has to be kept in step by hand;
  a placeholder Flint expands (`{scope} status REQ-014`) would remove that.
- an icon for requirements/specifications in Flint's icon set;
- `provider/setItems` for a few thousand items is fine at Flint's 250 ms
  re-index cap, but should be measured with Kiln's benchmark project in
  Flint's index benchmark.

## Jira

- **Links**: `jira = "KEY-1"` on an item (FORMAT.md). Rendered as the Jira
  macro or a link in Confluence (CONFLUENCE.md, "Jira"); opened from Flint.
- **Mirror (K8)**: `kiln list --jira` and the GUI show each linked
  ticket's status and assignee, and whether it is resolved (in the Done
  category), fetched read-only through `fenix-jira` in searches of up to
  100 keys (`key IN (…)`; when Jira refuses one because a key doesn't
  exist, that batch is asked again key by key and the unknown keys are
  reported as such). The mirror is cached in the state folder
  (`jira/<host>.json`) for 10 minutes, so `list --jira`, `check` and the
  GUI share one fetch. The GUI loads it on a thread of its own and shows
  each ticket after the item's status. `kiln check` rule K207 (off by
  default): an item done whose ticket isn't resolved, or the reverse
  (FORMAT.md); when Jira can't be read, the rest is checked and stderr
  says why K207 wasn't.
- **Sync (K10, ADR-014)**: with `[jira] project`, `kiln jira sync` keeps
  one project issue (`[jira] parent`, created by the first sync) and a
  sub-task per synced item (`[jira] types`, tasks by default) in step:
  it creates the sub-tasks and keeps their summary (`TASK-001 Title`),
  due date and status, by the transition that reaches the mapped status
  (`[jira.status]`, else the status category). Keys it creates are
  written into the items and `kiln.toml`, for the person who ran it to
  commit. Managed tickets are the project issue's sub-tasks (`parent =
  KEY`); every other ticket is never written. Nothing is deleted: a
  sub-task no item links is reported. `kiln jira pull` takes a ticket's
  status back into its item. Everything it uses is in `fenix-jira` at the
  pinned rev: `issue_types`, `create_fields`, `create` (a sub-task by its
  `parent` field), `update_summary`, `update_due`, `list_transitions`,
  `apply_transition`, `search_issues`.
- **Never**: assignees, sprints, comments, worklogs, deleting a ticket,
  or writing a ticket that isn't a sub-task of the project issue. That
  is Fenix's Jira page and Jira's own job.
- **Token**: `KILN_JIRA_TOKEN`, else the credential store under the suite's
  name `jira:<url>` (service `fenix-suite`), which Flint's M8 already
  uses, so a token stored once serves Flint and Kiln.

## Credential names, for the whole suite

Service `fenix-suite`; account `<kind>:<url>`, URL as configured without a
trailing slash. Kiln uses `confluence:<url>` and `jira:<url>`. Flint uses
`jira:<url>` and `gitlab:<url>` (its `crates/flint-platform/src/credentials.rs`).
Fenix still keeps its tokens in `settings.toml`; when it moves them (its
docs say this is planned, Flint's F12), the same names make one stored
token work in all three.

## Summary

| # | Change | Where | Noticed by | Size |
|---|---|---|---|---|
| G1 | `fenix/agenda.createTask` over the control channel | Fenix | K5 | small |
| G2 | Fallback font in `fenix-diagram` | Fenix | K2/K3 in CI | small |
| G3 | Additional language servers per buffer | Fenix | K4 (code files) | done (Fenix `471ed09`) |
| G4 | `[[ID]]` follows LSP definition in the reading view | Fenix | cosmetic | small |
| G5 | (handled) reload after outside edits | Fenix | — | — |
| G6 | Jira Cloud in `fenix-jira` | Fenix | K8, Cloud only | small |
| G7 | File create/rename in workspace edits | Fenix | K4 | medium |
| G8 | `didChangeWatchedFiles` (and `didClose`) to language servers | Fenix | K4 | small |
| G9 | A proxy (or a caller's agent) for `fenix-jira` | Fenix | K8, behind a proxy | small |

Nothing here blocks K0–K8.
