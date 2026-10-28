`kiln check` reads your whole project (`kiln.toml`, every item file and
every code reference) and reports what is wrong or worth a look: broken
references, statuses a type doesn't have, requirements nothing verifies,
milestones past their date. This page explains how to run it, what its
output looks like in each format, how to make rules stricter or quieter,
and then every rule, K001 to K207, with an example and the fix. Read it
when you set up CI or when a finding puzzles you. Terms such as *item*,
*type*, *link role* and *coverage* are explained in [Concepts](Concepts).

## Running it

```console
$ kiln check
kiln: no findings
```

`kiln check` finds the project by walking up from the current folder to
the nearest `kiln.toml` (or from `--project DIR`). It prints findings on
stdout, sorted by file, line, column and rule, and a one-line summary on
stderr. It is fast enough to run on every save: the target is under two
seconds for a 2,000-item project in a large repository.

Here it is on a project with a dozen deliberate mistakes:

```console
$ kiln check
design/decisions/ADR-002-yaml.md:5:10: K204 warning: ADR-002 is superseded, but no decision `supersedes` it
design/designs/DES-002-flow.md:10:3: K111 error: diagram: invalid flowchart edge syntax -- A -->
design/milestones/MS-002-beta.md:2:7: K206 warning: MS-002 was due 2026-09-01 and is active
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:10: K102 error: reference REQ-099 doesn't exist
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:19: K102 error: reference `REQ-7` isn't how IDs are written: REQ-007
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:28: K102 error: reference `REQ-0002` isn't how IDs are written: REQ-002
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:42: K102 error: reference `settings` isn't an ID: an ID is PREFIX-NUMBER, the prefix uppercase letters and digits (REQ-014)
design/requirements/REQ-003-settings-survive-an-upgrade.md:17:12: K110 error: `img/upgrade.png` doesn't exist (design/requirements/img/upgrade.png)
design/requirements/REQ-006-a.md:6:11: K103 error: parent cycle: REQ-006 → REQ-007 → REQ-006
design/requirements/REQ-007-b.md:6:11: K103 error: parent cycle: REQ-007 → REQ-006 → REQ-007
design/requirements/REQ-009-logs.md:2:7: K201 warning: REQ-009 is verified but nothing verifies it
design/requirements/REQ-009-logs.md:2:7: K202 warning: REQ-009 is verified but nothing implements it
design/requirements/REQ-009-logs.md:5:10: K205 warning: REQ-009 is verified, but no passing test or code reference verifies it
design/risks/RISK-002-broken.md:2:15: K100 error: front matter isn't valid TOML: invalid basic string
design/tasks/TASK-002-write-installer.md:5:10: K107 error: status `in-progress` isn't one of task's: todo, doing, blocked, done, dropped
design/tasks/TASK-002-write-installer.md:6:9: K104 error: tag `Release` must be lowercase
design/tasks/TASK-002-write-installer.md:7:1: K104 error: unknown key `priority` in front matter
design/tasks/TASK-002-write-installer.md:10:16: K108 error: `implements` can't point at a decision (ADR-001; to: requirement)
design/tasks/TASK-002-write-installer.md:11:1: K109 warning: `verified-by` is how `verifies` reads from the other side; write `verifies = ["TASK-002"]` in the other item
design/tasks/TASK-003-old-budget.md:8:16: K203 warning: TASK-003 links to REQ-008, which is obsolete
design/tests/upgrade-test.md:2:7: K105 warning: the file name doesn't start with REQ-005
design/tests/upgrade-test.md:2:7: K106 warning: REQ-005 has prefix REQ, but test items use TEST
kiln: 13 errors, 9 warnings
```

Each line is `path:line:column: CODE severity: message`. Paths are
relative to the project root with `/` separators, on Windows too. The
message names what is wrong and, where it helps, the fix.

## Exit codes

| Exit code | When |
|---|---|
| 0 | No finding has severity `error`. Warnings may have been printed. |
| 1 | At least one finding is an `error`. This includes a `kiln.toml` Kiln can't read at all (K001, or a `format` newer than this Kiln), which is reported as a finding so CI shows it. |
| 2 | Usage error: no `kiln.toml` in this folder or any above it, an unknown option or format. |

