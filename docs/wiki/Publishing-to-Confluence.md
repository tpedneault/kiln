This page explains how to publish your design document to Confluence, so
people who never open the repository (managers, reviewers, other teams)
read the same requirements, decisions and progress the team works from.
It covers configuration, tokens, the first publish, how Kiln plans each
publish, edits made in Confluence, retired pages, size limits, comments,
the Jira macro, and CI. Read [Templates and exports](Templates-and-exports)
first: a Confluence target is an export target like any other, with
`format = "confluence"`.

Publishing is **one way**: the repository is the source, Confluence is a
view. It is also **idempotent**: publishing the same commit twice changes
nothing the second time.

> [!NOTE]
> **What has been tested.** Every Confluence call Kiln makes is tested
> against a fake Confluence server built from Atlassian's documentation,
> for both Cloud and Data Center, including retries, conflicts and
> interrupted publishes. Not every call has yet been confirmed against
> live Confluence Cloud and Data Center instances: Atlassian's API differs
> by flavour and version in ways the documentation doesn't always say.
> Try a first publish under a scratch parent page, and please report
> anything that behaves differently from this page.

## What readers get

### `document` (the default): one page

```text
<parent page you chose>
└── Demo — design document
      banner · summary · table of contents
      1  Introduction                      (template prose)
      2  Requirements
         2.1  REQ-001 The app starts quickly
              approved
              Verified by: TEST-002
              Jira: DEMO-12
              <body>
         2.2  REQ-002 Cold start under one second    (a child: the same level)
      3  Design
      4  Decisions
      A  Traceability
      B  Open findings
```

- A banner at the top (an `info` macro) says the page is published by
  Kiln from the repository at a given commit, that edits belong in the
  repository and are overwritten here, and to comment at the bottom of
  the page.
- Every item heading carries an anchor named by its ID, so
  `…/Demo+-+design+document#REQ-002` links straight to it from
  Confluence, Jira, chat or another document.
- References between items are links to those anchors.
- Each publish that changes anything is **one new page version**, with a
  version message naming what changed, such as
  `kiln 9f8e7d6: REQ-014, ADR-004 changed; REQ-031 new`. Confluence's
  "page history" and "compare versions" become the document's revision
  history.

### `sections`

```text
└── Demo — design document               banner · summary · introduction · list of child pages
    ├── 2 Requirements                   one page per top-level section
    ├── 3 Design
    ├── 4 Decisions
    ├── A Traceability
    └── B Open findings
```

### `tree`

```text
└── Demo — design (browse)
    ├── Requirements
    │   ├── REQ-001 The app starts quickly     one page per item, nested by `parent`
    │   │   └── REQ-002 Cold start under one second
    ├── Decisions
    ├── Traceability
    └── Removed from the repository           items whose files are gone
```

In `tree`, each item page has a banner naming the item's source file and
the last commit that changed it, the full properties table, the body, and
its coverage.

## Configuration

Two places in `kiln.toml`, which is committed, so it never holds a token:
the **connection**, shared by every Confluence target, and each
**target's** own settings.

```toml
[confluence]
url = "https://confluence.example.com"        # Data Center: base URL, with any context path
# url = "https://example.atlassian.net/wiki"  # Cloud
space = "DEMO"

[[export]]
name = "spec"
format = "confluence"
title = "Demo — design document"
template = "design/templates/design-document.md"

[export.confluence]
parent = 123456789          # create the page under this page
```

Find a page's ID in its URL (`pageId=123456789`) or in "Page
information".

### `[confluence]`: the connection

