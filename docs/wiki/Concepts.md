The ideas Kiln is built on, each explained once. Other pages use these
words without defining them again. Read this after the
[Tutorial](Tutorial), or whenever a term is unclear.

## The repository is the source of truth

Kiln keeps no database and needs no server. Everything it knows comes
from three places:

- the files in your repository (`kiln.toml`, the item files, your source
  code);
- git history, for baselines and for the commit a link names;
- Jira, read-only, if you ask it to.

Delete Kiln and nothing is lost: the items are Markdown files anyone can
read on GitHub or GitLab.

Kiln writes only when you ask: `kiln new`, `set`, `link`, `unlink`,
`renumber`, `export`, and a status change from Flint. Exports are one-way.
The repository is the source and a Confluence page is a view of it.
Edits made in Confluence are detected and reported, never pulled back.

## Project

A **project** is a folder with a `kiln.toml` at its root, normally the
root of a repository. Kiln finds it by walking up from the current
folder, as git does. `kiln.toml` declares the types, link roles, where
items and code live, check severities and export targets
([Configuration](Configuration)).

## Item

An **item** is one Markdown file under the project's `paths` (`design/`
by default) whose first line is `+++`. It holds one idea: a requirement,
a decision, a risk. It has:

- a **front matter** block in TOML between two `+++` lines: `id`, `type`,
  `title`, `status`, and optionally `parent`, `tags`, `jira`, `owner`,
  `due`, `[links]` and `[extra]`;
- a **body** in Markdown: the prose readers see in the document.

Markdown files without front matter (a `README.md` in `design/`, a
template) are ignored. [Item files](Item-files) has the full format.

## Type

Every item has a **type**, declared in `kiln.toml`. A type has:

| Property | Means |
|---|---|
| `prefix` | the letters of its IDs (`REQ`) |
| `statuses` | the statuses its items can have, in order |
| `initial` | the status `kiln new` writes (default: the first) |
| `done` | statuses that count as finished for progress |
| `closed` | statuses that take an item out of coverage and progress (obsolete, rejected, dropped) |
| `parents` | the types an item of this type may have as `parent` |
| `template` | a file whose text becomes the body of new items |

