# Presets

A **preset** is a document template Kiln ships: arc42, a software
requirements specification, a design document. `kiln preset add` copies
one into your project and adds what it needs to `kiln.toml`: the item
types the template places, the links between them, and an export. This
page says what each preset adds, and the options worth knowing about
once it is there.

- [Adding a preset](#adding-a-preset)
- [arc42](#arc42)
- [Software requirements specification (srs)](#software-requirements-specification-srs)
- [Design document (design-doc)](#design-document-design-doc)
- [Publishing a preset to Confluence](#publishing-a-preset-to-confluence)

## Adding a preset

```console
$ kiln preset list
arc42       arc42 architecture documentation: twelve sections, one page each
srs         a software requirements specification after ISO/IEC/IEEE 29148, one document
design-doc  a design document: requirements, design, decisions, milestones, risks
$ kiln preset add arc42
kiln.toml
design/templates/arc42.md
design/templates/quality-scenario.md
design/templates/building-block.md
kiln: added the arc42 preset (types quality-goal, quality-scenario, constraint, block, term; already in kiln.toml, kept as they are: decision, risk; roles addresses; mitigates gains from decision, block; export `arc42`); `kiln export arc42` writes it
```

`kiln init --preset arc42` does the same in a new project. `--dry-run`
prints what would change and writes nothing.

`kiln preset add` writes:

- **the template**, `design/templates/arc42.md` (the `templates/`
  folder of your first `paths` entry);
- **the types and link roles it places**, into `kiln.toml`, each after
  your last type (or role), with a comment saying what it is for;
- **starting text for new items** of the types it adds
  (`design/templates/quality-scenario.md`, used by `kiln new
  quality-scenario`);
- **an export**, `[[export]] name = "arc42"`, writing Markdown, and the
  same export for Confluence commented out below it, at the end of
  `kiln.toml`.

The rest of `kiln.toml`, comments and layout included, stays as it was.

### Several presets in one project

Presets add to each other; nothing is declared twice:

- **A type `kiln.toml` already has is kept as it is**, whether you wrote
  it or another preset added it. `arc42` and `srs` both use `constraint`
  and `term`; whichever you add second finds them there and leaves them
  alone. The templates use no status names, so they work with your own
  definitions.
- **A link role `kiln.toml` already has gains only what is missing** in
  its `from` and `to` lists, never a second table, never a repeated
  entry.
- **Each preset has its own export**, named after it, so `kiln export
  arc42` and `kiln export srs` write separate documents. Adding a preset
  whose export is already there stops: it was added before.

```console
$ kiln preset add srs
kiln.toml
design/templates/srs.md
kiln: added the srs preset (already in kiln.toml, kept as they are: requirement, constraint, test, term; export `srs`); `kiln export srs` writes it
$ kiln preset add srs
kiln: kiln.toml already has an export named `srs`; the srs preset is added already, or rename that export first
```

It never overwrites and, when it refuses, changes nothing: a file it
would write that is already there, an export of the same name, a type
whose prefix another type already has (rename one first), or a result
`kiln check` would find an export or template error in.

### After adding

Create items (`kiln new quality-goal --title "…"`) and run `kiln export
arc42`. The template and the configuration are yours from then on. The
template's comments (`<!-- … -->`) are guidance for whoever edits it:
GitLab and GitHub hide them and exports leave them out. Replace the
placeholder sentences, delete the sections you don't need, move them
around. A newer Kiln never changes a preset you have added; to see the
current one, run `kiln init --preset NAME` in an empty folder.

## arc42

[arc42](https://arc42.org) is a template for architecture
documentation in twelve sections. Kiln's version follows its structure
with wording of its own, and fills the sections that are lists from
items.

### What fills each section

| arc42 section | In the template | Items |
|---|---|---|
| 1.1 Requirements overview | prose (or Kiln requirements, see [options](#arc42-requirements-in-section-1)) | |
| 1.2 Quality goals | a table | `quality-goal` (QG) |
| 1.3 Stakeholders | a Markdown table to fill in | |
| 2 Architecture constraints | a table | `constraint` (CON) |
| 3 Context and scope | prose and a Mermaid diagram | |
| 4 Solution strategy | prose | |
| 5 Building block view | a diagram, then the blocks nested by parent: levels 1, 2, 3 | `block` (BB) |
| 6 Runtime view | prose and sequence diagrams | |
| 7 Deployment view | prose | |
| 8 Crosscutting concepts | prose, a subsection per concept | |
| 9 Architecture decisions | each decision with its text | `decision` (ADR) |
| 10 Quality requirements | each scenario under its goal | `quality-scenario` (QS) |
| 11 Risks and technical debt | the risks report: open risks and what mitigates them | `risk` (RISK) |
| 12 Glossary | each term, bold, then its definition (`style="compact"`) | `term` (TERM) |

With `layout = "sections"`, each of the twelve is a page (or a file),
under a main page that lists them.

### What it adds to kiln.toml

To the `kiln.toml` that `kiln init` writes, `kiln preset add arc42` adds
the types below; `decision` and `risk` are already there and kept. In
`[links.mitigates]` it adds `decision` and `block` to `from`, so they can
mitigate a risk. Here for a project named Shop, whose items are in
`design/`:

<!-- added: arc42 -->
```toml
# arc42 section 1.2: the few quality goals that matter most.
[types.quality-goal]
prefix = "QG"
title = "Quality goal"
statuses = ["proposed", "agreed", "obsolete"]
closed = ["obsolete"]

# Section 10: a scenario makes a goal concrete; its goal is its parent.
[types.quality-scenario]
prefix = "QS"
title = "Quality scenario"
statuses = ["proposed", "agreed", "met", "obsolete"]
done = ["met"]
closed = ["obsolete"]
parents = ["quality-goal"]
template = "design/templates/quality-scenario.md"

# Section 2: technical and organisational constraints, conventions.
[types.constraint]
prefix = "CON"
statuses = ["proposed", "accepted", "obsolete"]
closed = ["obsolete"]

# Section 5: building blocks, nested by parent (levels 1, 2, 3).
[types.block]
prefix = "BB"
title = "Building block"
statuses = ["draft", "accepted", "obsolete"]
closed = ["obsolete"]
parents = ["block"]
template = "design/templates/building-block.md"

# Section 12: one item per term; the body is the definition.
[types.term]
prefix = "TERM"
title = "Term"
plural = "Glossary"
statuses = ["proposed", "accepted", "obsolete"]
closed = ["obsolete"]

# Decisions and blocks say which quality goals and scenarios they serve.
[links.addresses]
from = ["decision", "block"]
to = ["quality-goal", "quality-scenario"]
inverse = "addressed by"

[[export]]
name = "arc42"
format = "markdown"
layout = "sections"
title = "Shop — architecture"
template = "design/templates/arc42.md"
path = "docs/arc42"
```

That is enough for every section of the template. The export's `title`
is the main page's title, `<project name> — architecture`; change it as
you like (on Confluence it has to be unique in the space).

A quality goal and one of its scenarios:

```console
$ kiln new quality-goal --title "Pages answer within a second"
design/quality-goals/QG-001-pages-answer-within-a-second.md
$ kiln new quality-scenario --title "Search under peak load" --parent QG-001
design/quality-scenarios/QS-001-search-under-peak-load.md
```

### Options

Each option below is independent and goes into `kiln.toml` by hand.
Where `kiln.toml` already has a table with the same name
(`[links.verifies]`, say), replace it with the one here. The links ones
are what make Kiln worth more than a wiki for arc42: `kiln impact`,
`kiln trace` and `kiln show` follow them.

#### Tests verify quality scenarios

arc42 asks for scenarios that can be tested. Let tests (and test code)
prove them:

<!-- tested: arc42 -->
```toml
[links.verifies]
from = ["test", "code"]
to = ["requirement", "quality-scenario"]
inverse = "verified by"
```

A test item with `verifies = ["QS-001"]`, or `// verifies: QS-001` above
a test function, shows on the scenario as "Verified by".
`kiln trace --type quality-scenario` lists the scenarios nothing verifies.

#### Code implements building blocks

Say in the code which block it belongs to, and each block shows where it
lives ("Implemented by src/basket/mod.rs:1"):

<!-- tested: arc42 -->
```toml
[links.implements]
from = ["design", "task", "code"]
to = ["requirement", "block"]
inverse = "implemented by"
```

```rust
// implements: BB-003
pub mod basket;
```

Code is scanned only with a `[code]` section (`paths = ["src"]`); see
[Configuration](Configuration).

#### Blocks use other blocks

The arrows of the level-1 diagram, as links: `kiln impact BB-003` then
lists every block that depends on BB-003, directly or not.

<!-- tested: arc42 -->
```toml
[links.uses]
from = ["block"]
to = ["block"]
inverse = "used by"
```

#### Decisions affect blocks

Each block shows the decisions that shaped it, and each decision the
blocks it touches:

<!-- tested: arc42 -->
```toml
[links.affects]
from = ["decision"]
to = ["block"]
inverse = "affected by"
```

#### Decisions and blocks respect constraints

So a constraint shows what it constrains, and one nothing refers to
stands out:

<!-- tested: arc42 -->
```toml
[links.respects]
from = ["decision", "block"]
to = ["constraint"]
inverse = "respected by"
```

#### Starting text for new items

The preset gives quality scenarios and building blocks a starting text:
`template = "design/templates/quality-scenario.md"` in the type, and the
file, arc42's six parts of a scenario (source, stimulus, environment,
response, measure). Edit the files to change what `kiln new` writes, or
add a `template` to `decision` for context, decision and consequences.

#### Technical debt apart from risks

Tag debt items `debt` (`tags = ["debt"]` in the item) and give section 11
two parts in the template:

```markdown
# Risks and Technical Debt

## Technical Debt

<!-- kiln:items type="risk" tag="debt" style="row" -->

## Risks

<!-- kiln:items type="risk" -->
```

An item appears once per document, so "Risks" gets the rest. A separate
`debt` type works too, if debt needs other statuses than risks.

#### Quality goals in priority order

arc42 ranks its quality goals. Items are listed in ID order, so create
them in priority order, and use `kiln renumber QG-004 --to QG-001` (after
moving the others) to change it. A number in `[extra]`
(`priority = 1`) is kept in the file for people reading it, but Kiln
doesn't sort by it or show it in exports.

#### Crosscutting concepts as items

When the concepts of section 8 are rules blocks must follow (error
handling, security, logging), make them items and link the blocks that
apply them:

<!-- tested: arc42 -->
```toml
[types.concept]
prefix = "CPT"
statuses = ["draft", "accepted", "obsolete"]
closed = ["obsolete"]

[links.applies]
from = ["block"]
to = ["concept"]
inverse = "applied by"
```

and in the template, under `# Crosscutting Concepts`, replace the
example subsections with `<!-- kiln:items type="concept" -->`.

#### Runtime scenarios as items

When you want to know which scenarios a change to a block touches:

<!-- tested: arc42 -->
```toml
[types.runtime]
prefix = "RT"
title = "Runtime scenario"
statuses = ["draft", "accepted", "obsolete"]
closed = ["obsolete"]

[links.involves]
from = ["runtime"]
to = ["block"]
inverse = "involved in"
```

and under `# Runtime View`, `<!-- kiln:items type="runtime" -->`. Put
each scenario's sequence diagram in its body.

#### arc42 requirements in section 1

If your requirements are Kiln requirements (the `requirement` type of
`kiln init`), list them in section 1.1 instead of summarising:

```markdown
## Requirements Overview

<!-- kiln:items type="requirement" style="row" -->
```

Then blocks implementing requirements (with `implements`, above) give
you traceability from requirement to block to code.

#### Fewer types

The types `kiln init` writes and arc42 doesn't use (design, milestone,
task, test) cost nothing: a type without items never shows in the
document. Remove them if you want `kiln new` to offer fewer, along with
the links that mention them. Keep `test` if you verify scenarios.

#### Renaming a type a template uses

The template names its types in its directives
(`<!-- kiln:items type="block" -->`). Rename a type in `kiln.toml` and
change those too; `kiln check` (K112) names any directive still using the
old name, with its line. Items keep their `type = "…"` line, so rename
it there as well (a search and replace over `design/`).

#### Statuses and prefixes

The statuses above are a starting point; the template uses none by
name, so rename them as you like. `done` statuses count as finished for
progress, `closed` ones are left out of documents. Change a prefix
before creating items of that type; after, `kiln renumber` moves them one
at a time.

## Software requirements specification (srs)

A requirements specification after the outline of ISO/IEC/IEEE 29148:
introduction, references, the requirements by kind, verification,
appendices. One document.

### What fills each section

| Section | Items |
|---|---|
| 1.7 Definitions | `term`, bold with their definition |
| 3.1–3.5, 3.7 Functions, performance, usability, interfaces, logical database, system attributes | `requirement` with the tag `function`, `performance`, `usability`, `interface`, `data` or `attribute` |
| 3.6 Design constraints | `constraint`, a table |
| 3.8 Other requirements | every requirement not placed above |
| 4 Verification | `test`, a table, then the traceability report |
| Appendix: open findings | `kiln check`'s findings |

### What it adds to kiln.toml

`requirement`, `test` and `verifies` come with `kiln init` and are kept;
`kiln preset add srs` adds:

<!-- added: srs -->
```toml
[types.constraint]
prefix = "CON"
statuses = ["proposed", "accepted", "obsolete"]
closed = ["obsolete"]

[types.term]
prefix = "TERM"
title = "Term"
plural = "Glossary"
statuses = ["proposed", "accepted", "obsolete"]
closed = ["obsolete"]

[[export]]
name = "srs"
format = "markdown"
title = "Shop — software requirements specification"
template = "design/templates/srs.md"
path = "docs/SRS.md"
```

A requirement goes to its section by its tag, in its front matter:

```toml
+++
id = "REQ-001"
type = "requirement"
title = "Search returns results within 500 ms"
status = "proposed"
tags = ["performance"]
+++
```

### Options

- **Word for a deliverable.** `format = "docx"` with `path =
  "docs/SRS.docx"`, and `[export.docx] reference = "…"` for your
  company's styles ([Templates and exports](Templates-and-exports)).
- **Fail CI on gaps.** `[check] K201 = "error"` fails `kiln check` when
  an approved requirement has nothing verifying it; `K202` the same for
  implementing ([Checking](Checking)).
- **Starting text for requirements.** On a project without a
  `requirement` type, the preset adds one with `template =
  "design/templates/requirement.md"` ("The system shall…", rationale,
  acceptance). With the `kiln init` one, add that line yourself and write
  the file.
- **Your own kinds.** Change the tags in the template's section 3, or
  add sections: `<!-- kiln:items type="requirement" tag="security" -->`.
- **A page per section.** `layout = "sections"` when the document grows
  large on Confluence.

## Design document (design-doc)

The general design document, on one page, reading top-down: an
introduction (purpose and scope, goals and non-goals), the requirements
as one tree, design, decisions, verification (the tests as a table),
plan (the roadmap and the tasks), risks, then traceability and open
findings as appendices. Each item's facts are a small table under its
heading. It places the types `kiln init` writes, so on such a project it
adds only an export.

With several requirement levels ([Configuration](Configuration#requirements-requirement-levels)),
the requirements section is the
[requirements tree](Templates-and-exports#the-requirements-tree): each
top-level requirement with the requirements refining it beneath, level
by level, derived ones apart, and the traceability appendix is the
chain matrix. The preset then adds no `requirement` type beside your
levels: its `requirement` stands for them.

<!-- added: design-doc -->
```toml
[[export]]
name = "design-doc"
format = "markdown"
title = "Shop — design document"
template = "design/templates/design-doc.md"
path = "docs/DESIGN-DOCUMENT.md"
```

To give each top-level section a page of its own on Confluence, add
`layout = "sections"` to the Confluence target; the requirements tree
then is one page.

`kiln export --check` in CI keeps a committed copy current
([Continuous integration](Continuous-integration)).

## Removing a preset

Nothing in Kiln remembers that a preset was added, so removing one is
removing what it wrote: the template (`design/templates/arc42.md`), its
item templates, the export and its commented Confluence version at the
end of `kiln.toml`, and the types and roles nothing else uses. Remove a
type only once no item has it (`kiln list --type quality-goal`), or
`kiln check` reports those items (K104). Types another preset uses too
(`constraint`, `term` for arc42 and srs) stay.

## Publishing a preset to Confluence

Each preset's export is followed in `kiln.toml` by the same export for
Confluence, commented out. Set up the `[confluence]` connection
([Publishing to Confluence](Publishing-to-Confluence)), uncomment it and
set its `parent`:

<!-- tested: arc42 -->
```toml
[confluence]
url = "https://confluence.example.com"
space = "ARCH"

[[export]]
name = "arc42-confluence"
format = "confluence"
layout = "sections"
title = "Shop — architecture"
template = "design/templates/arc42.md"
[export.confluence]
parent = 123456789
qualify_titles = "always"
children = "list"
```

- `layout = "sections"`: a main page, and one page per arc42 section
  under it. `document` puts all twelve on one page; `tree` also gives
  each item a page of its own (a page per building block, say).
- `qualify_titles = "always"`: every section page carries the export's
  title, `1 Introduction and Goals (Shop — architecture)`, so other
  projects with the same template can publish to the same space
  ([Several projects in one space](Publishing-to-Confluence#several-projects-in-one-space)).
- `children = "list"`: the main page lists the twelve sections in
  document order. With the default, Confluence's Children macro lists
  them in its page-tree order, which can put "10" right after "1".
