# Configuration

What can be configured, where. The precise rules are in
[FORMAT.md](FORMAT.md), "`kiln.toml`"; this page is the overview.

## `kiln.toml`

The project's configuration, at its root and committed with it. `kiln
init` writes one with the default types. It never holds secrets: a key
named like a token or password is an error (K004).

| Section | Sets | Default |
|---|---|---|
| `format` | the format version; required | — (`1`) |
| `name` | the project's name in reports; required | — |
| `paths` | folders holding items | `["design"]` |
| `[code]` `paths`, `exclude`, `patterns` | where code references are scanned, globs left out, the regexes | no scanning; default pattern `implements:`/`verifies:` (every role whose `from` includes `code`) |
| `[repository]` `web` | a file's web URL, `{commit}`, `{path}`, `{line}` (used by exports from K2) | none |
| `[types.<name>]` | each type: `prefix`, `statuses`, `initial`, `done`, `closed`, `parents`, `title`, `plural`, `template` | the seven of `kiln init`: requirement, design, decision, milestone, task, test, risk |
| `[links.<role>]` | each link role: `from`, `to`, `inverse` | implements, verifies, part-of, supersedes, mitigates, depends-on |
| `[coverage]` `roles` | which roles count toward coverage | `["implements", "verifies"]` |
| `[requirements]` `levels`, `trace` | the requirement types, highest level first, and the role tracing each level to the one above (FORMAT.md, "Requirement levels") | one level, `requirement`; `trace = "refines"` |
| `[render]` `link_bare_ids` | whether a bare `REQ-014` in prose is a reference | `true` |
| `[jira]` `url` | where `jira` keys point, and the Jira the mirror reads (`kiln list --jira`, K207, the GUI); Server or Data Center | none |
| `[jira]` `project`, `parent`, `parent_type`, `subtask_type`, `types`, `[jira.status]` | `kiln jira sync`: the Jira project, the project issue (written by the first sync), the issue types, the Kiln types synced (`task`), Kiln status → Jira status name — INTEGRATION.md, "Jira" | none (no sync) |
| `[check]` | severity per rule: `K201 = "error"`, `K105 = "off"` | each rule's default (FORMAT.md, "Check rules") |
| `[[export]]` | export targets: `name`, `format` (markdown, html, docx, confluence, confluence-wiki), `layout`, `title`, `template`, `types`, `exclude_status`, `reports`, `numbering`, `item_style` (list, line, table, none), `link_to`, `path`, and `[export.markdown]` (`flavour`, `diagrams`), `[export.html]` (`theme`), `[export.docx]` (`reference`), `[export.confluence]` (`parent` or `page`, `space`, `labels`, `toc_levels`, `restrict_adopted`, `qualify_titles`, `children`) — EXPORT.md | none |
| `[confluence]` | the connection: `url`, `space`, `flavour`, `user`, `auth`, `labels`, `jira_macro`, `jira_server_id`, `on_drift`, `diagrams`, `max_page_kib`, … — CONFLUENCE.md | none |

Mistakes in `kiln.toml` are findings with the line they are on: `kiln
check` lists them. Other commands carry on with what is valid and say on
stderr how many problems there are.

To fail CI on approved requirements that nothing verifies (DESIGN.md,
goal 3):

```toml
[check]
K201 = "error"
```

## Environment

| Variable | Effect |
|---|---|
| `KILN_CONFLUENCE_TOKEN` | the Confluence token (a personal access token on Data Center, an API token on Cloud); never put it in a file. When unset, the token `kiln auth confluence` stored in the OS credential store is used |
| `KILN_CONFLUENCE_USER` | Cloud: the account e-mail the token belongs to (overrides `[confluence] user`) |
| `KILN_JIRA_TOKEN` | the Jira personal access token for the mirror; when unset, the token `kiln auth jira` stored is used |
| `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` | the proxy for Confluence calls, honouring `NO_PROXY` (CONFLUENCE.md, "The API, by flavour"). Jira calls don't use a proxy yet (INTEGRATION.md, G9) |
| `KILN_HOME` | puts Kiln's own folders (settings, state, logs) in one folder |
| `FENIX_HOME` | which Fenix `kiln new --open` talks to (the one started with the same `FENIX_HOME`) |

## Kiln's own folders

Settings in `%AppData%\kiln` / `~/.config/kiln`; state in
`%LocalAppData%\kiln\state` / `~/.local/state/kiln/state`, logs beside it;
all three under `KILN_HOME` when it is set. These live there: the
cache of rasterised diagrams (`state/diagrams`), which CI may cache
between runs, and one Confluence page cache per target
(`state/confluence/<space>-<project>-<export>.json`: page id → version and
`kiln` property), which saves a call per unchanged page and lets the Flint
provider open an item's page without asking the server;
`state/flint-project`, the project the Flint provider last followed Fenix
to; `state/projects.toml`, the projects the GUI's Home lists (each folder
and when it was last opened; in the state folder because the paths are
this machine's, so the list must not roam with the settings); and the
Jira mirror (`state/jira/<host>.json`: each linked ticket's
status and assignee, used for ten minutes). Any of them can be deleted at
any time.

## The GUI's settings

`settings.toml` in the settings folder holds the window's looks; every key
is optional, and an unknown key or a bad value is reported in the footer
and otherwise ignored (GUI.md, "Looks"):

```toml
theme = "Orbit Dark"   # one of Fenix's themes (as Flint has them)
font = "Cascadia Mono" # a monospace family; the system's monospace if absent
font_size = 15         # 8 to 40
```

When `kilnw` (Windows) can't start the window, it writes why to
`kilnw.log` in the temporary folder, since it has no console to say it.

## Credentials

Tokens are never in a file Kiln reads or writes. `kiln auth confluence`
stores one in the OS credential store (Windows Credential Manager; the
Secret Service on Linux, as KDE Wallet and GNOME Keyring provide) under
service `fenix-suite`, account `confluence:<url>`, the URL as configured
without a trailing slash: the same entry Flint reads, so storing it once
serves both. `KILN_CONFLUENCE_TOKEN` takes precedence when set.
`kiln auth jira` does the same for `[jira] url` as `jira:<url>`, the
entry Flint's Jira support reads; `KILN_JIRA_TOKEN` takes precedence.
