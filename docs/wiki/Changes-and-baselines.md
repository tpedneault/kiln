This page explains how to see what changed in your design since a
release, a review or any other point in its history. It covers
baselines, `kiln diff`, exports marked with what changed (`kiln export
--since`), the GUI's Changes page, and a release review workflow. Read it
when you need to tell reviewers "here is what is different since v1.0".

Kiln compares **items** (requirements, designs, decisions and so on; see
[Concepts](Concepts)), not lines of text, so the answer reads like
"REQ-003: status proposed → approved", not like a diff of files.

## Baselines

A **baseline** is a git revision: a tag such as `v1.0`, a branch such as
`main`, or a commit such as `2ca625a`. Kiln reads the project as it was
at that revision straight from git: its `kiln.toml` and its Markdown
files. Nothing is checked out, and your working tree is not touched.

Kiln then compares that project with the project now (your working tree,
uncommitted edits included) or with a second revision.

- Only the design is compared, not code. Code references move with every
  refactoring and aren't what a design reviewer reads.
- A revision from before the project had a `kiln.toml` has no items, so
  everything reads as added.
- A revision is always passed to git as a revision, never as an option:
  one that starts with `-` is refused.
- A revision git doesn't know is a usage error (exit code 2).

Tags make the best baselines: they have names people recognise and they
don't move. Tag every release or formal review.

## What counts as a change

Items are matched by ID.

- **Added**: the ID exists only in the newer project.
- **Removed**: the ID exists only in the baseline.
- **Changed**: the ID exists in both and any of these differ:

| Compared | Shown as |
|---|---|
| `status` | `status proposed → approved` |
| `title`, `type`, `parent`, `owner`, `due` | the old and new value: `title "Measure cold start" → "Measure cold start time"`, `owner (none) → alice` |
| `[links]`, each role and target | `+ verifies REQ-002`, `− mitigates RISK-001` |
| `tags` and `jira` keys, as sets | `tags (none) → perf` |
| the body (leading and trailing blank lines ignored) | `body` |
| the file the item lives in | `moved design/tests/TEST-001-measure-cold-start.md → design/tests/TEST-001-measure-cold-start-time.md` |

Some consequences:

- A renumbered item (`kiln renumber REQ-015 --to REQ-016`) reads as one
  removed and one added.
- An item that appears twice with the same ID (a duplicate, an error
  `kiln check` reports) is compared by its first copy on each side.
- Reformatting the front matter without changing a value is not a
  change.

## `kiln diff`

```text
kiln diff REV          compare REV with the working tree
kiln diff REV..REV     compare two revisions
```

Options: `--type TYPE` keeps one type; `--format json` gives the result
as data.

### Against the working tree

In a small project tagged `v1.0`, with some edits since (one requirement
approved, a test added and linked, a body edited, a risk file deleted),
none of them committed yet:

```console
$ kiln diff v1.0
v1.0 (2ca625a) → the working tree
~ REQ-002 Cold start under one second: − mitigates RISK-001; body
~ REQ-003 Works offline: status proposed → approved
+ TEST-002 Run with the network off (test)
- RISK-001 Slow disks on old laptops (risk)
1 added, 2 changed, 1 removed
```

Each line starts with `~` (changed), `+` (added) or `-` (removed). A
changed item lists what changed, separated by `;`.

### Between two revisions

After committing, compare the tag with `HEAD`, keeping only
requirements:

```console
$ kiln diff v1.0..HEAD --type requirement
v1.0 (2ca625a) → HEAD (c934ccd)
~ REQ-002 Cold start under one second: − mitigates RISK-001; body
~ REQ-003 Works offline: status proposed → approved
0 added, 2 changed, 0 removed
```

### As JSON

```console
$ kiln diff v1.0..HEAD --format json
{
  "from": "v1.0",
  "fromCommit": "2ca625af5bf299e52233d42b6bbaf2c2dd05eb97",
  "items": [
    {
      "change": "changed",
      "details": [
        "− mitigates RISK-001",
        "body"
      ],
      "id": "REQ-002",
      "path": "design/requirements/REQ-002-cold-start-under-one-second.md",
      "title": "Cold start under one second",
      "type": "requirement"
    },
    …
    {
      "change": "removed",
      "details": [],
      "id": "RISK-001",
      "path": "design/risks/RISK-001-slow-disks-on-old-laptops.md",
      "title": "Slow disks on old laptops",
      "type": "risk"
    }
  ],
  "to": "HEAD"
}
```

`change` is `added`, `changed` or `removed`; `details` holds the same
phrases as the text form.

### Errors

```console
$ kiln diff v9
kiln: v9: git failed: `v9` isn't a revision in this repository
```

The exit code is 2.

## Exports marked with what changed: `--since`

`kiln export --since REV` produces the normal document, plus two things
for reviewers (see [Templates and exports](Templates-and-exports) for
targets and templates):

