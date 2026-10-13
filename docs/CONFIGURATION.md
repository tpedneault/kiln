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
| `[render]` `link_bare_ids` | whether a bare `REQ-014` in prose is a reference | `true` |
| `[jira]` `url` | where `jira` keys point (read-only) | none |
| `[check]` | severity per rule: `K201 = "error"`, `K105 = "off"` | each rule's default (FORMAT.md, "Check rules") |
| `[confluence]`, `[[export]]` | exports (K2/K3; EXPORT.md, CONFLUENCE.md) | none |

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
| `FENIX_HOME` | which Fenix `kiln new --open` talks to (the one started with the same `FENIX_HOME`) |

Kiln keeps no settings or state of its own yet; its folders (and
`KILN_HOME`) arrive with the first thing that needs them (DESIGN.md,
"Kiln's own folders").
