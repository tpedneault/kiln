This page shows how to work on Kiln items from your editor and from the Flint launcher. It covers the language server (`kiln lsp`) and what it does for you while you write, how to set it up in the Fenix editor and in other editors, how to open items in Fenix, and the Flint provider (`kiln provider`). Read it once your project has a few items and you want completion, live checking and quick navigation instead of running commands by hand.

Fenix (https://github.com/tpedneault/fenix) is a text editor and Flint (https://github.com/tpedneault/flint) a keyboard launcher, both from the same suite as Kiln. Kiln works fully without them; they only add convenience.

## The language server

`kiln lsp` is a language server: a program your editor starts in the background and talks to over stdin and stdout using the Language Server Protocol (LSP). It reads your project the same way the command line does, so what you see in the editor matches `kiln check` and `kiln show`.

It finds the project from the editor's workspace folder, or from the first file you open, by walking up to the nearest `kiln.toml`. It serves Markdown files:

- **Item files** (files with Kiln front matter) get everything below.
- **Other Markdown files** in the project, such as a README, get only ID completion and hover, so the server is harmless across a whole repository.
- **Markdown files outside any project** get empty answers.

### What it offers

| Feature | What you get | When it helps |
|---|---|---|
| Completion | IDs (with title and status) after `[[`; IDs in `parent = "` (only types allowed as parent); IDs in `[links]` arrays (only types the role allows, not already listed); statuses after `status = "`; types after `type = "`; tags already used in the project; link roles at the start of a line in `[links]` | You type `verifies = ["` and pick the requirement from a list instead of looking up its number |
| Hover | On any ID: what `kiln show` prints for it (title, type, status, parent, links both ways, coverage) | You read `[[REQ-014]]` in a body and want to know what it says without opening it |
| Go to definition | Jumps to the item's `id` line (every copy when the ID is duplicated). On a code location, to that file and line | You follow a `parent` or a link to the item it names |
| Find references | Every link, `parent`, mention and code reference to the item (and its own `id` line, when asked) | Before you change a requirement, you list what points at it |
| Rename | Renumbers the ID everywhere, as `kiln renumber` does: front matter, bodies, code references, and the file name when the editor can rename files | Two branches picked the same number and you move one |
| Diagnostics | `kiln check` findings for the open files and for `kiln.toml`, recomputed after each edit | You misspell a status or link to an item that doesn't exist, and see it at once |
| Code actions | Set status; add a link from this item to the ID under the cursor, for each role allowed and not yet there; new child item; create a test (or design, task…) that covers this item; rename the file to match its ID | You mark a requirement approved, or create the test that verifies it, without leaving the file |
| Formatting | Puts front-matter keys in the standard order and quotes strings with `"…"`. The body is untouched | You hand-edited front matter and want it tidy |

A hover on `REQ-002` inside a test's `[links]` shows:

```text
REQ-002 Same bytes on Windows and Linux
requirement · verified · design/requirements/REQ-002-same-bytes-on-windows-and-linux.md:2
parent          REQ-001 Exports are deterministic
implemented by  TASK-001 Sort every unordered list
verified by     TEST-001 Export twice and compare
coverage        implements: yes, verifies: yes
```

Code action titles look like `Set status → approved`, ``Add `verifies` link to REQ-002``, `New child requirement`, `Create test that verifies REQ-001` and `Rename file to …`.

### How it changes files

The server never writes a file. Every change comes back to the editor as an edit, which the editor applies, so your editor's undo covers it. The edits are computed by the same code as `kiln set`, `kiln link`, `kiln renumber` and `kiln new`.

Some actions create or rename a file: "New child …", "Create test that verifies …", "Rename file to …", and the file-name part of rename. The server offers these for real only to editors that declare support for file operations in workspace edits (the LSP capability `workspace.workspaceEdit.resourceOperations` with `create` and `rename`). Other editors get them disabled, with a reason that names the command to run instead, for example `kiln new requirement --parent REQ-001 --open`. In that case rename still updates every reference but leaves the file name, and `kiln check` reports K105 (file name doesn't start with the ID) until you rename the file.

## Setting it up in Fenix

Fenix runs Kiln as an **additional** language server: beside each
language's own server (rust-analyzer for Rust, for example), for Markdown
and for the languages of your code. You need a Fenix recent enough to have
additional servers (October 2026 or later). From the project root:

```console
$ kiln init --fenix
.fenix/tools.json
kiln: Fenix now runs `kiln lsp` beside each language's own server for markdown in this project (`:lsp-restart` to pick it up)
```

In a new folder, `kiln init --name Demo --fenix` starts the project and does this in one step. The file it writes is:

```json
{
  "lsp": {
    "additional": [
      {
        "args": [
          "lsp"
        ],
        "executable": "kiln",
        "languages": [
          "markdown"
        ],
        "name": "kiln"
      }
    ]
  }
}
```

The languages are Markdown plus the language of every code file Kiln
scans (`[code] paths` in `kiln.toml`). A new project has no `[code]` yet,
so it starts with Markdown only. After you add `[code] paths`, run `kiln
init --fenix` again and it adds the code's languages:

```console
$ kiln init --fenix
.fenix/tools.json
kiln: Fenix now runs `kiln lsp` beside each language's own server for markdown, rust in this project (`:lsp-restart` to pick it up)
$ kiln init --fenix
kiln: .fenix/tools.json already runs `kiln lsp` for markdown, rust
```

`kiln init --fenix` merges into an existing `.fenix/tools.json` and never
replaces it: your tasks, your language servers and languages you added to
Kiln's entry by hand are kept. A project set up before additional servers
existed has Kiln as the Markdown server (`"markdown": {"executable":
"kiln", …}`); running `kiln init --fenix` moves that over and says
`moved `kiln lsp` from the Markdown server to an additional server`.
Run `:lsp-restart` in Fenix, or reopen the project, to pick it up. `kiln`
must be on your `PATH`.

In Fenix you then have, in item files, completion, hover, `gd` (definition), `gr` (references), rename with Fenix's multi-file preview, diagnostics, code actions and formatting.

In code files, beside the language's own server:

- **Completion** of IDs after a code reference's role word: type `// verifies: ` and Kiln offers the requirements, with their titles, leaving out those already on the line.
- **Hover** (`K`) on an ID shows what `kiln show` prints, after the language server's own hover.
- **`gd` and `gr`** on an ID go to the item and to everything that links to it.
- **Diagnostics** on code references as you type: `// implements: REQ-099` when REQ-099 doesn't exist (K102).

Rename and formatting in code files stay with the language's own server.

### What Fenix can't do yet

- **Actions that create or rename files.** Fenix's LSP client doesn't yet apply workspace edits that create or rename files. Fenix shows only code actions it can apply, so "New child …", "Create test that verifies …" and "Rename file to …" don't appear. Run the command instead, for example `kiln new test --title "…" --open`, then `kiln link TEST-002 verifies REQ-001`. Rename updates every reference but not the file name; `kiln check` then reports K105, and you rename the file yourself. (This is recorded in Kiln's integration notes as gap G7.)
- **Files changed outside the editor.** Fenix doesn't tell language servers about files created, deleted or changed elsewhere (`kiln new` in a terminal, a `git pull`). Kiln polls instead: item files and `kiln.toml` every 2 seconds, code files every 10 seconds. Changes made outside the editor show up within a few seconds. (Gap G8.)
- **Code files in older Fenix versions.** Before additional servers, Fenix ran one language server per file type, so Kiln served Markdown only. Update Fenix; an older one rejects the `"additional"` entry in `tools.json` as an unknown field.

## Setting it up in other editors

Any editor with an LSP client can run `kiln lsp` for Markdown files, and, where it allows several servers per file, for your code files too. The server needs three things:

- the command `kiln lsp` (or the full path to `kiln`, with the argument `lsp`);
- Markdown files (language id `markdown`);
- the project root as the workspace folder, which is the folder holding `kiln.toml`. Without it, the server uses the first file opened to find the project.

The snippets below are **examples to adapt**, written for the editors' current configuration styles. Check your editor's documentation if a key has moved. You can run `kiln lsp` alongside another Markdown server where the editor allows several servers per file.

### Visual Studio Code

VS Code has no built-in way to start an arbitrary language server; you need a generic LSP client extension from the marketplace. Configure it with:

| Setting | Value |
|---|---|
| command | `kiln` |
| arguments | `["lsp"]` |
| language / document selector | `markdown` |

The setting names differ between extensions; look up the ones your extension uses. Some extensions start the server in the workspace folder, which is what Kiln wants when you open the project root.

### Neovim

Neovim 0.11 and newer, in your `init.lua`:

```lua
vim.lsp.config('kiln', {
  cmd = { 'kiln', 'lsp' },
  filetypes = { 'markdown' },
  root_markers = { 'kiln.toml' },
})
vim.lsp.enable('kiln')
```

Neovim 0.10, with an autocommand:

```lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'markdown',
  callback = function(args)
    local root = vim.fs.root(args.buf, { 'kiln.toml' })
    if root then
      vim.lsp.start({ name = 'kiln', cmd = { 'kiln', 'lsp' }, root_dir = root })
    end
  end,
})
```

Both only start the server inside a Kiln project. Use `:checkhealth vim.lsp` (or `:LspInfo` with nvim-lspconfig) to see that it is attached.

### Helix

In `languages.toml` (your user config, or `.helix/languages.toml` in the project):

```toml
[language-server.kiln]
command = "kiln"
args = ["lsp"]

[[language]]
name = "markdown"
language-servers = ["kiln"]
```

To keep another Markdown server, list both: `language-servers = ["kiln", "marksman"]`. Helix looks for the workspace root by its own rules (a `.helix` folder or a version-control folder), which is usually the repository root; Kiln walks up from there or from the opened file to `kiln.toml`.

### Limitations in any editor

- The server never writes files. Your editor applies every change.
- Actions that create or rename a file work only if your editor declares support for file operations in workspace edits. Otherwise they are offered disabled, naming the command to run instead; whether your editor shows disabled actions depends on the editor.
- Files changed outside the editor are noticed within a few seconds (Kiln polls the disk), even if your editor sends no file notifications.
- The server writes no log of its own: stdout is the protocol, and its own messages go nowhere. If something looks wrong, run `kiln check` in a terminal to see the same findings.

## Opening items in Fenix

Two places open an item in Fenix at its title line:

- `kiln new TYPE --title "…" --open` creates the item and opens it.
- `Enter` on a row in the GUI ([The GUI](The-GUI)) opens that item, or that code location at its line.

Both try, in order:

1. the running Fenix, through its control channel;
2. starting Fenix as `fenix <path>:<line>`;
3. when Fenix isn't installed, the operating system's default editor for the file.

stderr says which happened. With `kiln new --open` the item is created either way. If you run more than one Fenix, `FENIX_HOME` picks the one Kiln talks to: the one started with the same `FENIX_HOME`.

## The Flint provider

Flint is a launcher: a hotkey opens a search box, and *providers* feed it items. `kiln provider` makes every open item of your project searchable in Flint, with actions on it.

### Declaring it

Add an entry to Flint's `settings.toml`:

```toml
[[providers]]
name = "kiln"
command = ["kiln", "provider"]
prefix = "k"
```

`prefix` is the scope keyword: typing `k ` in Flint searches only Kiln's items. If you choose another `name` or `prefix`, pass the same values to the provider, because it repeats its name to Flint and types the keyword in its drill-downs:

```toml
[[providers]]
name = "design"
command = ["kiln", "provider", "--name", "design", "--prefix", "d"]
prefix = "d"
```

To always show one project, whatever Fenix shows, add `"--project", "C:\\work\\flint"` (or a Linux path) to `command`.

### What appears

Every item that isn't closed appears as one entry:

```text
REQ-014 Flint appears within 50 ms of the hotkey
requirement · approved · under REQ-002
```

The first line is the ID and title; the second the type, status and parent. Tags and Jira keys are search keywords too, so `k perf` finds items tagged `perf`. Flint matches and ranks these entries itself, so ordinary typing costs no round trip to Kiln. Closed items are found through `k all`.

Very large projects: all items go to Flint in one message under Flint's 4 MiB limit. 2,000 items take about 1.6 MiB; past roughly 4,500 open items, the provider registers the first ones and logs that the rest are reachable through `k all`.

### Actions

`Enter` runs the first action. The others are in Flint's action menu.

| Action | Does |
|---|---|
| Open in Fenix | opens the item at its title line in Fenix (Flint disables it, with the reason, when Fenix lacks the capability) |
| Copy ID | copies `REQ-014` |
| Copy reference | copies `[[REQ-014]]` |
| Set status… | drills down to `k status REQ-014` |
| Show what depends on it | drills down to `k impact REQ-014` |
| Open in Confluence | offered when the project has a Confluence export: opens the item's page (or the page holding it) in the browser at `#REQ-014`. It uses the page cache `kiln export` keeps, else looks the page up by title with your token. When something is missing (no token, not published yet), Flint's footer says so |
| Open in Jira | opens `<[jira] url>/browse/<KEY>`, for items with a `jira` key |
| Start working on it | clocks in on the Fenix agenda task that has the item's Jira key; offered for items with a `jira` key. Fenix can't yet create an agenda task for Kiln, so without one the footer says `no agenda task for FLINT-88 (needs Fenix G1 to create one)`: create the task in Fenix first |
| Open in Kiln | opens Kiln's window on that item (`kilnw gui --project ROOT --item ID` on Windows, `kiln …` elsewhere); key `g` |

### Drill-downs

A drill-down is a query that starts with a verb after the scope keyword. The provider answers only these three; anything else is left to Flint's own matching.

| Query | Lists |
|---|---|
| `k status REQ-014` | the type's statuses, as `status REQ-014 → verified`, the current one marked. Choosing one edits the front matter as `kiln set` does (refusing a file changed on disk) and answers `REQ-014: approved → verified` |
| `k impact REQ-014` | everything that reaches the item, as `impact REQ-014 ← TEST-004 …`: items with their usual actions, and code locations that open in Fenix at the line |
| `k all text` | every item whose ID or title contains the text, closed ones included, as `all REQ-005 …` |

The results are titled to contain the query text, so they rank above the plain `REQ-014` entry that also matches.

### Which project it follows

In this order:

1. `--project DIR`, when given in `command`;
2. the project Fenix shows when you summon Flint (the file or project Fenix reports, if a `kiln.toml` is at or above it). The provider remembers it in Kiln's state folder (`state/flint-project`) for the next start;
3. the project it remembered last time;
4. the folder the provider started in.

One project is loaded at a time. When you switch Fenix to another Kiln project, the provider loads that one and replaces its items. Item files and `kiln.toml` are checked every 2 seconds, so edits reach Flint within about 2 seconds.

### Logs

The provider writes its messages to stderr, and Flint keeps them in its own logs folder as `logs/providers/<name>.log` (`logs/providers/kiln.log` with the default name). Look there when items don't appear or an action fails.

See also: [Command reference](Command-reference) · [The GUI](The-GUI) · [Checking](Checking) · [Configuration](Configuration)