1. **In-place markers.** Every item that changed since `REV` is marked
   "Changed", and every new one "New", right where it is in the document,
   so a reviewer reading top to bottom finds the changes in context.
2. **The `changes` report**, if the document includes one: a line of
   totals, then one row per item added, changed or removed, with what
   changed. Removed items are named but not linked, since they are no
   longer in the document.

The markers never become part of the heading text, so anchors, page
titles and links don't move when the baseline does:

| Format | Marker |
|---|---|
| Markdown | in italics on the line under the heading (`*Changed*`), because GitLab and GitHub build anchors from heading text |
| HTML, Confluence | a lozenge inside the heading, after its text (yellow "Changed", green "New" in Confluence) |

Items placed in a `style="row"` table aren't marked; the changes report
lists them.

### Adding a changes report

Without a template, add `changes` to `reports`:

```toml
[[export]]
name = "review"
format = "markdown"
title = "Demo — changes for review"
types = ["requirement", "test"]
reports = ["changes"]
path = "out/review.md"
```

With a template, place it with a directive, and show the baseline in
prose with `kiln:value since`:

```markdown
## About this version

Changes since <!-- kiln:value since -->.

…

## What changed

<!-- kiln:report changes -->
```

`<!-- kiln:value since -->` renders as `v1.0 (2ca625a)`, and as nothing
without `--since`.

### A real example

```console
$ kiln export review --since v1.0
review (markdown, document) → out/review.md
  create out/review.md
  leaves out: DES-001, ADR-001, MS-001, TASK-001
```

An excerpt of `out/review.md`:

```markdown
#### 1.1.1 REQ-002 Cold start under one second

*Changed*

**approved** · under [REQ-001 The app starts quickly](#11-req-001-the-app-starts-quickly) · …

### 1.2 REQ-003 Works offline

*Changed*

**approved** · verified by [TEST-002 Run with the network off](#22-test-002-run-with-the-network-off)

…

### 2.2 TEST-002 Run with the network off

*New*

**draft** · verifies [REQ-003 Works offline](#12-req-003-works-offline)

## A Changes

Since v1.0 (2ca625a): 1 added, 2 changed, 1 removed.

| Item | Change | What |
| --- | --- | --- |
| [REQ-002](#111-req-002-cold-start-under-one-second) Cold start under one second | **Changed** | − mitigates RISK-001; body |
| [REQ-003](#12-req-003-works-offline) Works offline | **Changed** | status proposed → approved |
| [TEST-002](#22-test-002-run-with-the-network-off) Run with the network off | **New** |  |
| RISK-001 Slow disks on old laptops | **Removed** |  |
```

The same target exported without `--since` has no markers, and its
changes report says how to get one:

```markdown
## A Changes

No baseline: export with `kiln export --since REV` to list what changed.
```

### Keep `--since` away from committed exports

A committed export checked with `kiln export --check` in CI must be
generated the same way every time, so don't use `--since` on it. Use
`--since` on a separate target (an HTML or Word file under an ignored
folder, or a Confluence page meant for a review), or name the targets:
`kiln export review --since v1.0`.

## The GUI's Changes page

`kiln gui` (or plain `kiln`) opens Kiln's window; see [The GUI](The-GUI).
Its **Changes** page (`SPC c`) lists the items added, changed and removed
since the baseline, grouped under Added, Changed and Removed, each with
what changed (`status draft → approved; + verifies REQ-002`).

The baseline is the most recent tag, or the revision you give:

```console
$ kiln gui --since v1.0
```

The window follows your edits: within two seconds of saving a file, the
page shows the change. `Enter` opens the item under the cursor in Fenix
(see [Editors and Flint](Editors-and-Flint)).

## A release review workflow

1. **Tag the release** once the design is reviewed and `kiln check`
   passes:

   ```console
   $ git tag v1.0
   $ git push origin v1.0
   ```

2. **Work as usual.** Change items, add new ones, commit. The GUI's
   Changes page (which defaults to the most recent tag) shows what has
   moved since v1.0 while you work.

3. **Before the next review, see what changed:**

   ```console
   $ kiln diff v1.0
   ```

   Read it as a reviewer would. Is every change intended? Does every
   removed item deserve to go? Is anything new still in `draft`?

4. **Send the document with the changes marked.** Export a review target
   with `--since`:

   ```console
   $ kiln export review-docx --since v1.0
   ```

   Reviewers get the whole document with "Changed" and "New" next to the
   items that moved, and a changes appendix. For Confluence, publish a
   review target the same way; each item's marker shows in the page, and
   Confluence's "compare versions" shows the exact text changes between
   publishes (see [Publishing to Confluence](Publishing-to-Confluence)).

5. **After the review, tag again** (`v1.1`), and the next cycle compares
   against it.

In CI, a merge request job can run `kiln diff origin/main` (with full
history fetched) to show what the merge request changes in the design.

See also: [Templates and exports](Templates-and-exports) ·
[The GUI](The-GUI) · [Command reference](Command-reference) ·
[Concepts](Concepts)