| Key | Default | Meaning |
|---|---|---|
| `url` | required | Data Center: the base URL, including any context path. Cloud: `https://<site>.atlassian.net/wiki`. |
| `space` | required | The space key the targets publish into. |
| `flavour` | `auto` | `auto`, `cloud` or `data-center`. `auto` treats `*.atlassian.net` as Cloud, anything else as Data Center. |
| `user` | `""` | Cloud only: the account e-mail the API token belongs to. `KILN_CONFLUENCE_USER` overrides it. |
| `auth` | `auto` | `auto`, `bearer` or `basic`. `auto` means bearer (personal access token) on Data Center, basic on Cloud. |
| `labels` | `[]` | Labels added to every page Kiln publishes. |
| `jira_macro` | `true` | Render `jira` keys as the Jira macro (see [Jira keys](#the-jira-macro)). |
| `jira_server_id` | `""` | Data Center with several Jira servers linked: which one the macro uses. |
| `restrict_edits` | `false` | Restrict editing of pages Kiln creates to the publishing account (see [Drift](#drift-edits-made-in-confluence)). |
| `on_drift` | `stop` | `stop`, `skip` or `overwrite`: what to do with a page edited in Confluence. |
| `concurrency` | `4` | Requests in flight. Read but not used yet: Kiln currently sends one request at a time. |
| `diagrams` | `image` | `image` (attached PNG pictures) or `source` (Mermaid source as a code block). |
| `max_page_kib` | `1024` | Warn above this page size; refuse above four times it (see [Size limits](#size-limits-and-when-to-switch-layout)). |

`[confluence]` may be absent when no target uses it. A Confluence target
without the keys it needs fails `kiln check` (K005), naming them.

### `[export.confluence]`: one target

| Key | Default | Meaning |
|---|---|---|
| `parent` | — | Create the target's (root) page under this page ID. |
| `page` | — | Instead, write into this existing page (`document` layout only; see [Adopting an existing page](#adopting-an-existing-page)). |
| `space` | `[confluence] space` | Publish this target into another space. |
| `labels` | `[]` | Labels added to this target's pages, on top of `[confluence] labels`. |
| `toc_levels` | `3` | How many heading levels the table of contents (`kiln:toc`) shows. |
| `restrict_adopted` | `false` | With `restrict_edits`, also restrict an adopted page. |
| `qualify_titles` | `taken` | `taken` or `always`: when section and item pages carry the target's title in parentheses (see [Several projects in one space](#several-projects-in-one-space)). |
| `children` | `macro` | `macro` (Confluence's `children` macro) or `list` (links Kiln writes, in document order): how a `sections` or `tree` page lists its child pages. |

Give exactly one of `parent` or `page`. `page` with a layout other than
`document` is a K005 error.

## Credentials

| Flavour | Credential | Sent as |
|---|---|---|
| Data Center 7.9 and later | a personal access token | `Authorization: Bearer <token>` |
| Data Center without personal access tokens | username and password, only with `auth = "basic"` | Basic |
| Cloud | your account e-mail (`user`) and an API token | Basic |

Kiln looks for the token in this order:

1. **`KILN_CONFLUENCE_TOKEN`** (and `KILN_CONFLUENCE_USER`, which overrides
   `user`). Use this in CI, as masked, protected variables.
2. **The OS credential store**: Windows Credential Manager, or the Secret
   Service on Linux (KDE Wallet, GNOME Keyring). Store a token there with
   `kiln auth confluence`.

`kiln auth confluence` asks for the token without echoing it, checks it
against the server, and stores it under service `fenix-suite`, account
`confluence:<url>` (the URL exactly as configured, without a trailing
slash). Flint reads the same entry, so one stored token serves both. On
Cloud, set `[confluence] user` first: the e-mail is configuration, not a
secret, so it isn't stored with the token.

```console
$ kiln auth confluence
Token for https://confluence.example.com:
stored the token for https://confluence.example.com (signed in as Thomas) in the credential store as fenix-suite/confluence:https://confluence.example.com
```

`kiln auth confluence --forget` removes it.

On a headless Linux machine with no Secret Service, the store can't be
read, and Kiln says so:

```console
$ kiln export spec --dry-run
kiln: couldn't read `confluence:https://confluence.example.com` from the credential store: Platform secure storage failure: no secret service provider or dbus session found (or set KILN_CONFLUENCE_TOKEN)
```

A token never appears in a file, a log, the plan output or an error
message. When the server refuses it, the error says "Confluence refused
the token for <url> (run `kiln auth confluence`, or check
KILN_CONFLUENCE_TOKEN)".

Proxies: Confluence calls go through `HTTPS_PROXY` (or `HTTP_PROXY`,
`ALL_PROXY`) and honour `NO_PROXY`, so an internal Confluence listed in
`NO_PROXY` is reached directly.

## Your first publish, step by step

1. **Pick a parent page** in the space, ideally a scratch page for the
   first try, and note its ID.
2. **Configure** `[confluence]` and a target with `[export.confluence]
   parent = <id>` as above, then run `kiln check` to validate it.
3. **Store a token**: `kiln auth confluence`, or set
   `KILN_CONFLUENCE_TOKEN`.
4. **Dry run.** Kiln reads the space (read-only calls) and prints the plan:

   ```console
   $ kiln export spec --dry-run
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     create    "Demo — design document": new (7 KiB)
     1 create, 0 update, 0 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   ```

   The first line names the target, its format and layout, the space and
   the parent page by ID and title: check that it is the page you meant
   before publishing. Then one line per page: the operation, the title, why,
   and the page's size. Then totals, and the open items the document
   leaves out.

5. **Publish.** On a terminal, `kiln export spec` shows the plan and asks
   before publishing. Without a terminal (CI, a script), it stops after
   the plan and exits 1, so a job that forgot `--yes` doesn't pass green
   having published nothing:

   ```console
   $ kiln export spec </dev/null
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     create    "Demo — design document": new (7 KiB)
     1 create, 0 update, 0 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   kiln: `spec` not published: pass --yes to publish without asking
   ```

   `--yes` publishes without asking:

   ```console
   $ kiln export spec --yes
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     create    "Demo — design document": new (7 KiB)
     1 create, 0 update, 0 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   spec: published 1 created, 0 updated, 0 retired, 0 pruned
   ```

6. **Publish again** to see that nothing changes:

   ```console
   $ kiln export spec --yes
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     unchanged "Demo — design document" (7 KiB)
     0 create, 0 update, 1 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   ```

7. **Change an item, commit, publish.** The plan names what changed:

   ```console
   $ kiln set REQ-003 status=verified
   $ git commit -qam "REQ-003 verified"
   $ kiln export spec --yes
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     update    "Demo — design document": REQ-003 changed (7 KiB)
     0 create, 1 update, 0 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   spec: published 0 created, 1 updated, 0 retired, 0 pruned
   ```

   Kiln names changed items by the last commit that changed each file, so
   uncommitted edits show as "content changed (template, layout or
   references)" instead. Publish from commits; CI passes
   `--require-clean` to refuse a dirty working tree.

`kiln export` with no name runs every target, file targets included.
`--format json` prints the plan as data (and, with `--yes`, what was
published under `applied`), for CI artefacts.

## Adopting an existing page

Sometimes a page already exists with the right place, permissions and
links pointing at it. Write into it with `page` instead of `parent`
(`document` layout only):

```toml
[export.confluence]
page = 987654321
```

The first publish refuses unless the page is empty or you pass
`--adopt`. With `--adopt`, its body is replaced (the old body stays in
the page history) and the page is Kiln's from then on. Kiln never changes
an adopted page's title, parent or restrictions unless configured to.

## Page identity: why targets never collide

Kiln stores a small `kiln` content property on every page it publishes,
naming the project, the export target and a page key (`doc` for the root,
`section:requirements`, `item:REQ-014`, `report:traceability`, and so
on). A page's identity is that triple, not its title or position.

So two targets can share a parent page, a space, even item IDs, and never
touch each other's pages. Kiln only looks under the target's parent (or
the adopted page). Pages there without this target's property are
**foreign**: never modified, moved or deleted, and listed in the plan.

Page titles in a space are unique. The root page takes the target's
`title`; section pages are `<number> <heading>` and item pages
`<ID> <title>`. When the space already has a page with that title (another
target's, or a page someone else made), Kiln appends the target's title in
parentheses. Titles are capped at 255 characters, shortening the item
title with `…`, never the ID.

### Several projects in one space

Two projects using the same template, such as two arc42 documents, both
have a page called "1 Introduction and Goals". By default the first to
publish gets that title and the second gets "1 Introduction and Goals
(Basket service — architecture)". That works, but which project gets
which depends on who published first, and if the first project's page
goes away, the second project's page is renamed on its next publish,
which breaks links people saved.

For a space several projects publish to, set it once and for all:

```toml
[export.confluence]
parent = 123456789
qualify_titles = "always"   # every section page carries this target's title, from the first publish
```

Every section and item page then carries the target's title, so its title
depends only on its own project. The root page is the target's `title`
itself, so give each project's document a title with its name in it (the
configurations on [Presets](Presets) do: `Shop — architecture`).

When a title can't be made unique, because the root's title is taken or
two targets share the same `title`, the plan stops before anything is
written and names the title:

```console
$ kiln export arc42-confluence
kiln: the title "Architecture" is held by another page in the space, which this export doesn't own; give the export another `title`
```

The order of child pages: a `sections` page lists its children with
Confluence's `children` macro, in the order Confluence keeps the page
tree. If that comes out alphabetical, "10 Quality Requirements" lands
between "1 …" and "2 …". `children = "list"` has Kiln write the list
itself, in document order.

## The plan: create, update, unchanged, drift, retire

Each publish compares what the target would publish with what is in
Confluence, page by page, and plans one operation per page:

| Operation | When |
|---|---|
| `create` | the page doesn't exist yet |
| `unchanged` | the page exists and what Kiln would write is identical; costs no write |
| `update` | the page exists and something differs (body, title, parent, labels, attachments) |
| `drift` | the page was edited in Confluence since Kiln last wrote it (see [Drift](#drift-edits-made-in-confluence)) |
| `retire` | a page of this target that is no longer produced (in `tree`, a removed item; in any layout, a section that went away) |

A retired page that is produced again is updated back into place ("back
in the repository"). With `--prune-removed DAYS`, old retired pages are
also pruned (sent to the trash).

Kiln applies the plan in a safe order: new pages parent first, then
updates and moves, then labels and attachments, then retires, then the
root page last, so the commit the root page names means everything under
it is at that commit. A publish that stops half-way (a network error,
Ctrl-C) leaves what it applied applied; the next run picks up from there
and ends in the same state as an uninterrupted run.

`--yes` on a plan more than two minutes old reads the space again first.

## Drift: edits made in Confluence

**Drift** means someone edited a published page in Confluence. Kiln
detects it when the page's version is newer than the one it published
**and** the body differs from what it wrote. A version bump with the same
body (an admin moving the page, say) is not drift.

By default (`on_drift = "stop"`) a drifted page stops the whole target:

```console
$ kiln export spec --yes
spec (confluence, document) → space DEMO, page 123456789 "Demo"
  drift     "Demo — design document": drift: edited by alice at 2026-10-10T14:02:00.000Z, version 4 (7 KiB)
  0 create, 0 update, 0 unchanged, 1 drift, 0 retire
  refused: "Demo — design document" was edited in Confluence (edited by alice at 2026-10-10T14:02:00.000Z, version 4); copy the edit into the repository, then run again with --force, or set on_drift = "skip"
  leaves out: TASK-001, TEST-001, TEST-002
```

Nothing of that target is published, and the exit code is 1. To resolve
it:

1. Open the page history in Confluence, find the version and author the
   plan names, and read what changed.
2. Carry the edit into the item files by hand, if it is worth keeping,
   and commit.
3. Publish with `--force`, which overwrites the edited page:

   ```console
   $ kiln export spec --yes --force
   spec (confluence, document) → space DEMO, page 123456789 "Demo"
     update    "Demo — design document": drift: edited by alice at 2026-10-10T14:02:00.000Z, version 4; overwritten (7 KiB)
     0 create, 1 update, 0 unchanged, 0 drift, 0 retire
     leaves out: TASK-001, TEST-001, TEST-002
   spec: published 0 created, 1 updated, 0 retired, 0 pruned
   ```

`on_drift` choices:

| `on_drift` | Does |
|---|---|
| `stop` (default) | refuses the whole plan until the edit is dealt with |
| `skip` | leaves drifted pages alone and publishes everything else |
| `overwrite` | always overwrites, as `--force` does once |

With the `document` layout, drift anywhere blocks the whole document,
since it is one page. **`restrict_edits = true`** prevents most drift: it
adds an edit restriction for the publishing account to each page Kiln
creates (not to adopted pages unless `restrict_adopted = true`). It is off
by default because some organisations disallow restrictions or need
admins to keep edit rights.

## Retired pages and `--prune-removed`

When a page is no longer produced (an item file deleted in a `tree`
target, a section that went away, or the item pages left over after
switching from `tree` to `document`), Kiln doesn't delete it. It
**retires** it: moves it under a "Removed from the repository" page
(created on demand under the root), replaces its body with a warning
banner and the last published content, and adds the label `kiln-removed`.
Links to it keep working.

```console
$ git rm -q design/designs/DES-001-lazy-loading-of-plugins.md
$ git commit -qam "Drop the design"
$ kiln export spec --yes
spec (confluence, tree) → space DEMO, page 123456789 "Demo"
  update    "Demo — design document": content changed (template, layout or references) (1 KiB)
  …
  unchanged "REQ-001 The app starts and responds quickly" (1 KiB)
  …
  retire    "DES-001 Lazy loading of plugins": no longer produced: moved under "Removed from the repository"
  0 create, 9 update, 2 unchanged, 0 drift, 1 retire
  leaves out: TASK-001, TEST-001, TEST-002
spec: published 0 created, 9 updated, 1 retired, 0 pruned
```

To clean up, `kiln export --prune-removed DAYS` sends pages retired more
than `DAYS` days ago to the space's trash, where a space admin can still
restore them.

## Size limits and when to switch layout

A whole design document on one page can grow large, and large pages are
slow to open and edit in Confluence. After rendering, Kiln measures each
page's storage size (attachments such as diagrams don't count):

- above `max_page_kib` (default 1024 KiB) the plan shows a warning;
- above four times `max_page_kib` it refuses that page, and nothing of
  the target is published (exit 1).

```text
  warning: "Demo — design document" is 5 KiB, over max_page_kib (2 KiB): large pages are slow to open and edit; consider layout = "sections"
```

```text
  refused: "Demo — design document" is 5 KiB, over four times max_page_kib (1 KiB); use layout = "sections"
```

(These came from a deliberately tiny `max_page_kib`.) When you see the
warning, switch to `layout = "sections"`: one page per top-level section.
The old root page stays the root, and the plan creates the section pages.
Choose `tree` instead when people mostly look up single items.

The size at which a page becomes slow in practice depends on your
Confluence. If your server rejects a page as too large, the error names
the page and its size and recommends `layout = "sections"`.

## Comments

Confluence has two kinds of comments:

- **Footer comments** (at the bottom of the page) live outside the page
  body and survive every publish.
- **Inline comments** anchor to markers inside the body. A publish
  rewrites the body, which drops those markers and orphans the comments.

That is why the banner asks readers to comment at the bottom. It matters
most with the `document` layout, where every change rewrites the one page
everyone comments on.

`kiln confluence comments` lists the unresolved footer comments on each
Confluence target's pages (or only the targets you name), each under the
item it is about when Kiln can tell, with a link that opens it:

```console
$ kiln confluence comments
spec → space FLINT: 2 unresolved comments
  REQ-014   Alice Martin, 2026-10-12: Startup in 50 ms Is that measured cold?
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=456#comment-456
  (page)    Bob Chen, 2026-10-13: Can we get a PDF of this?
            on "Flint — design document"
            https://confluence.example.com/pages/viewpage.action?pageId=123&focusedCommentId=457#comment-457
```

(That example is from Kiln's own documentation. With no comments you
get `spec → space DEMO: 0 unresolved comments`.)

- On an item's own page (`tree`), a comment is about that item.
- On a page holding many items, a comment that quotes the page (with
  Confluence's "quote") is placed under the nearest item above the quoted
  text; any other comment is about the page, shown as `(page)`.
- Replies are listed too; resolved comments and retired pages are left
  out.
- Authors are display names on Data Center and account IDs on Cloud.

It needs the same token as publishing, only reads, and supports
`--format json`.

## The Jira macro

Items can name Jira tickets with a `jira` key (see [Jira](Jira)). In
Confluence each key renders as:

- the **Jira macro** when `jira_macro = true` (the default). It shows the
  ticket's live status to readers. It needs an application link between
  Confluence and Jira; on Data Center with several Jira servers linked,
  set `[confluence] jira_server_id`;
- with `jira_macro = false`, a link to `<[jira] url>/browse/<KEY>`, or
  plain text when `[jira] url` isn't set.

## Snapshots: planning offline

A **snapshot** is a JSON record of what a target's pages look like in
Confluence. It lets you plan without a token or a network, for tests,
demos, or reviewing a plan on a machine that can't reach Confluence.

- `kiln export spec --dry-run --save-snapshot live.json` writes the
  snapshot a live dry run read.
- `kiln export spec --dry-run --snapshot live.json` plans against it,
  offline. `--snapshot` works only with `--dry-run` or `--check`.

The smallest snapshot, a parent page with nothing published under it yet:

```json
{"anchorPage": 123456789}
```

```console
$ kiln export spec --dry-run --snapshot first.json
spec (confluence, document) → space DEMO, page 123456789 "Demo"
  create    "Demo — design document": new (7 KiB)
  1 create, 0 update, 0 unchanged, 0 drift, 0 retire
  leaves out: TASK-001, TEST-001, TEST-002
$ kiln export spec --snapshot first.json
kiln: --snapshot plans offline; publishing reads the live space (add --dry-run)
```

## Running it in CI

A typical pipeline:

- on every merge request, a **dry run** with `--format json` saved as an
  artefact, so reviewers see what will change in Confluence (a read-only
  token is enough);
- on the default branch, a **publish** with `kiln export --require-clean
  --yes`.

Three things matter:

- **Full history.** Links name the last commit that changed each file, so
  CI must fetch full history: `fetch-depth: 0` (GitHub Actions),
  `GIT_DEPTH: 0` (GitLab). Otherwise exports stop with "the clone is
  shallow".
- **Serial publishes.** Kiln doesn't lock (Confluence has no way to), so
  run publishes one at a time: `concurrency: confluence` in GitHub
  Actions, `resource_group: confluence` in GitLab. An accidental overlap
  fails safely on a conflict rather than corrupting anything.
- **Fonts.** Diagrams are drawn as pictures for Confluence; a slim image
  needs a font package (see [Templates and exports](Templates-and-exports#diagrams)).

GitLab:

```yaml
export-plan:
  stage: test
  rules: [{ if: $CI_PIPELINE_SOURCE == "merge_request_event" }]
  variables:
    GIT_DEPTH: 0
  script:
    - kiln export --dry-run --format json > export-plan.json
  artifacts:
    paths: [export-plan.json]

export-publish:
  stage: deploy
  rules: [{ if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH }]
  resource_group: confluence
  variables:
    GIT_DEPTH: 0
  script:
    - kiln export --require-clean --yes
```

Set `KILN_CONFLUENCE_TOKEN` (and, on Cloud, `KILN_CONFLUENCE_USER`) as
masked, protected CI variables. Complete files for GitHub Actions and
GitLab are on [Continuous integration](Continuous-integration), and in the
repository as
[contrib/github-workflow.yml](https://github.com/tpedneault/kiln/blob/master/contrib/github-workflow.yml)
and
[contrib/gitlab-ci.yml](https://github.com/tpedneault/kiln/blob/master/contrib/gitlab-ci.yml).

In CI there is no page cache, so Kiln reads every page's property: one
call for a `document`, one per page for a `tree`. A tree of hundreds of
pages takes tens of seconds to plan. On a desktop, a cache in Kiln's
state folder (`state/confluence/<space>-<project>-<export>.json`) makes
later runs cost a call per changed page.

## Errors and exit codes

Errors are one line on stderr starting with `kiln:`.

| Exit code | Means, for Confluence |
|---|---|
| 0 | published, or the dry run finished |
| 1 | the plan was refused (drift with `on_drift = "stop"`, a page too large, an adopted page with content and no `--adopt`), or no `--yes` and no terminal; nothing of that target was published |
| 2 | configuration or usage: a broken target (K005, K112), the credential store can't be read, `--snapshot` without `--dry-run`, `--require-clean` on a dirty tree, a shallow clone |
| 3 | a network or server error; the next run finishes what this one started |

Server behaviour Kiln handles for you:

- **429 Too Many Requests** and **503**: waits for `Retry-After` when the
  server sends it, otherwise backs off from 1 s, up to 6 attempts.
- **409 Conflict** (the page changed since Kiln read it): reads it again
  and retries once; a second conflict fails naming the page.
- **413** or a size error: names the page and its size and recommends
  `layout = "sections"`.
- Timeouts: 10 s to connect, 60 s per request (120 s for attachments and
  large bodies).

See also: [Templates and exports](Templates-and-exports) ·
[Continuous integration](Continuous-integration) · [Jira](Jira) ·
[Troubleshooting and FAQ](Troubleshooting)