`kiln init` declares seven types: requirement, design, decision,
milestone, task, test and risk ([Tutorial](Tutorial#1-start-the-project)).
You can change them.

## Status

An item's **status** is one of its type's statuses. Kiln gives meaning to
three groups of statuses:

- **initial**, where coverage warnings don't apply yet;
- **done**, which counts toward progress;
- **closed**, where the item no longer counts and links to it raise a
  warning.

Everything else about statuses is your process. Change a status with
`kiln set ID status=…`, the language server's code action, or Flint.

## ID

Every item has an **ID**: its type's prefix, a dash and a number with at
least three digits (`REQ-001`, `REQ-014`, `REQ-1000`). Each number has
exactly one spelling, so `REQ-7` and `REQ-0007` are not IDs. That makes
an ID findable with a plain text search.

- `kiln new` takes the next number after the highest one among the
  existing items of that prefix, closed ones included. A retired
  (`obsolete`) item keeps its number taken. A deleted file doesn't: if it
  had the highest number, that number is handed out again. That is why
  items are retired, not deleted.
- The `id` in front matter is authoritative. The file name normally
  starts with the ID (`REQ-014-flint-appears-fast.md`), but that is a
  convention; Kiln warns when it doesn't (K105).
- Two branches can create the same ID. `kiln check` reports the duplicate
  (K101), and `kiln renumber` moves one of them and rewrites the
  references.

## Parent and the tree

`parent = "REQ-002"` makes an item a child of another, of a type its
type allows. Parents form a tree; Kiln reports a cycle (K103). Use it to
break a broad item into precise ones. Exports nest children under their
parent, and the GUI's Tree page shows the whole tree.

## Link and role

A **link** connects one item (or a line of code) to another with a
**role**: `implements`, `verifies`, `part-of`, `supersedes`,
`mitigates`, `depends-on`. Each role is declared in `kiln.toml` with:

- `from`: the types that may be its source (and `code`, for code
  references);
- `to`: the types it may point at;
- `inverse`: how it reads from the other side ("implemented by").

A link is written **once, on its source side**, in the `[links]` table:

```toml
[links]
implements = ["REQ-001", "REQ-002"]
mitigates = ["RISK-003"]
```

Kiln computes the inverse. `kiln show REQ-001` then lists "implemented
by DES-001". Never write the inverse in a file: Kiln flags it (K109) and
tells you which side to write it on.

## Mention

A **mention** is a reference to an item in a body: `[[REQ-014]]`, or a
bare `REQ-014` in prose (when `[render] link_bare_ids` is on, the
default). In exports a mention becomes a link with the item's ID and
title. Mentions are listed as "mentioned by", but they carry no role and
count toward no coverage. Coverage needs a deliberate link.

A bare ID counts only if its prefix is declared in your project, it
stands as a whole word, and it is not inside a code span or block. A
mention of an ID that doesn't exist is an error (K102).

## Code reference

A **code reference** is a line in your source code that names a role
and IDs:

```python
# implements: REQ-007
def schedule(): ...

# verifies: REQ-007, REQ-008
def test_schedule_runs_every_minute(): ...
```

Kiln scans the folders listed in `[code] paths` (respecting
`.gitignore`) and matches the text `<role>: ID, ID…` on any line, in any
language. It doesn't parse the code. Each match is a link from that file
and line, for every role whose `from` includes `code`: `implements` and
`verifies` by default.

## Coverage

An item is **covered** for a role when at least one non-closed source
links to it with that role. `[coverage] roles` lists the roles that
count, `implements` and `verifies` by default. A parent is also covered
when all of its non-closed children are.

`kiln check` warns about requirements past their initial status, and not
closed, that lack coverage:

- **K201**: nothing verifies the requirement;
- **K202**: nothing implements it.

`kiln trace` and the traceability report show coverage for every
requirement, with the gaps.

## Requirement level

Requirements written at several levels (stakeholder, system, software;
or high- and low-level) are a type per level, listed highest first in
`[requirements] levels`. `parent` decomposes within a level; a link
(`refines` by default) crosses from each level to the one above. A
requirement that deliberately answers nothing above it is **derived**
(`derived = true`). `kiln check` asks that every level traces up and is
refined down (K208–K210), and `kiln trace --levels` shows the chains. See
[Configuration](Configuration#requirements-requirement-levels).

## Progress

A milestone's **progress** is its done items out of all its items,
leaving closed ones out. Its items are those `part-of` it, directly or
through their parent; the milestones it `depends-on` don't add theirs.
Progress appears in `kiln show`, `kiln status`, the roadmap report and
the GUI's Roadmap page, the same number everywhere.

## Impact

The **impact** of an item is everything that reaches it, transitively,
by a link or a parent edge, each with the path that reaches it:

- items that implement or verify it;
- the children of those items;
- code that implements those children;
- and so on.

`kiln impact ID` and the GUI's Impact page answer the question "if this
changes, what else should I look at?".

## Finding and severity

A **finding** is something `kiln check` reports: a rule code (K102), a
severity, a file, line and column, and a one-line message. Each rule has
a default severity (`error`, `warning` or `off`), which `[check]` in
`kiln.toml` can change. Errors make `kiln check` exit with 1 and fail CI;
warnings are listed but don't fail. [Checking](Checking) has every rule.

## Export, target and template

An **export target** (`[[export]]` in `kiln.toml`) produces one document
in one **format**: Markdown, HTML, Word or Confluence. It uses one
**layout**:

- `document`: everything in one file or page;
- `sections`: a page per top-level section;
- `tree`: a page per item.

A **template** is a Markdown file that shapes the document: your
headings and prose, plus **directives** (`<!-- kiln:items type="requirement"
-->`) where items and **reports** (traceability, roadmap, risks,
findings, changes) go. [Templates and exports](Templates-and-exports)
covers it all.

A **preset** is a template Kiln ships (arc42, a requirements
specification, a design document) together with the types it places and
an export for it. `kiln preset add arc42` copies it into your project,
where it is yours to change; several presets share the types they have
in common. See [Presets](Presets).

## Baseline

A **baseline** is a git revision (a tag, branch or commit) that the
current project is compared with. Items are matched by ID and reported
added, removed or changed. `kiln diff`, `kiln export --since` and the
GUI's Changes page use baselines ([Changes and
baselines](Changes-and-baselines)).

See also: [Item files](Item-files) · [Configuration](Configuration) ·
[Designing a project with Kiln](Designing-with-Kiln)
