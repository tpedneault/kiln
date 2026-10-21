# Presets

A **preset** is a document template Kiln ships: arc42, a software
requirements specification, a design document. `kiln preset add` copies
one into your project; everything else is up to you: the item types the
template places, how they link to each other, and the export that turns
it into a document. This page has the configuration for each preset,
ready to paste, and the options worth knowing about.

- [Adding a preset](#adding-a-preset)
- [arc42](#arc42)
- [Software requirements specification (srs)](#software-requirements-specification-srs)
- [Design document (design-doc)](#design-document-design-doc)
- [Publishing a preset to Confluence](#publishing-a-preset-to-confluence)

## Adding a preset

```console
$ kiln preset list
arc42       arc42 architecture documentation: twelve sections, one page each
srs         a software requirements specification after ISO/IEC/IEEE 29148
design-doc  a design document: requirements, design, decisions, milestones, risks
$ kiln preset add arc42
design/templates/arc42.md
kiln: added the arc42 template; before an export can use it, kiln.toml needs what it places: `quality-goal` isn't a type; `constraint` isn't a type; `block` isn't a type; `quality-scenario` isn't a type; `term` isn't a type (the wiki's "Presets" page has the configuration and its options)
```

`kiln init --preset arc42` does the same in a new project. `--dry-run`
names the file without writing it.

`kiln preset add` writes **one file**, the template, into the
`templates/` folder of your items folder (`design/templates/arc42.md`).
It never overwrites: if the file is there, it stops. It doesn't touch
`kiln.toml`. It tells you what the template needs that `kiln.toml`
doesn't have yet, and this page has it.

Then, for any preset:

1. **Add the types and links** from the preset's section below to
   `kiln.toml`. Where `kiln.toml` already has a table with the same name
   (`[links.mitigates]`, say), replace it with the one here.
2. **Add the export**, the `[[export]]` in the same section.
3. **Run `kiln check`.** It now checks the template too (K112): a
   directive naming a type you don't have says so, with the line.
4. **Create items** (`kiln new quality-goal --title "…"`) and run
   `kiln export arc42`.

The template is yours from then on. Its comments (`<!-- … -->`) are
guidance for whoever edits it: GitLab and GitHub hide them and exports
leave them out. Replace the placeholder sentences, delete the sections
you don't need, move them around. A newer Kiln never changes a template
you have added; to see the current one, run `kiln preset add` in an
empty folder after `kiln init`.

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

### Configuration

Add this to the `kiln.toml` that `kiln init` writes. `decision` and
`risk` are already there; replace `[links.mitigates]` with the one below,
which adds decisions and blocks to what can mitigate a risk.

<!-- tested: arc42 -->
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

[links.mitigates]
from = ["requirement", "design", "task", "decision", "block"]
to = ["risk"]
inverse = "mitigated by"

[[export]]
name = "arc42"
format = "markdown"
layout = "sections"
title = "Shop — architecture"
template = "design/templates/arc42.md"
path = "docs/arc42"
```

That is enough for every section of the template. Put your system's name
in `title`; it is the main page's title (and, on Confluence, has to be
unique in the space).

A quality goal and one of its scenarios:

```console
$ kiln new quality-goal --title "Pages answer within a second"
design/quality-goals/QG-001-pages-answer-within-a-second.md
$ kiln new quality-scenario --title "Search under peak load" --parent QG-001
design/quality-scenarios/QS-001-search-under-peak-load.md
```

### Options

Each option below is independent. The links ones are what make Kiln
worth more than a wiki for arc42: `kiln impact`, `kiln trace` and
`kiln show` follow them.

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

A type's `template` is the body `kiln new` writes. A quality scenario
in arc42's six parts:

<!-- tested: arc42 -->
```toml
[types.quality-scenario]
prefix = "QS"
title = "Quality scenario"
statuses = ["proposed", "agreed", "met", "obsolete"]
done = ["met"]
closed = ["obsolete"]
parents = ["quality-goal"]
template = "design/templates/quality-scenario.md"
```

`design/templates/quality-scenario.md` (a file without front matter, so
it is not an item):

```markdown
**Source:** who or what causes it.

**Stimulus:** what happens.

**Environment:** in which conditions (normal operation, peak load…).

**Response:** what the system does.

**Measure:** how the response is judged, with a number where there is one.
```

The same works for blocks (purpose, interfaces, where the code is) and
decisions (context, decision, consequences).

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

### Configuration

`requirement`, `test` and `verifies` come with `kiln init`; add:

<!-- tested: srs -->
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
- **Starting text for requirements.** `template =
  "design/templates/requirement.md"` under `[types.requirement]`, with
  "The system shall…", rationale and acceptance criteria.
- **Your own kinds.** Change the tags in the template's section 3, or
  add sections: `<!-- kiln:items type="requirement" tag="security" -->`.
- **A page per section.** `layout = "sections"` when the document grows
  large on Confluence.

## Design document (design-doc)

The general design document: goals and non-goals, requirements, design,
decisions, milestones, risks, traceability and open findings, in one
document. It places the types `kiln init` writes, so it needs only an
export:

<!-- tested: design-doc -->
```toml
[[export]]
name = "design-doc"
format = "markdown"
title = "Shop — design document"
template = "design/templates/design-doc.md"
path = "docs/DESIGN-DOCUMENT.md"
```

`kiln export --check` in CI keeps a committed copy current
([Continuous integration](Continuous-integration)).

## Publishing a preset to Confluence

The same export with `format = "confluence"`, after the `[confluence]`
connection is set up ([Publishing to Confluence](Publishing-to-Confluence)):

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
