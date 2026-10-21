# Exporting

How Kiln turns a project into documents for people who don't use Kiln.
Every export is a **target** declared in `kiln.toml`: what to include, how
to arrange it (a **layout**), and what to write it as (a **format**). The
default is the simplest useful thing: the whole design as **one
document**, on one Confluence page or in one file.

Implemented in `kiln-export` (assembly, layouts, the Markdown, HTML and
Word back ends) and `kiln-confluence` (the Confluence back end, see
[CONFLUENCE.md](CONFLUENCE.md)).

## The pipeline

```
index ──► select ──► assemble ──► split (layout) ──► back end ──► destination
          which      one document   into pages       storage /      Confluence
          items      tree, ordered  (or not)         Markdown /     pages, or
                     and numbered                    HTML / Word    files
```

1. **Select** the items the target includes (by template directives, or
   by `types` and status when there's no template).
2. **Assemble** a single document tree: sections, items in their place
   with their properties and bodies, report sections, numbering, and an
   anchor for every item.
3. **Split** it according to the layout: not at all (`document`), at each
   top-level section (`sections`), or at each section and each item
   (`tree`). Links between items resolve after the split: same page →
   anchor, other page → page link (plus anchor).
4. **Back end** renders each page in its format.
5. **Destination**: Confluence pages through the planner (CONFLUENCE.md),
   or files written only when their content changed.

Every target goes through the same five steps, so a layout or a format is
added once and works with every other choice.

## Targets

```toml
[[export]]
name = "spec"                         # `kiln export spec`
format = "confluence"                 # confluence | markdown | html
layout = "document"                   # document (default) | sections | tree
title = "Flint — design document"
template = "design/templates/design-document.md"   # optional; see "Templates"

[export.confluence]                   # format-specific settings
parent = 123456789                    # create the page under this page…
# page = 987654321                    # …or write into this existing page (document layout only)

[[export]]
name = "design-tree"
format = "confluence"
layout = "tree"                       # the one-page-per-item layout
title = "Flint — design (browse)"
types = ["design", "decision"]
[export.confluence]
parent = 123456789

[[export]]
name = "spec-md"
format = "markdown"
title = "Flint — design document"
template = "design/templates/design-document.md"
path = "docs/generated/DESIGN.md"     # file formats: a file (document) or a folder (sections, tree)

[[export]]
name = "spec-html"
format = "html"
title = "Flint — design document"
path = "target/kiln/design.html"
```

| Key | Default | Meaning |
|---|---|---|
| `name` | required | unique; what `kiln export <name>` runs; part of every published page's identity |
| `format` | required | `confluence`, `markdown`, `html`, `docx` |
| `layout` | `document` | `document`, `sections`, `tree` |
| `title` | the project `name` | the document's title; for Confluence, the (root) page title |
| `template` | none | a Markdown file in the repo that arranges the document |
| `types` | every type, in `kiln.toml` order | without a template: which types get a section |
| `exclude_status` | each type's `closed` statuses | items left out |
| `reports` | `["traceability", "findings"]` | without a template: report sections appended as appendices |
| `numbering` | `true` | section numbers (`2.1.3`) on headings |
| `item_style` | `list` for `document` and `sections`, `table` for `tree` | how an item's properties are shown (see "Items in a document") |
| `link_to` | `[]` | other exports to link into for items this one doesn't contain, in order |
| `path` | required for file formats | output file (`document`) or folder (`sections`, `tree`), relative to the project |
| `[export.<format>]` | — | settings only that format reads |

`kiln check` validates `[[export]]` (K005) and every template (K112), so a
broken export fails the merge request, not the publish after it.

## Layouts

| Layout | Confluence | Markdown / HTML / Word |
|---|---|---|
| **`document`** | one page holding the whole document | one file |
| **`sections`** | a page with the opening material and a list of its child pages (Confluence's `children` macro, or links in document order with `children = "list"`, CONFLUENCE.md); one child page per top-level section (each type section, each report) | `index.md` / `index.html` / `index.docx` plus one file per section |
| **`tree`** | as `sections`, and each item its own page under its section, nested by `parent` | a folder per section, a file per item |

`document` is the default because it is what a design document is: one
thing to read top to bottom, print, review in one sitting, and link to.
Every change to the project lands as one new version of one page, so
Confluence's page history becomes the document's revision history, and
"compare versions" shows a reviewer what changed.

Choose `sections` when the document grows past what one page carries well
(see "Size" in CONFLUENCE.md; Kiln warns), and `tree` when people browse
and link to individual items more than they read the whole: a requirement
database rather than a specification.

A project can have several targets: a one-page specification for review
and a browsable tree of the same items, or one document per audience.

## Templates

Without a template, a document is: the title, a short summary (the commit,
counts by type and status, coverage, milestone progress), a table of
contents, one section per type in `types` that has items, then the
`reports` as appendices (lettered `A`, `B`).

A template is a Markdown file in the repository that gives the document
its own shape and prose: an introduction, scope, definitions, the order
of sections. **Directives** are HTML comments, so the template still reads
as a plain outline on GitLab or GitHub. Any other HTML comment in a
template is a note to whoever edits it: GitLab and GitHub hide it, and so
does every export (an item body is different: raw HTML there is shown as
text, FORMAT.md, "Body").

Kiln ships ready-made templates as **presets** (DESIGN.md, "Presets"):
`kiln preset add arc42` (or `srs`, `design-doc`) copies one into
`design/templates/`, with the types it places and a target for it, and
from then on it is the project's own file, guidance in its comments:

~~~markdown
# Introduction

<!-- kiln:summary -->

This document describes Flint, a keyboard-first launcher…

<!-- kiln:toc -->

# Scope

Flint covers Windows and Linux (KDE Plasma 6 on Wayland). It does not…

# Requirements

## Performance

<!-- kiln:items type="requirement" tag="performance" -->

## Everything else

<!-- kiln:items type="requirement" -->

# Design

<!-- kiln:items type="design" root="DES-001" -->

# Decisions

<!-- kiln:items type="decision" status="accepted,superseded" style="row" -->

# Appendix: traceability

<!-- kiln:report traceability type="requirement" -->

# Appendix: open findings

<!-- kiln:report findings -->
~~~

| Directive | Places |
|---|---|
| `<!-- kiln:items … -->` | the selected items, as a flat list (in `tree`: nested by `parent`), under the heading it sits in |
| `<!-- kiln:item REQ-014 -->` | one item |
| `<!-- kiln:report traceability\|findings\|roadmap\|changes\|risks … -->` | a report (see "Reports") |
| `<!-- kiln:summary -->` | the commit, its date, counts, coverage, progress |
| `<!-- kiln:toc -->` | a table of contents (Confluence's `toc` macro; a generated list in files) |
| `<!-- kiln:value name\|commit\|date\|since -->` | a value, inline (`since`: the `--since` baseline, empty without one) |

`kiln:items` attributes, all optional and combined with AND:

| Attribute | Selects / does |
|---|---|
| `type="requirement,design"` | by type |
| `status="approved"` / `status="!obsolete"` | by status, or excluding |
| `tag="performance"` | by tag |
| `root="REQ-002"` | that item and its descendants |
| `jira="FLINT-*"` | items with a matching Jira key |
| `style="full\|row\|compact"` | `full` (default): each item as a heading with its properties and body; `row`: all of them as one table (ID, title, status, links), bodies left out, which suits many small items such as tests; `compact`: each item as a bold "ID title" line followed by its body, no heading and no properties, which suits items read for their prose, such as glossary terms |
| `nest="flat\|parent"` | `flat` (default, except in `tree`): every item a heading at the directive's level, in tree order, each parent followed by its children; `parent`: children one heading level below their parent (the default in `tree`, where it makes the page tree) |
| `split="auto\|no"` | in `tree`, `no` keeps these items inline in their section page instead of a page each |

Rules:

- The template's shallowest heading level is the document's top level,
  so a template may use `#` or `##` for its sections.
- An explicit `status=` in `kiln:items` decides on its own, even for
  statuses the target excludes by default (`status="accepted,superseded"`
  shows superseded decisions though they are closed).
- An item appears **once** per document. A later directive that selects
  an item an earlier one placed skips it (so "Everything else" above
  really is everything else). An item selected by none is simply not in
  that document; `kiln export --dry-run` lists what each target leaves
  out, so an omission is a decision, not an accident.
- Items placed with `style="row"` have no heading; each row carries an
  anchor named by the ID, so references still land on it. The same goes
  for `style="compact"`, whose anchor opens the item's bold line; the
  body's own headings become bold paragraphs, since there is no item
  heading to put them under. Items a report lists (the roadmap's
  milestones, the risks) and that nothing else placed are placed on
  their row the same way, after everything else, so a report never takes
  an item from a later `kiln:items`.
- Items are a flat list at the directive's level, in tree order: each
  item is followed by its children (within the directive), depth first,
  so related items stay together while every one is a heading of the
  same level, and a table of contents a few levels deep (Confluence's
  `toc` stops at `toc_levels`, 3 by default) lists every one. The parent
  shows in the item's properties. `nest="parent"` (the default in `tree`)
  puts children one heading level below their parent instead; an item
  whose parent isn't placed by the same directive then starts at the
  directive's level.
- The template's own headings are the document's sections and are
  numbered with everything else. Template prose is rendered like any item
  body: references, diagrams and callouts all work.
- Unknown directives, unknown attributes, unknown types and IDs are K112
  errors with the template's line.

## Items in a document

An item placed with `style="full"` becomes:

```
2.1.3  REQ-014 Flint appears within 50 ms of the hotkey   ← heading, with an anchor "REQ-014"
approved                                                  ← properties (item_style = "list")
Parent: REQ-002 Flint is fast
Verified by: TEST-031 Hotkey timing, TEST-032 Cold start
Jira: FLINT-88
<body>
2.1.4  REQ-031 …                                          ← a child: the next heading, same level
```

`item_style`:

- `list` (default for `document` and `sections`): the status lozenge on
  its own line, then one line per relationship, its label in bold:
  parent, outgoing links, inverses ("Verified by", "Implemented by" with
  code locations), Jira. Each relationship reads on its own, and an item
  with none takes one line. Lines are broken inside one paragraph
  (Confluence and HTML `<br>`, Markdown's `\` hard break, wiki markup's
  `\\`, a Word line break), so the block stays compact.
- `line`: the same on a single line, joined with `·`; the most compact,
  for long documents whose items have few links.
- `table`: the full properties table (default for `tree`, where each item
  is a page of its own).
- `none`: heading and body only, for documents meant to read as prose.

Headings: the document's sections and items use levels 1–6 by depth. In
Confluence the page title is the document's title and sections start at
level 1; in a file the title is the first heading, so sections start one
level below it.
Below level 6, items keep their number (`2.1.3.1.2.1`) on a level-6
heading. An item body's own headings are moved under the item's level,
and those that would pass level 6 become bold paragraphs.

## Reports

| Report | Content |
|---|---|
| `traceability` | per non-closed item of `type` (default requirement): implemented by, verified by, gaps marked |
| `findings` | `kiln check` warnings and errors, by rule, linked to the repository |
| `roadmap` | milestones in `due` order, status, progress, included items |
| `risks` | open risks and what mitigates them |
| `changes` | with `--since REV`: a line of totals, then one row per item added, changed or removed between REV and what is exported (the working tree as `kiln export` reads it), with a New / Changed / Removed lozenge and what changed (`status draft → approved; + verifies REQ-002; body`). Removed items are named, not linked. Without `--since` it says how to get one |

With `--since REV`, every item that changed since REV is also marked in
place with a "Changed" or "New" lozenge next to its heading, so a reviewer
reading the document finds the changes where they are, not only in the
appendix. The marker is never part of the heading text, so anchors, page
titles and links don't move when the baseline does: Markdown puts it in
italics on the line under the heading (`*Changed*`), because GitLab and
GitHub derive anchors from heading text; HTML and Confluence put a lozenge
inside the heading after its text. Items placed in a `style="row"` table
or with `style="compact"` aren't marked; the changes report lists them.

What counts as a change, and how REV is read, is FORMAT.md's "Baselines".
`<!-- kiln:value since -->` is the baseline as `v1.0 (1a2b3c4)`, empty
without `--since`.

Reports in `tree` and `sections` get their own page or file; a report
whose table passes `max_rows` (default 300) is split by top-level item.

## Formats

The back ends render the same split pages. Each gets the document tree
from `kiln-render` with references already resolved to "same page,
anchor X" or "page P, anchor X".

### `confluence`

Storage format, published through the planner with drift detection. All
of it is in [CONFLUENCE.md](CONFLUENCE.md).

### `markdown`

CommonMark with GitHub tables, readable on GitLab and GitHub.

```toml
[export.markdown]
flavour = "gitlab"         # gitlab | github | commonmark: how heading anchors are formed
diagrams = "source"        # source: keep ```mermaid blocks (GitLab and GitHub draw them) | image: PNG files beside the document
```

- Item anchors: headings start with the number and ID, so the anchor is
  predictable; Kiln computes it with the flavour's own slug rules (GitLab's
  and GitHub's differ: GitLab collapses repeated `-`) and links with it,
  across pages and into `link_to` Markdown exports too. `commonmark` adds
  an explicit `<a id="REQ-014"></a>` before the heading. GitLab and GitHub
  keep no anchors of Kiln's making, so a reference to a row item lands on
  the section holding the row.
- Images in the repository link to the raw file (`/-/raw/`, `/raw/`) when
  the export is outside the repository, so they display; relatively when
  it is inside.
- With `diagrams = "image"`, PNG files are written beside the page and the
  source follows each picture in a `<details>` block.
- Useful committed in the repository (a generated `docs/DESIGN.md`) with
  `kiln export --check` in CI to fail when it is stale, or attached to a
  release.

### `html`

One self-contained file per page: styles inline, images as `data:` URIs,
no scripts, no network fetches. A print stylesheet (page breaks before
top-level sections, link URLs shown, no navigation) makes the browser's
"Print to PDF" a decent PDF, which covers the PDF case without a PDF
engine. Light theme by default; `[export.html] theme = "<Fenix theme
name>"` will use one of the suite's palettes (only `light` exists so far;
another name is K005). A remote image (an `https:` URL) is never fetched:
it shows as its alt text, so the file stays self-contained.

### `docx`

Word, for organisations whose formal reviews run on Word documents. One
`.docx` per page, written directly as WordprocessingML (ADR-011):

- headings are Word's own **Heading 1–6** styles (a top-level section is
  Heading 1), so the navigation pane and Word's table of contents work;
  a heading deeper than six is a bold paragraph;
- every item heading is a **bookmark** named after its ID with `-` as `_`
  (`REQ_014`: Word's bookmark names allow only letters, digits and `_`),
  and references are links to it; across files (`sections`, `tree`) they
  link to the other file, and to its bookmark when the item isn't the
  page;
- `<!-- kiln:toc -->` is Word's TOC field over headings 1–3, already
  filled with links, so the document reads right before Word updates it;
  the settings ask Word to update fields on opening, which adds page
  numbers (Word asks first);
- top-level sections after the first start a new page; tables have their
  own borders and a header row repeated across pages; repository images
  (PNG, JPEG, GIF) and diagrams are embedded, scaled to the page's width;
  a remote image is never fetched and shows as its alt text, as in HTML;
- section numbers are text (`2.1.3`), and so are list bullets and
  numbers, so a reference document's numbering can't clash with them;
- status lozenges are white bold text on the colour Confluence uses;
  callouts and quotes are indented paragraphs with a coloured bar.

A **reference document** sets the house style, as Word users already
share one:

```toml
[export.docx]
reference = "design/templates/company.docx"   # project-relative
```

Kiln takes its styles (adding the ones Kiln uses that it lacks, such as
latent heading styles Word never wrote down), theme, numbering
definitions and the last section's page size and margins (without its
headers and footers, whose parts aren't copied). Without one, the
document is A4 with 2.54 cm margins in Calibri 11. `kiln check` reports a
reference that doesn't exist or isn't a `.docx` (K112).

The bytes are deterministic like every other format: parts in a fixed
order, a fixed zip time, no creation date, ids counted per document. The
one exception is the same as HTML's: diagram pictures.

### `confluence-wiki`

Confluence wiki markup in a `.txt` file per page, for pasting into a page
by hand (in the editor, Insert → Markup → Confluence wiki) where Kiln
can't publish: no token, no API access, a page someone else owns. It has
the `confluence` format's look without its planner, so no drift
detection and no idempotence: each paste replaces what you paste over.

```toml
[[export]]
name = "wiki"
format = "confluence-wiki"
path = "out/design.txt"
```

- Sections start at `h1.` (the page title is Confluence's) and carry no
  section numbers, whatever `numbering` says; each item heading starts
  with `{anchor:REQ-014}`, so `…#REQ-014` and references land on it.
- Status lozenges, callouts, the table of contents and code are the same
  macros the `confluence` format uses (`status`, `info`/`tip`/`note`/
  `warning`, `toc`, `code`); `sections` and `tree` root pages list their
  children with `children`, and links between pages name the page by
  title, so create each page under the title its file shows.
- Diagrams and repository images are written beside the file
  (`diagram-….png`, `image-….png`) and named in the markup: attach them
  to the page for them to show.
- Jira keys are links to `[jira] url`, not the Jira macro: a file can't
  know whether the Confluence it is pasted into has Jira linked.
- Repository links are web links naming a commit, wherever the file is
  written, since they are read in Confluence.

### Adding a format

A back end implements one trait: render a split page (the document tree
plus resolved links) into bytes and attachments. Layouts, templates,
selection, reports, numbering and link resolution come with it.
Destinations are separate: files, or Confluence's planner.

## Running exports

```
kiln export [NAME…] [--dry-run | --check] [--yes] [--force] [--since REV]
            [--require-clean] [--allow-shallow] [--format text|json]
```

- No names: every target. Names: those. A target or template `kiln check`
  calls broken (K005, K112) doesn't export.
- `--dry-run`: render everything, print what would change (Confluence: the
  plan; files: which files would be written), write nothing.
- `--check`: like `--dry-run`, and exit 1 if anything would change. For
  committed file exports in CI.
- File targets write a file only when its bytes change, atomically
  (`fenix_storage::write`), and remove files they wrote earlier that are
  no longer produced (tracked in a `.kiln-export` list in the target's
  folder, written only for targets that produce more than one file; files
  Kiln didn't write are never removed).
- `--dry-run` also lists the open items each target leaves out.
- Confluence targets: see CONFLUENCE.md for `--yes`, `--force`, drift.
  Until K3 they only plan: `--dry-run` (or `--check`) against the live
  space, or `--snapshot FILE` offline; `--save-snapshot FILE` records the
  live snapshot.
- `--since REV` (a tag, branch or commit): the baseline for the `changes`
  report and the in-place markers. The project at REV is read from git
  (its `kiln.toml` and Markdown files), not checked out.
- `link_to` links files to files and Confluence pages to Confluence pages;
  a `link_to` across the two is ignored and the item links to its file in
  the repository.
- Exit codes as in DESIGN.md: 1 means `--check` found changes or a plan
  was refused.

## Determinism

The same project at the same commit exports byte-identical output, on
Windows and on Linux. Everything without an inherent order is sorted;
line endings are normalised; the time of the export never appears (the
summary shows the published commit and that commit's date). The one
exception is rasterised diagrams: their pixels depend on the fonts a
machine has, so an HTML or Word export with diagrams (or Markdown with
`diagrams = "image"`) is identical only between machines with the same
fonts. Picture names are content-addressed by the diagram source, not the
pixels, so they don't change.

### Exports kept in the repository

A file target whose `path` is inside the repository and not ignored by
git (a committed `docs/generated/DESIGN.md`) can't name the commit that
will contain it: that commit doesn't exist when the file is written, and
`kiln export --check` in CI would never pass. Such a target is
**in-tree**:

- links to repository files are relative paths (with `#L<line>` for code
  locations), which GitLab and GitHub resolve at the revision being
  viewed, instead of web URLs naming commits;
- the summary says "Generated by Kiln from the files in this repository"
  instead of naming a commit, and `kiln:value commit` renders as "this
  revision" (`date` as nothing).

Everything else follows the rules below. A file under an ignored folder
(`target/kiln/design.html`) is not in-tree and links to the web like a
Confluence page. Code locations carry line numbers, so an in-tree export
that shows them goes stale when code above them moves; `--check` says so
and `kiln export` brings it back.

### Which commit a link names

Every link into the repository (an item's source, a code location, a
relative link) names **the last commit that changed that file**, not the
commit being exported. Otherwise every merge would change every page that
links to code, and Confluence page history would fill with versions that
change nothing a reader cares about. With this rule, output changes only
when something it shows changed; the summary, which names the exported
commit, is the one place that changes on every new commit.

`kiln-git` gets these in one pass (`git log --format=%H --name-only` over
the project). A shallow clone can't answer for files older than
its boundary, so exports check `git rev-parse --is-shallow-repository`
and fail with "the clone is shallow: fetch full history (GitLab:
GIT_DEPTH: 0; GitHub: fetch-depth: 0)" unless `--allow-shallow`, which
falls back to the exported commit for those files.

## Diagrams

Mermaid blocks are rendered with `fenix-diagram` (`render` in the
`default` theme, then `png` at scale 2 on a white background) when the
format wants pictures: Confluence always, HTML always (as `data:` URIs),
Word always (embedded), Markdown only with `diagrams = "image"`. The
source follows each picture in a collapsed block ("Diagram source"),
except in Word, which has no such block.

**Fonts.** Rasterising needs fonts. `fenix-diagram` loads system fonts
only, and slim CI images may have none, which produces diagrams without
text and no error. Before rasterising, Kiln checks the font database is
non-empty and otherwise fails with "no fonts found for diagrams: install
fonts-dejavu (Debian/Ubuntu), or set diagrams = "source"" (which shows the
source as a code block instead). INTEGRATION.md G2 asks `fenix-diagram`
for a bundled fallback font.

**Caching.** PNGs are cached in Kiln's state folder by
`sha256(source + fenix-diagram rev + scale)`; CI can cache that folder.
The same hash names the picture (`diagram-<12 hex>.png`), so a Confluence
dry run knows every attachment without drawing anything.

## Testing

- **Assembly**: fixture projects × templates → golden document trees
  (as a readable text dump): placement, "appears once", nesting, numbering,
  heading demotion past level 6, `--since` markers.
- **Layouts**: one golden split per layout for the same document; link
  resolution across pages (same page → anchor; other page → page + anchor;
  not exported → `link_to` target or repository link).
- **Back ends**: golden output per construct and for a whole sample
  document, per format; Markdown anchors checked against GitLab's and
  GitHub's slug rules; HTML checked for no external references; Word
  checked part by part (every XML part well formed, every relationship id
  declared, every picture present) with `document.xml` golden, and a
  reference document's styles and page setup taken over; the files open
  and convert in LibreOffice (checked by hand, not in CI).
- **Determinism**: export twice, and on both CI operating systems, and
  compare bytes.