Warnings never change the exit code. To make CI fail on something that
is a warning by default, raise it to `error` in `[check]` (see
[Severities](#severities-and-check)).

```console
$ kiln check --format xml
kiln: invalid value 'xml' for '--format <FORMAT>' (see `kiln --help`)
```

## Output formats

`--format` picks how findings are printed. The summary always goes to
stderr, so stdout holds only the findings and can be redirected to a
file.

| Format | For |
|---|---|
| `text` (default) | People and editors: `path:line:col: K102 error: …` |
| `json` | Scripts: an array of objects with `code`, `severity`, `path`, `line`, `column`, `message` |
| `gitlab` | A GitLab Code Quality report, so findings show on merge requests |
| `github` | GitHub Actions workflow commands, so findings annotate the pull request diff |

The examples below are from a project with one broken reference and one
requirement nothing covers.

`text`:

```console
$ kiln check
design/decisions/ADR-001-use-toml-for-settings.md:21:5: K102 error: reference REQ-099 doesn't exist
design/requirements/REQ-004-logs.md:2:7: K201 warning: REQ-004 is approved but nothing verifies it
design/requirements/REQ-004-logs.md:2:7: K202 warning: REQ-004 is approved but nothing implements it
kiln: 1 error, 2 warnings
```

`json`:

```console
$ kiln check --format json
[
  {
    "code": "K102",
    "column": 5,
    "line": 21,
    "message": "reference REQ-099 doesn't exist",
    "path": "design/decisions/ADR-001-use-toml-for-settings.md",
    "severity": "error"
  },
  {
    "code": "K201",
    "column": 7,
    "line": 2,
    "message": "REQ-004 is approved but nothing verifies it",
    "path": "design/requirements/REQ-004-logs.md",
    "severity": "warning"
  },
  …
]
kiln: 1 error, 2 warnings
```

`gitlab` (errors become `major`, warnings `minor`; the fingerprint lets
GitLab track a finding across pipelines):

```console
$ kiln check --format gitlab
[
  {
    "check_name": "K102",
    "description": "K102: reference REQ-099 doesn't exist",
    "fingerprint": "aed4e7856e6a2267",
    "location": {
      "lines": {
        "begin": 21
      },
      "path": "design/decisions/ADR-001-use-toml-for-settings.md"
    },
    "severity": "major"
  },
  {
    "check_name": "K201",
    "description": "K201: REQ-004 is approved but nothing verifies it",
    "fingerprint": "d951969b04dc1480",
    …
    "severity": "minor"
  },
  …
]
kiln: 1 error, 2 warnings
```

`github`:

```console
$ kiln check --format github
::error file=design/decisions/ADR-001-use-toml-for-settings.md,line=21,col=5,title=K102::reference REQ-099 doesn't exist
::warning file=design/requirements/REQ-004-logs.md,line=2,col=7,title=K201::REQ-004 is approved but nothing verifies it
::warning file=design/requirements/REQ-004-logs.md,line=2,col=7,title=K202::REQ-004 is approved but nothing implements it
kiln: 1 error, 2 warnings
```

The exit code is the same in every format. Ready-made CI jobs are on
[Continuous integration](Continuous-integration).

## Severities and `[check]`

Every rule has a default severity:

- **error**: something is broken. `kiln check` exits 1.
- **warning**: worth a look; listed, but `kiln check` exits 0.
- **off**: not checked.

Change any of them in `kiln.toml`:

```toml
[check]
K201 = "error"   # an approved requirement nothing verifies fails CI
K105 = "off"     # we don't start file names with the ID
K207 = "warning" # compare done tasks with their Jira tickets
```

Keys are rule codes; values are `"error"`, `"warning"` or `"off"`. An
unknown code or value is itself an error (K002). K001 can't be turned
off, since nothing else can be read without the file.

The same findings appear in your editor through `kiln lsp` (after each
edit) and in exports with the `findings` report, at the same severities.

## Every rule

| Code | Default | Rule |
|---|---|---|
| K001 | error | `kiln.toml` can't be parsed |
| K002 | error | `kiln.toml` is invalid |
| K003 | error | unknown key in `kiln.toml` |
| K004 | error | something that looks like a secret in `kiln.toml` |
| K005 | error | an `[[export]]` target is invalid |
| K100 | error | front matter can't be parsed |
| K101 | error | duplicate ID |
| K102 | error | reference to an ID that doesn't exist, or that isn't written as an ID |
| K103 | error | `parent` cycle |
| K104 | error | bad front matter: unknown key, missing key, wrong value type, undeclared type, misspelt ID, bad title or tag |
| K105 | warning | file name doesn't start with the ID |
| K106 | warning | ID prefix doesn't match the type |
| K107 | error | status not allowed for the type |
| K108 | error | link role or parent not allowed |
| K109 | warning | a link written on its inverse side |
| K110 | error | relative link or image to a file that doesn't exist |
| K111 | error | a Mermaid diagram fails to parse |
| K112 | error | an export template is invalid |
| K113 | warning | the agent guide's generated section is out of date with `kiln.toml`, or broken |
| K201 | warning | a requirement past its initial status with no `verifies` coverage |
| K202 | warning | a requirement past its initial status with no `implements` coverage |
| K203 | warning | an open item links to a closed one |
| K204 | warning | a superseded decision that nothing supersedes |
| K205 | warning | status `verified` with no passing `verifies` source |
| K206 | warning | a milestone past its due date, neither done nor closed |
| K207 | off | a done item whose Jira ticket isn't resolved, or the reverse |
| K208 | warning | a requirement below the top level that traces to nothing above it and isn't `derived` |
| K209 | warning | a trace link that doesn't point at the level directly above |
| K210 | warning | a requirement above the lowest level that nothing below refines |
| K211 | warning | a done milestone with open work in it |
| K212 | warning | a done item that depends on one that isn't |
| K213 | warning | open work in no milestone |
| K214 | warning | a requirement implemented or verified while work implementing it is open |

## Configuration rules (K001–K005)

These are about `kiln.toml` itself. They are reported on `kiln.toml` with
the line of the offending key. See [Configuration](Configuration) for
every key.

### K001

**Triggers:** `kiln.toml` isn't valid TOML: an unclosed string, a table
defined twice, a missing `=`.

```text
kiln.toml:3:13: K001 error: kiln.toml isn't valid TOML: invalid basic string
```

**Fix:** correct the syntax at the line and column given. Nothing else in
the project is checked until you do, and other commands (`kiln list`…)
refuse to run (exit 2). A common cause is adding a second
`[links.verifies]` table instead of editing the existing one.

### K002

**Triggers:** the file is TOML but its content is wrong: `format` or
`name` missing, a `format` newer than this Kiln, a path that is absolute
or uses `..`, a folder in `paths` that doesn't exist, a type without
`prefix` or `statuses`, a prefix used twice, an `initial`, `done` or
`closed` status not in `statuses`, a `parents`, `from` or `to` naming a
type that doesn't exist, `code` in `to`, a role missing `from`, `to` or
`inverse`, a code pattern that isn't a valid regex or has no `ids` group,
an unknown rule or severity in `[check]`, a bad value in `[confluence]`.

```text
kiln.toml:2:10: K002 error: format 2 is newer than this Kiln reads (1); update Kiln
kiln.toml:1:1: K002 error: paths: `specs` doesn't exist
kiln.toml:87:17: K002 error: `code.paths`: `../outside` must be relative and inside the project (no `..`)
kiln.toml:90:10: K002 error: prefix `REQ` is used by another type
kiln.toml:93:8: K002 error: [links.covers] needs `from`, `to` and `inverse`, naming types that exist
kiln.toml:95:7: K002 error: to: `interface` isn't a type
kiln.toml:106:8: K002 error: [check] K201: `fatal` isn't error, warning or off
kiln.toml:107:1: K002 error: [check]: `K999` isn't a check rule
```

**Fix:** as the message says. An invalid type or role is left out while
the rest of the file applies, so you may also see follow-on findings in
items that use it (K104, K108); they go away with the fix.

### K003

**Triggers:** a key Kiln doesn't know, anywhere in `kiln.toml`.

```text
kiln.toml:99:1: K003 error: unknown key `link_bare_id` in [render]
kiln.toml:86:1: K003 error: unknown key `paths_extra` in [links.depends-on]
```

**Fix:** correct the spelling or remove the key. The second example is a
classic TOML trap: a top-level key written at the bottom of the file
belongs to the last table above it. Put top-level keys (`format`,
`name`, `paths`) before the first `[table]`.

### K004

**Triggers:** a key whose name contains `token`, `password`, `passwd`,
`secret`, `api_key` or `apikey`, at any level.

```text
kiln.toml:103:1: K004 error: `api_token` looks like a secret; kiln.toml is committed, so credentials go in the environment or the OS credential store
```

**Fix:** remove the key, and if a real token was committed, revoke it.
Provide tokens through `KILN_CONFLUENCE_TOKEN` or `KILN_JIRA_TOKEN`, or
store them with `kiln auth confluence` / `kiln auth jira` (see
[Configuration](Configuration#credentials)).

### K005

**Triggers:** an `[[export]]` target that can't work: unknown `format` or
`layout`, two targets with the same `name`, `page =` with a layout other
than `document`, a file format (markdown, html, docx) without `path`, a
Confluence target when `[confluence]` lacks `url` or `space`, a `link_to`
naming no target, a `qualify_titles` other than `taken` or `always`, a
`children` other than `macro` or `list`.

```text
kiln.toml:111:10: K005 error: format `pdf` isn't confluence, markdown, html or docx
kiln.toml:89:8: K005 error: export `md` writes markdown files and needs a `path`
kiln.toml:93:8: K005 error: Confluence target `spec` needs [confluence] url and space
kiln.toml:96:9: K005 error: [export.confluence] page = … writes one existing page, so it needs layout = "document"
kiln.toml:98:18: K005 error: `qualify_titles` `sometimes` isn't taken or always
```

**Fix:** complete or correct the target (see
[Templates and exports](Templates-and-exports)). An invalid target is left
out; the others still export.

## File rules (K100–K113)

These are about item files, code references and export templates.

### K100

**Triggers:** a file starts with `+++` but has no closing `+++` line, or
the front matter between them isn't valid TOML.

```text
design/risks/RISK-002-broken.md:2:15: K100 error: front matter isn't valid TOML: invalid basic string
```

**Fix:** correct the TOML. Typical causes: a missing closing quote, a
YAML habit (`status: draft` instead of `status = "draft"`), an unquoted
string. The item isn't loaded until it parses, so references to it may
also fail K102 meanwhile.

### K101

**Triggers:** two or more item files have the same `id`. Usually two
branches each ran `kiln new` and were merged. Reported on every copy.

```text
design/requirements/REQ-003-reuse-test.md:2:7: K101 error: REQ-003 is also the ID of design/requirements/REQ-003-settings-survive-an-upgrade.md; `kiln renumber REQ-003 --to REQ-004 --file design/requirements/REQ-003-reuse-test.md` moves this one
```

**Fix:** run the `kiln renumber` the message suggests on the copy that
should move. Kiln can't know which item existing references meant, so it
lists them on stderr for you to check by hand. See
[Item files](Item-files#duplicates-after-a-merge-and-kiln-renumber).

### K102

**Triggers:** a reference names an item that doesn't exist, or isn't
spelt as an ID. Reported for `parent`, `[links]`, `[[…]]` and bare IDs in
the body, and code references.

```text
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:10: K102 error: reference REQ-099 doesn't exist
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:19: K102 error: reference `REQ-7` isn't how IDs are written: REQ-007
design/requirements/REQ-003-settings-survive-an-upgrade.md:16:42: K102 error: reference `settings` isn't an ID: an ID is PREFIX-NUMBER, the prefix uppercase letters and digits (REQ-014)
src/main.rs:4:14: K102 error: verifies REQ-042 doesn't exist
```

**Fix:** correct the ID, create the missing item, or remove the
reference. Two common surprises:

- A bare ID in prose counts as a reference when its prefix is one of
  yours (with `[render] link_bare_ids = true`, the default). If you are
  writing about someone else's numbering that happens to share a prefix,
  put it in a code span: `` `TEST-4410` ``.
- `[[…]]` is only for item references. `[[settings]]` is an error.

A code reference to a missing item in a test fixture can be silenced
with `[code] exclude`.

### K103

**Triggers:** following `parent` from an item leads back to it.

```text
design/requirements/REQ-006-a.md:6:11: K103 error: parent cycle: REQ-006 → REQ-007 → REQ-006
design/requirements/REQ-007-b.md:6:11: K103 error: parent cycle: REQ-007 → REQ-006 → REQ-007
```

**Fix:** remove or change one `parent` in the cycle, for example
`kiln set REQ-006 parent=` (an empty value removes it).

### K104

**Triggers:** front matter that parses but is wrong: an unknown key, a
missing `id`, `type`, `title` or `status`, a value of the wrong type
(`due = "2026-01-01"` in quotes, `extra` that isn't a table), a `type`
that `kiln.toml` doesn't declare, an `id` not spelt as an ID, a title
over 200 characters or on several lines, a tag with uppercase letters.

```text
design/tasks/TASK-002-write-installer.md:6:9: K104 error: tag `Release` must be lowercase
design/tasks/TASK-002-write-installer.md:7:1: K104 error: unknown key `priority` in front matter
design/requirements/REQ-020.md:1:1: K104 error: front matter needs `title`
design/requirements/REQ-020.md:5:7: K104 error: `due` must be a date (due = 2026-11-30)
design/requirements/REQ-14.md:2:7: K104 error: id `REQ-14`: write it REQ-014 (at least three digits, no extra zeros)
design/HAZ-001-x.md:3:8: K104 error: type `hazard` isn't declared in kiln.toml (requirement, design, decision, milestone, task, test, risk)
```

**Fix:** as the message says. Fields Kiln has no key for (priority,
likelihood…) go in an `[extra]` table, which Kiln accepts and keeps. A
new type goes in `kiln.toml` first (see
[Configuration](Configuration#adding-a-type)). An item whose `id` is
unusable isn't loaded, so references to it fail K102 too.

### K105

**Triggers:** the file name doesn't start with the item's ID.

```text
design/tests/upgrade-test.md:2:7: K105 warning: the file name doesn't start with REQ-005
```

**Fix:** rename the file to `<ID>-<anything>.md` (in an editor connected
to `kiln lsp`, the code action "rename the file to match its ID" does
it). If your team names files differently on purpose, set `K105 = "off"`.

### K106

**Triggers:** the ID's prefix isn't the prefix of the item's `type`.

```text
design/tests/upgrade-test.md:2:7: K106 warning: REQ-005 has prefix REQ, but test items use TEST
```

**Fix:** usually the item was copied from another type's file. Either fix
`type`, or give it an ID of the right prefix: create a fresh item with
`kiln new test` and move the content, or rename the `id` by hand to the
next free number and update references. It also appears after you change
a type's prefix in `kiln.toml`.

### K107

**Triggers:** `status` isn't one of the type's statuses.

```text
design/tasks/TASK-002-write-installer.md:5:10: K107 error: status `in-progress` isn't one of task's: todo, doing, blocked, done, dropped
```

**Fix:** use one of the listed statuses (`kiln set TASK-002
status=doing`), or add the status to the type in `kiln.toml`. After
renaming statuses in `kiln.toml`, every item with an old status shows up
here.

### K108

**Triggers:** a link or parent the configuration doesn't allow: a role
not declared in `kiln.toml`, a role this item's type can't be the source
of, a target of a type the role doesn't point at, a `parent` of a type
not in the type's `parents`, a code reference whose role doesn't list
`code` in `from`.

```text
design/tests/TEST-005-x.md:8:1: K108 error: a test can't be the source of `implements` (from: design, task, code)
design/tests/TEST-005-x.md:9:1: K108 error: `covers` isn't a link role in kiln.toml
design/tasks/TASK-002-write-installer.md:10:16: K108 error: `implements` can't point at a decision (ADR-001; to: requirement)
design/requirements/REQ-020.md:6:11: K108 error: a requirement can't have a decision as parent (allowed: requirement)
src/main.rs:3:4: K108 error: `mitigates` isn't a link role code can use (implements, verifies)
```

**Fix:** use a role that fits (a test `verifies`, a task `implements`),
link the right item, or widen the role's `from`/`to` or the type's
`parents` in `kiln.toml` if your process really needs it. `kiln link`
checks the same things before it writes.

### K109

**Triggers:** a link is written under its inverse name, on the target's
side: `verified-by = ["TEST-001"]` in a requirement instead of
`verifies = ["REQ-…"]` in the test. Kiln recognises any role's `inverse`
with spaces turned into hyphens (`implemented-by`, `mitigated-by`,
`blocks`…).

```text
design/tasks/TASK-002-write-installer.md:11:1: K109 warning: `verified-by` is how `verifies` reads from the other side; write `verifies = ["TASK-002"]` in the other item
```

**Fix:** remove the line and write the link in the other item, as the
message shows: `kiln link TEST-001 verifies TASK-002`. The inverse appears
on this side by itself. Until you fix it, the link doesn't count.

### K110

**Triggers:** a relative link or image in a body points at a file that
doesn't exist, or outside the project.

```text
design/requirements/REQ-003-settings-survive-an-upgrade.md:17:12: K110 error: `img/upgrade.png` doesn't exist (design/requirements/img/upgrade.png)
```

**Fix:** the message shows where Kiln looked (in brackets): paths are
relative to the item's folder, or to the project root when they start
with `/`. Correct the path, or add the file. URLs, `mailto:` and
`#anchors` are never checked.

### K111

**Triggers:** a ` ```mermaid ` block that the diagram renderer can't
parse. Reported on the diagram's own line.

```text
design/designs/DES-002-flow.md:10:3: K111 error: diagram: invalid flowchart edge syntax -- A -->
```

**Fix:** correct the diagram. Exports use the same renderer, so a diagram
that passes the check draws. If you only want to show diagram code
without drawing it, use a different fence language (`text`).

### K112

**Triggers:** an export target's `template` file doesn't exist or isn't
UTF-8, or contains an unknown directive or attribute, or names a type,
status or ID that doesn't exist; a Word target's `reference` document
doesn't exist or isn't a `.docx`. Each template is checked once, even if
several targets share it.

```text
kiln.toml:1:1: K112 error: export `md`: template design/templates/missing.md doesn't exist or isn't UTF-8
design/templates/doc.md:3:1: K112 error: `widget` isn't a type
design/templates/doc.md:5:1: K112 error: unknown directive kiln:frobnicate (items, item, report, summary, toc, value)
design/templates/doc.md:9:1: K112 error: style `table` isn't full, row or compact
```

**Fix:** correct the path or the directive. The directives are on
[Templates and exports](Templates-and-exports). A preset's template
names the preset's types; if you rename or remove one of them, change the
template's directives too ([Presets](Presets#renaming-a-type-a-template-uses)).

### K113

**Triggers:** the agent guide (`AGENTS.md` in the first folder of
`paths`, usually `design/AGENTS.md`) has Kiln's section, and that
section is not what `kiln agents` would write from `kiln.toml` now (a
type, status, role or export was added, changed or removed since), or
its begin marker has no end marker.

```text
design/AGENTS.md:51:1: K113 warning: Kiln's section no longer matches kiln.toml; `kiln agents` brings it up to date
```

A stale guide tells coding agents about types and folders the project no
longer has, so they file items in the wrong place or with the wrong
statuses.

**Fix:** run `kiln agents` and commit the result. Only Kiln's section
changes. Without a guide, or with the section deleted, there is nothing
to check.

## Design rules (K201–K214)

These look at the design as a whole: what is covered, what is overdue,
what is inconsistent. They are warnings by default (K207 is off), because
a project in progress always has gaps. Several look for specific type,
status and role names from the defaults; with other names they simply
don't fire (see [Configuration](Configuration#what-to-watch-for)).

### K201 and K202: coverage of requirements

**Triggers:** a requirement (an item of the type named `requirement`,
or with `[requirements] levels`, of any level whose type the role's `to`
includes; [Configuration](Configuration#requirements-requirement-levels)) that

- has a status other than its type's `initial` status (`draft` by
  default), and
- isn't in a `closed` status (`obsolete` by default), and
- isn't covered for `verifies` (K201) or `implements` (K202).

```text
design/requirements/REQ-004-logs.md:2:7: K201 warning: REQ-004 is approved but nothing verifies it
design/requirements/REQ-004-logs.md:2:7: K202 warning: REQ-004 is approved but nothing implements it
```

What "covered" means, exactly:

- At least one source links to the requirement with that role, and the
  source is **not closed**. Sources are items (`verifies` from a test,
  `implements` from a design or task) and code references
  (`// verifies: REQ-004`). A test in any non-closed status counts, even
  `draft` or `failing`; an `obsolete` test doesn't.
- **Or** the requirement has at least one non-closed child, and every
  non-closed child is covered. Closed children are ignored. So a parent
  requirement that only groups others is covered once all its live
  children are. `kiln trace` shows this as "(through its children)":

  ```console
  $ kiln trace
  …
  REQ-004  Logs rotate daily  (approved)
           implemented by: — gap
           verified by: (through its children)

  REQ-005  Logs rotate at midnight  (proposed)
           implemented by: — gap
           verified by: TEST-002
  …
  ```

  Here REQ-004 has two children: REQ-005, verified by a test, and an
  obsolete one that doesn't count.
- Mentions in the body (`[[REQ-004]]`, a bare `REQ-004`) never count.

"Past the initial status" means *any* status other than `initial`, not
"later in the list": with `initial = "proposed"`, a requirement in `draft`
is checked too. The idea is that a draft requirement is still being
written, while one that has been proposed or approved should be
traceable. A rule only runs if its role (`verifies` or `implements`)
exists in `kiln.toml`.

**Fix:** write the missing link where it belongs: in the test
(`kiln link TEST-002 verifies REQ-004`), in the task or design (`kiln
link TASK-007 implements REQ-004`), or as a code reference. Or split the
requirement into children that are each covered. If the requirement is
no longer wanted, set it to a closed status.

### K203

**Triggers:** an item that isn't closed links (in `[links]`) to an item
that is closed.

```text
design/tasks/TASK-003-old-budget.md:8:16: K203 warning: TASK-003 links to REQ-008, which is obsolete
```

**Fix:** usually the target was replaced: point the link at the new item,
or close the source too (`kiln set TASK-003 status=dropped`). Closed
items linking to closed items are fine.

One expected case: with the default types, `superseded` is a closed
status of decisions, so the `supersedes` link that K204 asks for also
gets K203 on the new decision ("ADR-004 links to ADR-002, which is
superseded"). Kiln has no per-role exception. If your project uses
`supersedes`, either accept this warning, or remove `superseded` from the
decision type's `closed` list (superseded decisions then stay visible in
exports and lists), or set `K203 = "off"`.

### K204

**Triggers:** an item of type `decision` with status `superseded` that no
other decision links to with `supersedes`.

```text
design/decisions/ADR-002-yaml.md:5:10: K204 warning: ADR-002 is superseded, but no decision `supersedes` it
```

**Fix:** in the decision that replaced it, add the link:
`kiln link ADR-004 supersedes ADR-002`. A reader of ADR-002 can then
follow "superseded by ADR-004". If nothing replaced it, the right status
is probably `rejected`.

### K205

**Triggers:** any item with status `verified` that has no **passing
`verifies` source**. A passing source is either

- an item of type `test` with status `passing` that links to it with
  `verifies`, or
- a code reference `verifies: ID` (code is taken as passing: Kiln
  doesn't run your tests).

A test in `draft`, `ready` or `failing` doesn't count here, although it
counts for coverage (K201).

```text
design/requirements/REQ-009-logs.md:5:10: K205 warning: REQ-009 is verified, but no passing test or code reference verifies it
```

**Fix:** set the test's status to `passing` once it passes, add a
`verifies` link from a passing test or code, or move the item back to a
status that doesn't claim verification (`implemented`).

### K206

**Triggers:** an item of type `milestone` with a `due` date before today
(UTC) whose status is neither in `done` nor in `closed`.

```text
design/milestones/MS-002-beta.md:2:7: K206 warning: MS-002 was due 2026-09-01 and is active
```

**Fix:** finish it (`kiln set MS-002 status=done`), move the date
(`kiln set MS-002 due=2026-12-01`), or drop it (`status=dropped`). Tasks
with past `due` dates are not checked.

### K207

**Triggers:** an item that has `jira` keys and whose type has `done`
statuses (tasks, milestones, requirements with the defaults), when
either

- the item is done but one of its tickets isn't resolved, or
- every ticket is resolved but the item is neither done nor closed.

"Resolved" means the ticket's status is in Jira's **Done** category,
whatever your workflow calls it. Tickets Jira doesn't know are left out.
The message names the tickets and their statuses, and for the second
case the status to set.

K207 is **off by default** because it needs Jira: turn it on with
`K207 = "warning"` (or `"error"`), and set `[jira] url` and a token
(`KILN_JIRA_TOKEN`, or `kiln auth jira`). It reads the same cached
mirror as `kiln list --jira` (tickets are re-read after ten minutes).
When Jira can't be read (no token, no network, a Jira Cloud URL), `kiln
check` says why on stderr and checks everything else; Jira never makes
the check fail by being unavailable:

```console
$ kiln check
kiln: K207 not checked: couldn't read `jira:https://jira.example.com` from the credential store: Platform secure storage failure: no secret service provider or dbus session found (or set KILN_JIRA_TOKEN)
kiln: no findings
```

Limits: only Jira Server and Data Center can be read, not Jira Cloud;
Jira calls don't go through `HTTPS_PROXY` yet; and the Jira support has
been tested against a simulated Jira, not yet a live instance. See
[Jira](Jira).

**Fix:** update whichever side is behind: set the item's status
(`kiln set TASK-021 status=done`), or resolve or reopen the ticket in
Jira.

### K208

Only with several requirement levels
([Configuration](Configuration#requirements-requirement-levels)).

**Triggers:**

- a requirement below the top level, past its type's `initial` status
  and not closed, that has no trace link (`refines` by default) to an
  open requirement, itself or through a parent of its own type, and
  isn't `derived = true`;
- `derived = true` on an item that isn't a requirement below the top
  level;
- `derived = true` on a requirement that also has a trace link.

```text
design/system/SYS-002-logs.md:2:7: K208 warning: SYS-002 is approved but refines no stakeholder requirement; link it with `kiln link SYS-002 refines <ID>`, or set `derived = true` and say why in its body
```

A requirement that answers nothing above it is either forgotten or
deliberate; `derived = true` says which, and reviewers see it in the
merge request and the export.

**Fix:** link it (`kiln link SYS-002 refines STK-001`), or add `derived =
true` and a paragraph on why it exists. Decomposed requirements need the
link only at the top of their subtree: children trace through their
parent.

### K209

**Triggers:** a trace link whose target isn't of the level directly
above: it skips a level (software refining a stakeholder requirement),
or points at the same level or lower.

```text
design/software/SW-004-poll.md:7:13: K209 warning: SW-004 refines STK-001, a stakeholder requirement; `refines` points at the level directly above, a system requirement
```

The link still counts where it points, so the requirement above isn't
also reported by K210: one wrong link, one finding.

**Fix:** point it at the system requirement that the software
requirement refines (`kiln unlink SW-004 refines STK-001`, then `kiln
link SW-004 refines SYS-001`); if no system requirement covers it yet,
that is the gap to fill.

### K210

**Triggers:** a requirement above the lowest level, past its type's
`initial` status and not closed, that no open requirement of the level
below refines, directly or through all its open children (the same
"covered" as K201).

```text
design/stakeholder/STK-002-audit.md:2:7: K210 warning: STK-002 is approved but no system requirement refines it
```

**Fix:** write the requirement of the level below that answers it, and
link it upward; or, if it needs no further refinement, close it or
reconsider whether it belongs at this level.

### K211

**Triggers:** a milestone whose status is done (`done` by default) while
work that is `part-of` it, directly or through its parent, is neither
done nor closed. One finding per such item, on the milestone's status.

```text
design/milestones/MS-001-first-release.md:5:10: K211 warning: MS-001 is done, but TASK-004 (doing) is part of it
```

**Fix:** finish the work, drop it (`kiln set TASK-004 status=dropped`),
move it to the next milestone (`kiln unlink TASK-004 part-of MS-001`,
then `kiln link TASK-004 part-of MS-002`), or set the milestone back to
`active`.

### K212

**Triggers:** an item whose status is done while something it
`depends-on` is neither done nor closed.

```text
design/tasks/TASK-002-wire-the-button.md:5:10: K212 warning: TASK-002 is done, but it depends on TASK-001 (doing)
```

**Fix:** finish the dependency, or, if the work never needed it, remove
the link (`kiln unlink TASK-002 depends-on TASK-001`).

### K213

**Triggers:** open work (an item of a type the `part-of` role may link
from: `task` by default) that is `part-of` no milestone, itself or
through its parent. A sub-task (a task with a task `parent`) belongs
where its parent does.

```text
design/tasks/TASK-007-tidy-the-logs.md:2:7: K213 warning: TASK-007 is open work in no milestone; `kiln link TASK-007 part-of MS-…` places it
```

**Fix:** place it (`kiln link TASK-007 part-of MS-002`). If your project
keeps loose tasks on purpose, turn the rule off (`K213 = "off"`).

### K214

**Triggers:** a requirement with status `implemented` or `verified`
while work that `implements` it is neither done nor closed.

```text
design/requirements/REQ-003-export-csv.md:5:10: K214 warning: REQ-003 is implemented, but TASK-001 (doing) implementing it isn't done
```

**Fix:** finish the work, or set the requirement back to `approved`
until it is.

All four come from the same idea as `kiln next`: work is *open* while
neither done nor closed. They catch a status moved ahead of the work it
stands for, the mistake that is easiest to make quietly, by a person or
a coding agent.

## Recommended settings for a team

Start with the defaults while you write the first items. Then tighten as
the design settles:

```toml
[check]
# Once requirements are reviewed: an approved requirement must be
# verified and implemented by something, or CI fails.
K201 = "error"
K202 = "error"

# Before a release: no live work pointing at retired items.
K203 = "error"

# Claims of verification must be backed by a passing test.
K205 = "error"

# With [jira] url and a token in CI: keep Kiln and Jira in step.
K207 = "warning"
```

Suggestions:

- **Keep every error rule an error.** They mean something is broken (a
  reference to nothing, a status the type doesn't have), and they are
  cheap to fix when they appear.
- **K201 as error once the design settles** is the most useful change: it
  is what turns Kiln from a document store into a traceability gate. Use
  the `initial` status (`draft`) for requirements still being written, so
  the gate applies only to requirements someone has proposed or approved.
- **K202 as error** when every requirement should map to a design, a task
  or code. Projects that track implementation only through code
  references need `[code]` configured first.
- **K206 stays a warning**: dates slip, and a red CI over a date helps
  nobody. Review the warnings in planning instead.
- **K105 off** only if you deliberately name files without IDs.
- **K203**: if you use `supersedes`, read [K203](#k203) first.

## Warnings, errors and CI

- `kiln check` exits 1 only for errors, so in CI a job running `kiln
  check` fails on errors and passes with warnings.
- Warnings still show: in the job log, as annotations on a GitHub pull
  request with `--format github`, or in GitLab's merge-request widget with
  `--format gitlab`. They are visible without blocking anyone.
- To produce a GitLab report *and* fail on errors, run it twice: once for
  the report, allowing failure, and once plainly:

  ```yaml
  kiln-check:
    script:
      - kiln check --format gitlab > gl-code-quality.json || true
      - kiln check
    artifacts:
      when: always
      reports:
        codequality: gl-code-quality.json
  ```

- On GitHub, one step is enough:

  ```yaml
  - run: kiln check --format github
  ```

- The rules are the same everywhere: a finding your editor shows through
  `kiln lsp` is the one CI reports, at the severity `kiln.toml` sets.

Complete workflows, including exports, are on
[Continuous integration](Continuous-integration).

See also: [Item files](Item-files), [Configuration](Configuration),
[Continuous integration](Continuous-integration),
[Troubleshooting and FAQ](Troubleshooting)
