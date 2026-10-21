This page is the complete reference for `kiln.toml`, the file that
defines a Kiln project: its item types, statuses, link roles, where code
references are scanned and how strict `kiln check` is. It also covers the
environment variables Kiln reads, the folders Kiln keeps on your machine,
the GUI's own settings file and where tokens are stored. Read it when you
set up a project or adapt Kiln to your team's process. Terms such as
*item*, *type*, *link role* and *coverage* are explained in
[Concepts](Concepts); the item files themselves in
[Item files](Item-files).

## The file

`kiln.toml` sits at the root of the project, normally the repository root,
and is committed with it. Every `kiln` command finds the project by
walking up from the current folder to the nearest `kiln.toml`;
`--project DIR` starts the walk from `DIR` instead.

It is TOML. Two rules apply to the whole file:

- **An unknown key is an error (K003).** Typos must be loud: `[render]
  link_bare_id = true` (missing `s`) is reported, not silently ignored.
- **No secrets (K004).** The file is committed, so any key whose name
  contains `token`, `password`, `passwd`, `secret`, `api_key` or `apikey`
  is an error, wherever it is. Tokens go in the environment or the OS
  credential store (see [Credentials](#credentials)).

Mistakes are reported by `kiln check` with the line they are on. Kiln is
lenient where it can be: an invalid type, link role or pattern is reported
and left out, and the rest of the file still applies, so one typo doesn't
hide the others. Other commands carry on with what is valid and say on
stderr how many problems there are. Only a file that isn't valid TOML
(K001) or whose `format` this Kiln doesn't read stops everything.

```console
$ kiln check
kiln.toml:87:17: K002 error: `code.paths`: `../outside` must be relative and inside the project (no `..`)
kiln.toml:90:10: K002 error: prefix `REQ` is used by another type
kiln.toml:99:1: K003 error: unknown key `link_bare_id` in [render]
kiln.toml:103:1: K003 error: unknown key `api_token` in [jira]
kiln.toml:103:1: K004 error: `api_token` looks like a secret; kiln.toml is committed, so credentials go in the environment or the OS credential store
kiln.toml:106:8: K002 error: [check] K201: `fatal` isn't error, warning or off
…
```

## What `kiln init` writes

`kiln init --name Demo` in an empty folder writes this `kiln.toml` and a
`design/` folder with a `README.md`:

```toml
# Kiln project configuration: docs/FORMAT.md in Kiln's repository.
format = 1
name = "Demo"
paths = ["design"]

# Where source code is scanned for code references: a comment with
# `implements:` or `verifies:` and item IDs. Uncomment and list your
# source folders.
# [code]
# paths = ["src"]
# exclude = ["tests/fixtures"]

[types.requirement]
prefix = "REQ"
statuses = ["draft", "proposed", "approved", "implemented", "verified", "obsolete"]
initial = "draft"
done = ["verified"]
closed = ["obsolete"]
parents = ["requirement"]

[types.design]
prefix = "DES"
statuses = ["draft", "accepted", "obsolete"]
closed = ["obsolete"]
parents = ["design"]

[types.decision]
prefix = "ADR"
statuses = ["proposed", "accepted", "rejected", "superseded"]
closed = ["rejected", "superseded"]

[types.milestone]
prefix = "MS"
statuses = ["planned", "active", "done", "dropped"]
done = ["done"]
closed = ["dropped"]

[types.task]
prefix = "TASK"
statuses = ["todo", "doing", "blocked", "done", "dropped"]
done = ["done"]
closed = ["dropped"]
parents = ["task"]

[types.test]
prefix = "TEST"
statuses = ["draft", "ready", "passing", "failing", "obsolete"]
closed = ["obsolete"]

[types.risk]
prefix = "RISK"
statuses = ["open", "mitigated", "accepted", "closed"]
closed = ["closed"]

[links.implements]
from = ["design", "task", "code"]
to = ["requirement"]
inverse = "implemented by"

[links.verifies]
from = ["test", "code"]
to = ["requirement"]
inverse = "verified by"

[links.part-of]
from = ["task"]
to = ["milestone"]
inverse = "includes"

[links.supersedes]
from = ["decision"]
to = ["decision"]
inverse = "superseded by"

[links.mitigates]
from = ["requirement", "design", "task"]
to = ["risk"]
inverse = "mitigated by"

[links.depends-on]
from = ["task", "milestone"]
to = ["task", "milestone"]
inverse = "blocks"
```

`kiln init` never overwrites an existing `kiln.toml` (it stops with exit
2). `NAME` defaults to the folder's name. Sections it leaves out
(`[coverage]`, `[render]`, `[check]`) take their defaults, described
below.

## Top-level keys

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `format` | integer | yes | — | The file-format version. Always `1` today. |
| `name` | string | yes | — | The project's name, shown in reports, exports and Confluence. |
| `paths` | array of strings | no | `["design"]` | Folders holding item files. |

- A `format` newer than this Kiln reads is refused with "update Kiln"
  (K002), and nothing else in the file is read.
- `paths` are relative to the project root and must stay inside it: an
  absolute path or any `..` is K002. A listed folder that doesn't exist is
  K002 too. `kiln new` puts new items in the **first** folder listed.
- At least one `[types.*]` table is required.

## `[code]`: code references

A **code reference** is a line in your source code that links code to an
item, such as a comment `// implements: REQ-014` above a function or
`# verifies: REQ-014, REQ-015` above a test. It counts like a front-matter
link from the pseudo-type `code`: it can provide coverage, and `kiln show`
lists the location (`src/main.rs:12`) under "implemented by" or
"verified by".

```toml
[code]
paths = ["src", "tests"]
exclude = ["tests/fixtures"]
patterns = ['(?:implements|verifies):\s*(?P<ids>[A-Z][A-Z0-9]*-\d+(?:\s*,\s*[A-Z][A-Z0-9]*-\d+)*)']
```

| Key | Type | Default | Meaning |
|---|---|---|---|
| `paths` | array of strings | none: no scanning | Folders scanned for code references, relative, inside the project. |
| `exclude` | array of strings | none | gitignore-style globs left out, such as `"crates/*/tests/fixtures"`. |
| `patterns` | array of strings | the default pattern | Regular expressions that find code references. |

Without a `[code]` section (or without `paths`), no code is scanned. That
is what `kiln init` gives you; uncomment the section and list your source
folders.

How scanning works:

- Every file under `paths` is read, honouring `.gitignore`, minus the
  `exclude` globs. Binary files (a NUL byte in the first 8 KiB), files
  over 2 MiB and the item files under `paths` are skipped. A file that
  isn't UTF-8 is scanned with invalid bytes replaced.
- Kiln matches whole lines and doesn't parse languages, so it works for
  any language (Rust, C, Tcl, Python, VHDL…). A match inside a string
  literal counts too; the role word and colon make accidental matches
  rare.
- `exclude` is for files that contain code references on purpose, such
  as test fixtures that would otherwise point at items that don't exist.

### The default pattern

Without `patterns`, Kiln matches `<role>: ID[, ID…]` for every link role
whose `from` includes `code`. With the default roles that is
`implements` and `verifies`:

```rust
// implements: REQ-014
fn show_window(&mut self) { /* … */ }

#[test]
// verifies: REQ-014, REQ-015
fn hotkey_to_first_frame_is_under_budget() { /* … */ }
```

```python
# verifies: REQ-031
def test_rate_limit(): ...
```

The default pattern is case-sensitive: `Implements: REQ-014` is not
matched.

### Writing your own pattern

A pattern must have a named group **`ids`** holding one or more IDs
separated by commas. The **link role** is the word just before the colon
that precedes the `ids` group. That role must be declared and must list
`code` in its `from`; otherwise every match is error K108:

```text
src/main.rs:3:4: K108 error: `mitigates` isn't a link role code can use (implements, verifies)
```

A pattern without an `ids` group, or that isn't a valid regular
expression, is K002 and is left out. In TOML, write patterns as literal
strings in single quotes (`'…'`) so backslashes need no escaping.

## `[repository]`

```toml
[repository]
web = "https://github.com/acme/app/blob/{commit}/{path}#L{line}"
```

| Key | Meaning |
|---|---|
| `web` | How a file in the repository is linked on the web. `{commit}`, `{path}` and `{line}` are filled in. |

Exports use it to turn code locations (`src/main.rs:12`) into links. For
GitLab: `"https://gitlab.example.com/team/app/-/blob/{commit}/{path}#L{line}"`.
Without it, code locations are plain text. See
[Templates and exports](Templates-and-exports).

## `[types.<name>]`: item types

Each `[types.<name>]` table declares one kind of item. `<name>` is a
lowercase ASCII word, optionally with hyphens (`requirement`,
`test-case`); `code` is reserved.

```toml
[types.requirement]
prefix = "REQ"
title = "Requirement"
plural = "Requirements"
statuses = ["draft", "proposed", "approved", "implemented", "verified", "obsolete"]
initial = "draft"
done = ["verified"]
closed = ["obsolete"]
parents = ["requirement"]
template = "design/templates/requirement.md"
```

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `prefix` | string | yes | — | The ID prefix: uppercase letters and digits, starting with a letter, unique across types. |
| `statuses` | array of strings | yes | — | Every status an item of this type may have, in workflow order. Lowercase words with `-`. |
| `title` | string | no | the name, capitalised (`test-case` → `Test Case`) | Display name. |
| `plural` | string | no | `title` plus `s` | Display name for many; also names the folder `kiln new` uses. |
| `initial` | string | no | the first status | The status `kiln new` writes. |
| `done` | array of strings | no | none | Statuses that count as finished. |
| `closed` | array of strings | no | none | Statuses that take an item out of play. |
| `parents` | array of strings | no | none | Types allowed as this type's `parent`. |
| `template` | string | no | none | A file whose content is the body of a new item. |

What each key affects:

| Key | Affects |
|---|---|
| `prefix` | IDs (`kiln new` allocates the next number for this prefix); K106 when an item's ID prefix doesn't match its type; which bare IDs in prose are references. |
| `statuses` | K107 for any other status; the GUI's Board page (one group per status, in this order); completion of `status = "` in an editor. |
| `title`, `plural` | Headings in exports, `kiln trace` and the GUI. `plural` in lowercase with hyphens is the folder of new items (`design/requirements/`). |
| `initial` | What `kiln new` writes. For the type named `requirement`, also where coverage rules start: K201 and K202 skip items still in the initial status. |
| `done` | Milestone progress (done ÷ open items); the Board; K206 (a milestone that is done isn't overdue); K207 (only types with `done` statuses are compared with Jira). |
| `closed` | Closed items are left out of coverage and progress; a closed item provides no coverage; K203 warns when an open item links to a closed one; K201/K202 skip closed requirements; exports leave closed items out by default; the Board and Tree hide them until you press `a`; the Flint provider leaves them out. |
| `parents` | K108 when an item's `parent` is another type; `kiln new --parent` refuses a disallowed parent. Children count toward their parent's coverage. |
| `template` | The body `kiln new` writes (see [Item files](Item-files#templates-for-new-items)). Relative to the project root; must stay inside the project (K002). |

`initial` must be one of `statuses`, and every entry of `done` and
`closed` must be too (else K002). A type missing `prefix` or `statuses`,
or with a prefix another type uses, is reported (K002) and left out; its
items then fail K104 ("type isn't declared").

## `[links.<role>]`: link roles

Each `[links.<role>]` table declares a kind of link between items.

```toml
[links.verifies]
from = ["test", "code"]
to = ["requirement"]
inverse = "verified by"
```

| Key | Type | Required | Meaning |
|---|---|---|---|
| `from` | array of strings | yes | Types that may write this link in their `[links]`. May include `code`. |
| `to` | array of strings | yes | Types the link may point at. Never `code`. |
| `inverse` | string | yes | How the link reads from the target's side ("verified by"). Never written in a file. |

- A link is written **only on its `from` side**. The target shows it
  under `inverse`, computed by Kiln.
- `code` is a pseudo-type meaning "a code reference". Put it in `from` to
  let source code use this role (see [`[code]`](#code-code-references)).
  `code` in `to` is K002.
- Naming a type that doesn't exist in `from` or `to`, or leaving out a
  key, is K002 and the role is left out.
- If someone writes the inverse as a key, Kiln recognises it: the
  inverse with spaces turned into hyphens (`verified-by`) is warning
  K109, telling them which item should hold the link.

Some features look for specific role names: milestone progress follows
`part-of` and `depends-on`, rule K204 looks for `supersedes`, K201 and
K202 look for `verifies` and `implements`, and the GUI's Roadmap and
Decisions pages use `part-of` and `supersedes`. Renaming those roles
switches the features off. See [What to watch for](#what-to-watch-for).

## `[coverage]`

```toml
[coverage]
roles = ["implements", "verifies"]
```

| Key | Default | Meaning |
|---|---|---|
| `roles` | `["implements", "verifies"]` (those of them that are declared) | The roles that count toward coverage. |

An item is **covered** for a role when at least one non-closed item or a
code reference links to it with that role, or when all its non-closed
children are covered for it. `kiln show`, `kiln trace` and the
traceability report in exports show coverage for every role in `roles`
whose `to` includes the item's type:

```console
$ kiln show HAZ-001
HAZ-001 Battery overheats while charging
hazard · identified · design/hazards/HAZ-001-battery-overheats-while-charging.md:2
controlled by  REQ-001 The app starts within two seconds
coverage       controls: yes
```

Rules K201 and K202 don't read `roles`: they always check `verifies`
and `implements` on the type named `requirement`.

## `[render]`

```toml
[render]
link_bare_ids = true
```

| Key | Default | Meaning |
|---|---|---|
| `link_bare_ids` | `true` | Whether a bare `REQ-014` in prose is a reference, like `[[REQ-014]]`. |

With `true`, a bare ID with a declared prefix in prose must exist (else
K102). Put foreign IDs that share one of your prefixes in code spans.
Details are on [Item files](Item-files#bare-ids-in-prose).

## `[jira]`

```toml
[jira]
url = "https://jira.example.com"
```

| Key | Meaning |
|---|---|
| `url` | Where `jira = "KEY-1"` keys point, and the Jira that `kiln list --jira`, the GUI and rule K207 read. |

Read-only. Jira Server and Data Center only; a Jira Cloud URL is refused
when Kiln tries to read it. The token never goes here: set
`KILN_JIRA_TOKEN` or run `kiln auth jira`. See [Jira](Jira).

## `[check]`

```toml
[check]
K201 = "error"   # an approved requirement nobody verifies fails CI
K105 = "off"     # we name files our own way
```

Each key is a rule code, each value `"error"`, `"warning"` or `"off"`.
An unknown rule code or another value is K002. K001 can't be turned off.
Errors make `kiln check` exit 1; warnings are listed but don't fail.
Every rule and a recommended set of severities is on
[Checking](Checking).

## `[confluence]` and `[[export]]`

These two configure exports. In short:

```toml
[confluence]                        # the connection, shared by every Confluence target
url = "https://confluence.example.com"
space = "APP"

[[export]]                          # one export target; repeat for more
name = "design-md"
format = "markdown"                 # markdown, html, docx or confluence
path = "docs/generated/DESIGN.md"
template = "design/templates/design-document.md"
```

- `[[export]]` keys: `name`, `format`, `layout`, `title`, `template`,
  `types`, `exclude_status`, `reports`, `numbering`, `item_style`,
  `link_to`, `path`, and per-format tables `[export.markdown]`
  (`flavour`, `diagrams`), `[export.html]` (`theme`), `[export.docx]`
  (`reference`) and `[export.confluence]` (`parent` or `page`, `space`,
  `labels`, `toc_levels`, `restrict_adopted`, `qualify_titles`,
  `children`). All are described on
  [Templates and exports](Templates-and-exports).
- `[confluence]` keys: `url`, `space`, `flavour`, `user`, `auth`,
  `labels`, `jira_macro`, `jira_server_id`, `restrict_edits`, `on_drift`,
  `concurrency`, `diagrams`, `max_page_kib`. All are described on
  [Publishing to Confluence](Publishing-to-Confluence).

A bad value in `[confluence]` is K002. An `[[export]]` that can't work
(unknown format, a file format without `path`, a Confluence target without
`[confluence] url` and `space`…) is K005 and is left out, so the other
targets still export. A template that doesn't exist or has a bad
directive is K112.

## Customising

The defaults are a starting point. Change them in `kiln.toml`, then run
`kiln check` to see what your change affects.

### Adding a type

A safety project might add hazards; a systems project interfaces:

```toml
[types.hazard]
prefix = "HAZ"
statuses = ["identified", "analysed", "controlled", "accepted", "retired"]
initial = "identified"
done = ["controlled", "accepted"]
closed = ["retired"]

[types.interface]
prefix = "IF"
title = "Interface"
plural = "Interfaces"
statuses = ["draft", "agreed", "frozen", "obsolete"]
closed = ["obsolete"]
```

A type is useful once something can link to it, so add or extend roles
too (next section). Then:

```console
$ kiln new hazard --title "Battery overheats while charging"
design/hazards/HAZ-001-battery-overheats-while-charging.md
$ kiln new interface --title "Plug-in API"
design/interfaces/IF-001-plug-in-api.md
```

### Adding a link role, or extending one

A new role, so requirements and designs can control hazards:

```toml
[links.controls]
from = ["requirement", "design"]
to = ["hazard"]
inverse = "controlled by"
```

To let tests verify interfaces as well as requirements, **edit the
existing table** rather than adding a second one; TOML refuses a table
defined twice (K001, and nothing else is read):

```toml
[links.verifies]
from = ["test", "code"]
to = ["requirement", "interface"]
inverse = "verified by"
```

To see coverage of hazards and interfaces in `kiln show`, `kiln trace`
and exports, list the roles in `[coverage]`:

```toml
[coverage]
roles = ["implements", "verifies", "controls"]
```

```console
$ kiln link REQ-001 controls HAZ-001
REQ-001 controls HAZ-001
$ kiln trace --type hazard
Hazards coverage at 34d54e60ca72

HAZ-001  Battery overheats while charging  (identified)
         controlled by: REQ-001

1 hazards; covered: controls 1/1
```

### Renaming or adding statuses

Edit `statuses`, and keep `initial`, `done` and `closed` in step:

```toml
[types.task]
prefix = "TASK"
statuses = ["backlog", "in-progress", "in-review", "done", "wont-do"]
initial = "backlog"
done = ["done"]
closed = ["wont-do"]
parents = ["task"]
```

Existing items with an old status now fail K107 ("status `doing` isn't
one of task's: …"). Fix them with `kiln set TASK-001 status=in-progress`,
or a search-and-replace on `status = "doing"`. The GUI's Board groups items
under one heading per status, in the order you list them.

### What to watch for

- **Some rules and pages look for specific names.** They are what the
  feature is about; a project with other names simply doesn't trigger
  them:

  | Feature | Looks for |
  |---|---|
  | K201, K202 (coverage of requirements) | type `requirement`, roles `verifies` and `implements` |
  | K204 (superseded decision) | type `decision`, status `superseded`, role `supersedes` |
  | K205 (verified without a passing source) | status `verified` (any type), a source of type `test` with status `passing` |
  | K206 (overdue milestone) | type `milestone` |
  | Milestone progress, the Roadmap page | type `milestone`, roles `part-of` and `depends-on` |
  | Board page | type `task`, else every type with `done` statuses except `milestone` |
  | Decisions page | type `decision`, role `supersedes` |
  | `kiln trace` with no `--type` | type `requirement` |

  So if you rename `requirement` to `req`, or `verified` to
  `accepted`, those checks stop. Keep the default names where you want
  the feature.
- **A changed prefix doesn't rename existing items.** Their IDs keep the
  old prefix and get warning K106 ("RISK-001 has prefix RISK, but risk
  items use RSK"), bare mentions with the old prefix stop being
  references, and new items start again at `RSK-001`. `kiln renumber`
  can't help (it keeps the prefix), so choose prefixes before you start.
- **Removing a type or role** turns every item or link that uses it into
  findings (K104, K108). Mark items obsolete first, or move them.
- **Prefixes and bare IDs.** Every prefix makes bare `PREFIX-NNN` text in
  prose a reference. A short prefix such as `IF` or `ID` can collide with
  ordinary text or a supplier's numbering; check with `kiln check` after
  adding one.
- **Defining a table twice** (`[links.verifies]` again at the bottom) is
  invalid TOML: K001, and nothing is read until you merge them.

## Environment variables

| Variable | Effect |
|---|---|
| `KILN_CONFLUENCE_TOKEN` | The Confluence token: a personal access token on Data Center, an API token on Cloud. Takes precedence over a token stored by `kiln auth confluence`. Never put it in a file. |
| `KILN_CONFLUENCE_USER` | Confluence Cloud only: the account e-mail the token belongs to. Overrides `[confluence] user`. |
| `KILN_JIRA_TOKEN` | The Jira personal access token for reading tickets. Takes precedence over a token stored by `kiln auth jira`. |
| `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` | The proxy for Confluence calls (lowercase names work too). `HTTPS_PROXY` for `https://` URLs, `HTTP_PROXY` for `http://`, else `ALL_PROXY`; no proxy for hosts listed in `NO_PROXY` (names, `.domain` suffixes, IPv4 ranges such as `10.0.0.0/8`, or `*`). **Jira calls don't use a proxy yet**, so a Jira reachable only through a proxy can't be read. |
| `KILN_HOME` | Puts all of Kiln's own folders (settings, state, logs) in this one folder. |
| `FENIX_HOME` | Which running Fenix editor `kiln new --open` talks to: the one started with the same `FENIX_HOME`. |
| `KILN_BLESS` | Only for developing Kiln itself: `KILN_BLESS=1 cargo test` rewrites the golden files. Not read by the `kiln` program. |

## Kiln's own folders

Kiln keeps some files outside the project, per user and machine:

| Folder | Windows | Linux | With `KILN_HOME` |
|---|---|---|---|
| settings | `%AppData%\kiln` | `~/.config/kiln` | `$KILN_HOME` |
| state | `%LocalAppData%\kiln\state` | `~/.local/state/kiln/state` | `$KILN_HOME/state` |
| logs | `%LocalAppData%\kiln\logs` | `~/.local/state/kiln/logs` | `$KILN_HOME/logs` |

The state folder holds caches, all safe to delete at any time:

| Path | What |
|---|---|
| `state/diagrams/` | Rendered Mermaid diagrams, by a hash of their source. CI can cache this folder between runs. |
| `state/confluence/<space>-<project>-<export>.json` | One page cache per Confluence target (page id, version), which saves an API call per unchanged page and lets the Flint provider open an item's page offline. |
| `state/flint-project` | The project the Flint provider last followed Fenix to. |
| `state/jira/<host>.json` | The Jira mirror: each linked ticket's status and assignee, reused for ten minutes. |
| `state/projects.toml` | The projects the GUI's Home lists, with when each was last opened. Deleting it empties Home; opening a project adds it back. |

## The GUI's settings

`settings.toml` in the settings folder sets how `kiln gui` looks. Every
key is optional:

```toml
theme = "Orbit Dark"    # one of Fenix's themes; default Orbit Dark
font = "Cascadia Mono"  # a monospace family; default: the one Fenix would pick
font_size = 15          # points, 8 to 40
```

An unknown key, theme or font, or a size out of range, is reported in the
GUI's footer and otherwise ignored; it never stops the window. On
Windows, when `kilnw` can't open the window, it writes why to `kilnw.log`
in the temporary folder. See [The GUI](The-GUI).

## Credentials

Tokens are never in a file Kiln reads or writes, never in logs, plan
output or error messages. There are two ways to provide one:

1. **An environment variable**: `KILN_CONFLUENCE_TOKEN`, `KILN_JIRA_TOKEN`.
   This is what CI uses, as a masked secret. It always takes precedence.
2. **The OS credential store**, filled by `kiln auth`:

   ```console
   $ kiln auth confluence
   $ kiln auth jira
   ```

   Each asks for the token without echoing it, checks it against the
   server, and stores it in Windows Credential Manager, or on Linux in the
   Secret Service (as KDE Wallet and GNOME Keyring provide). The entry is
   service `fenix-suite`, account `confluence:<url>` or `jira:<url>`, the
   URL as in `kiln.toml` without a trailing slash. Flint reads the same
   entries, so storing a token once serves both. `--forget` removes it.

On Confluence Cloud, set `[confluence] user` (or `KILN_CONFLUENCE_USER`)
to your account e-mail first. A headless Linux machine without a Secret
Service (a CI container) can't use the store; use the environment
variables there.

See also: [Item files](Item-files), [Checking](Checking),
[Templates and exports](Templates-and-exports),
[Publishing to Confluence](Publishing-to-Confluence)
