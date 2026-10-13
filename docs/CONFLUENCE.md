# Publishing to Confluence

The `confluence` export format puts a Kiln document into a Confluence
space, so people who never open the repository (managers, reviewers,
other teams, customers with Confluence access) read the same requirements,
decisions and progress the team works from. By default that is **one
page holding the whole design document**; the `sections` and `tree`
layouts spread it over a page tree instead ([EXPORT.md](EXPORT.md),
"Layouts").

It is a one-way, idempotent publisher: the repository is the source,
Confluence is a view, and publishing the same commit twice changes
nothing the second time.

Implemented in `kiln-confluence`, on top of `kiln-export`'s assembly and
layouts. Endpoint details marked **verify** were checked against
Atlassian's documentation and community answers on 2026-10-12 but must be
confirmed against live Cloud and Data Center instances in K2/K3 before
being relied on; record what was found here.

## What readers get

### `document` (default): one page

```
<parent page you chose>
└── Flint — design document
      banner · summary · table of contents
      1  Introduction                      (template prose)
      2  Requirements
         2.1  REQ-001 Flint is a launcher…
              approved · verified by TEST-002 · Jira FLINT-12
              <body>
              2.1.1  REQ-002 Fast enough to be a reflex
                     2.1.1.1  REQ-014 Flint appears within 50 ms of the hotkey
      3  Design
      4  Decisions
      A  Traceability
      B  Open findings
```

- A banner at the top (Confluence `info` macro): "Published by Kiln from
  `<repository>` at `9f8e7d6` (2026-10-12). Edit it in the repository;
  changes made here are overwritten. Comment at the bottom of the page."
- Every item heading carries an anchor named by its ID, so
  `…/Flint+-+design+document#REQ-014` links straight to it, from
  Confluence, Jira, chat or another document.
- References between items in the same document are links to those
  anchors.
- Each publish that changes anything is one new page version, with a
  version message naming what changed: `kiln 9f8e7d6: REQ-014, ADR-004
  changed; REQ-031 new (+3 more)`. Confluence's "page history" and
  "compare versions" become the document's revision history.

### `sections`

```
└── Flint — design document               banner · summary · introduction · children list
    ├── 2 Requirements                     one page per top-level section
    ├── 3 Design
    ├── 4 Decisions
    ├── A Traceability
    └── B Open findings
```

### `tree`

```
└── Flint — design (browse)
    ├── Requirements
    │   ├── REQ-001 Flint is a launcher…  one page per item, nested by `parent`
    │   │   └── REQ-002 Fast enough to be a reflex
    │   │       └── REQ-014 Flint appears within 50 ms of the hotkey
    ├── Decisions
    ├── Traceability
    └── Removed from the repository        items whose files are gone
```

In `tree`, an item page has the banner (naming the item's source file and
the last commit that changed it), the full properties table
(`item_style = "table"`), the body, and its coverage.

## Configuration

Two places in `kiln.toml` (committed, so no secrets): the connection,
shared by every Confluence target, and each target's own settings.

```toml
[confluence]                          # the connection
url = "https://confluence.example.com"     # Data Center base, including any context path
# url = "https://example.atlassian.net/wiki" # Cloud
flavour = "auto"            # auto | cloud | data-center; auto: *.atlassian.net is cloud
space = "FLINT"
user = ""                   # Cloud only: the account e-mail the token belongs to
auth = "auto"               # auto | bearer | basic; auto: bearer on Data Center, basic on Cloud
labels = ["flint"]          # added to every page Kiln publishes
jira_macro = true           # the Jira macro for `jira` keys (needs a Jira application link)
jira_server_id = ""         # Data Center with several Jira links (see "Jira")
restrict_edits = false      # restrict editing of published pages to the publishing account
on_drift = "stop"           # stop | skip | overwrite (see "Drift")
concurrency = 4             # requests in flight
diagrams = "image"          # image | source (EXPORT.md, "Diagrams")
max_page_kib = 1024         # warn above this storage size; refuse above 4× (see "Size")

[[export]]
name = "spec"
format = "confluence"
title = "Flint — design document"
template = "design/templates/design-document.md"

[export.confluence]                   # this target
parent = 123456789          # create the (root) page under this page…
# page = 987654321          # …or write into this existing page (document layout only; see "Adopting a page")
space = "FLINT"             # optional: another space than [confluence] space
labels = ["design-doc"]     # added to this target's pages
```

`[confluence]` may be absent when no target uses it; a target with
`format = "confluence"` then fails `kiln check` (K005) naming the missing
keys.

## Credentials

| Flavour | Credential | Sent as |
|---|---|---|
| Data Center 7.9+ | personal access token | `Authorization: Bearer <token>` |
| Data Center, no PATs | username + password (only with `auth = "basic"`) | Basic |
| Cloud | account e-mail (`user`) + API token | Basic |

Looked up in this order:

1. `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER` overriding `user`):
   for CI, set as masked protected variables.
2. The OS credential store through `keyring`, using the suite's naming
   (Flint's F12, already implemented by Flint): service `fenix-suite`,
   account `confluence:<url>` with the URL exactly as configured, trailing
   slash removed. `kiln auth confluence` prompts without echo, checks the
   token against the server (`whoami`), then stores it.

A token is never written to a file, a log, the plan output or an error
message. A 401 says "Confluence refused the token for <url> (run `kiln
auth confluence`, or check KILN_CONFLUENCE_TOKEN)".

## Pages and their identity

`kiln-export` hands this back end a list of **pages**, each with a key,
a title, a parent key, a document tree with resolved links, and its
attachments. Keys are stable across publishes:

| Layout | Keys |
|---|---|
| `document` | `doc` |
| `sections` | `doc` (the root), `section:<slug>` per top-level section, from its heading text (`section:requirements`), `report:<name>` |
| `tree` | `doc`, `section:<slug>`, `item:<ID>`, `report:<name>`, `report:<name>:<ID>` (split reports), `removed` |

A page's identity in Confluence is `(project, export name, key)`, stored
in its `kiln` content property. Two targets can share a parent page, a
space, even item IDs, and never touch each other's pages.

Titles: the root page is the target's `title`. Section pages are
`<number> <heading>` and item pages `<ID> <title>`; when the space already
has a page with that title (another target's, or a foreign page), the
target's `title` is appended in parentheses. Titles are capped at 255
characters, truncating the item title with `…`, never the ID.

### Adopting a page

With `[export.confluence] page = <id>` (document layout only), Kiln writes
into a page that already exists, which is how a document gets the place
and the permissions someone has already set up for it. The first publish
refuses unless the page is empty or `--adopt` is given; with `--adopt`,
its existing body is replaced (it stays in the page's history) and the
page becomes Kiln's from then on. Kiln never changes an adopted page's
title, parent or restrictions unless configured to.

### Size

A whole design document on one page can grow large. After rendering,
Kiln measures each page's storage size: above `max_page_kib` (default
1024 KiB) the plan shows a warning recommending `layout = "sections"`;
above four times that, the publish refuses that page. Large pages are
slow to open and edit in Confluence well before any hard limit; what the
hard limits are on each flavour is **verify** for K3 (record them here).
Attachments (diagrams) don't count toward the page's size.

## How a publish runs

```
kiln-export (pure) ──► desired pages
                                     ╲
                                      plan (pure) ──► ops ──► apply ──► Confluence
                                     ╱                         │
snapshot (reads) ──► remote pages                            read-back, property
```

### 1. Render

`kiln-export` assembles and splits the document (EXPORT.md); this back end
renders each page to storage format (see "Storage format") and computes
its `renderHash` over title, parent key, body, labels and the set of
attachment hashes. Refuses on a dirty working tree with `--require-clean`
(CI passes it).

### 2. Snapshot

Read what's there, under the target's parent (or the adopted page) only;
Kiln never looks at or touches other pages:

1. Find the root page: among `parent`'s children, the one whose `kiln`
   property names this project and export with key `doc` (the local cache
   remembers its id); with `page =`, that page. First publish: absent.
2. For `sections` and `tree`: list the root's descendants with id, title,
   parent id, version number and version author.
3. Read each page's `kiln` property, but only when its version differs
   from the local cache (state folder,
   `confluence/<space>-<project>-<export>.json`: page id → version,
   property). In CI there is no cache and every property is read; at
   concurrency 4 that is a few seconds for hundreds of pages, and one call
   for a document.
4. Pages under the root without this target's property are **foreign**:
   never modified, moved or deleted, and listed in the plan.

### 3. Plan

A pure function of desired and remote state, unit-tested table-driven.
For each desired page, matched to a remote page by its key:

| Situation | Op |
|---|---|
| no remote page | `create` |
| remote exists, `renderHash` equal | `unchanged` (no call) |
| remote exists, something differs, no drift | `update` (and `move`, `label`, `attach`, `unattach` as needed) |
| remote exists, drift (below) | per `on_drift`: `stop` refuses the whole plan; `skip` leaves that page; `overwrite` (or `--force`) updates it |
| a remote page of this target with no desired page (`tree`: a removed item; any layout: a section that went away) | `retire`: move under "Removed from the repository" (created on demand under the root), replace the body with a banner and the last published content, label `kiln-removed` |
| title collides and can't be disambiguated | error naming the pages; nothing applied |

For `document` the plan is a single line: `unchanged`, or `update` with
the items that changed.

`--dry-run` prints the plan and exits: one line per op with page title and
reason ("REQ-014 changed: status, body", "parent REQ-002 → REQ-003",
"drift: edited by alice at 2026-10-10 14:02, version 7"), then totals and
each page's size. `--format json` gives the plan as data, for CI
artifacts. Without `--yes`, an interactive run shows the totals and asks
before applying; non-interactive runs without `--yes` stop after the plan.

### 4. Apply

Order: create pages parent-first, then updates and moves, then labels and
attachments, then retires, then the root page last (so the commit the
root page names means everything under it is at that commit). Each
page's property is written **after** its body. Attachments are uploaded
before the body that references them.

After each create or update, read the page's storage body back, normalise
it (see "Drift") and store its hash in the property as `remoteHash`. That
makes drift detection independent of whatever Confluence does to storage
on save.

`--yes` on a plan whose snapshot is older than 2 minutes re-snapshots
first.

### The `kiln` content property

Key `kiln`, value (JSON, well under the property size limit; comments
here are for the reader only):

```jsonc
{
  "schema": 1,
  "project": "Flint",
  "export": "spec",
  "key": "doc",                            // or "item:REQ-014", "section:requirements", …
  "items": { "REQ-014": "a1b2c3d4…", … },  // items on this page → last commit that changed each
  "published": "9f8e7d6c5b4a…",            // commit of the publish that last wrote this page
  "renderHash": "sha256:…",
  "remoteHash": "sha256:…",
  "publishedVersion": 8,
  "attachments": { "diagram-3f9a1c2b7e44.png": "sha256:…" }
}
```

`items` is how the plan and the version message name what changed on a
page holding many items without fetching its old body. For a large
document it is the largest field; if it nears the property size limit
(**verify** the limit), store it as a gzip+base64 string instead.

## Drift

Someone edited a published page in Confluence. Detected when the page's
current version is newer than `publishedVersion` **and** the normalised
hash of its current body differs from `remoteHash` (a version bump with an
identical body, e.g. a page move by an admin or a property-only change,
is not drift). **verify**: whether writing a content property, adding a
label or uploading an attachment bumps the page version on each flavour;
`publishedVersion` must be the version after Kiln's last write of any
kind to the page.

- The plan names the page, the version and its author and time, and links
  to Confluence's comparison of that version with Kiln's last, so the edit
  can be read and carried into the repository by hand.
- `on_drift = "stop"` (the default) refuses to publish anything until the
  edit is dealt with: re-run with `--force` after copying it into the repo,
  or set `skip` to publish everything else.
- With the `document` layout, drift anywhere blocks the whole document:
  one popular page attracts more stray edits than many small ones. Use
  `restrict_edits = true` for documents when the organisation allows it.
- `restrict_edits = true` adds an edit restriction for the publishing
  account to each page Kiln creates (not to adopted pages unless
  `[export.confluence] restrict_adopted = true`), which prevents most
  drift. Off by default: some organisations disallow restrictions or need
  admins to keep edit rights. (**verify** the restrictions endpoints on
  both flavours.)

**Normalising storage** before hashing: parse as XML; drop
`ac:macro-id`, `ac:local-id`, `local-id` and `data-*` attributes; drop
`ac:inline-comment-marker` elements (keeping their text); sort attributes;
collapse whitespace between block elements; serialise canonically. The
normaliser is tested on bodies captured from both flavours.

## Comments

Footer comments live outside the page body and survive every update.
Inline comments anchor to markers in the body; Kiln's update drops those
markers and orphans the comments, so the banner asks readers to comment at
the bottom. This matters more for `document`, where every change rewrites
the page everyone comments on. A later improvement (not before K6): carry
existing `ac:inline-comment-marker` elements over into the new body
wherever the marked text still appears verbatim in the same block.

`kiln confluence comments [EXPORT]` (K6) lists unresolved footer comments
per page, and for a document per nearest item heading when the comment
quotes text (**verify** what the API returns for that), so feedback from
Confluence readers reaches the team.

## Storage format

The back end turns each page's document tree into Confluence storage
format (XHTML with `ac:`/`ri:` elements). It always produces well-formed
XML (a test parses every golden file), escapes all text, and never passes
raw HTML through.

| Document node | Storage |
|---|---|
| headings, paragraphs, emphasis, strong, strikethrough, inline code | `h1`–`h6`, `p`, `em`, `strong`, `del`, `code`; numbering as text in the heading |
| item heading | the heading, preceded by the `anchor` macro named by the ID (`<ac:structured-macro ac:name="anchor"><ac:parameter ac:name="">REQ-014</ac:parameter></ac:structured-macro>`, **verify** the default-parameter form on both flavours) |
| properties line / table | a `p` of status lozenge and links / a two-column `table` |
| "Changed" / "New" markers (`--since`) | `status` macro, Yellow / Green, after the heading text |
| lists, GitHub task lists | `ul`/`ol`; task items as text `☐`/`☑` (Confluence task lists are interactive, which invites edits) |
| tables, `style="row"` item tables | `table`/`tbody`/`tr`/`th`/`td` |
| fenced code | `code` macro with `language` parameter, body in CDATA (split any `]]>`) |
| Mermaid diagram | attached as `diagram-<sha256 first 12 hex>.png`, shown with `ac:image` + `ri:attachment`; the source in a collapsed `expand` macro titled "Diagram source" (EXPORT.md, "Diagrams") |
| a diagram that fails to render | a `warning` macro with the error and line, and the source in a code macro; also finding K111 |
| reference, same page | `<ac:link ac:anchor="REQ-014">` with a plain-text body "REQ-014 Title" or the custom text |
| reference, another page of this target or of a `link_to` target | `<ac:link ac:anchor="REQ-014"><ri:page ri:content-title="…" ri:space-key="…"/>…</ac:link>` (no anchor for a `tree` item page) |
| reference to an item exported nowhere | text, plus a link to its file in the repository |
| callouts `[!NOTE]` … | `info` (NOTE, IMPORTANT), `tip` (TIP), `note` (WARNING), `warning` (CAUTION) macros |
| relative links to repo files, code locations | `a href` to the repository web URL (EXPORT.md, "Which commit a link names") |
| relative images | attached (content-addressed name) and shown with `ac:image` |
| status | `status` macro, `colour` by group: initial Grey, done Green, closed Grey, others Blue; gaps Red |
| table of contents (`kiln:toc`) | the `toc` macro (`maxLevel` 3 by default; `[export.confluence] toc_levels`) |
| `sections`/`tree` root's list of child pages | the `children` macro (**verify** parameters; fall back to a generated list) |

Because cross-page links go by title, a title change re-renders every page
that links to the renamed item; the planner sees those as body changes and
updates them in the same run.

## Jira

With `jira_macro = true`, each `jira` key renders as the Jira macro
(`ac:structured-macro ac:name="jira"` with `key`), which shows the live
status to readers and needs an application link between Confluence and
Jira. On Data Center with several Jira servers linked the macro also needs
a `serverId`; set `[confluence] jira_server_id`. With `jira_macro = false`,
keys render as links to `<[jira] url>/browse/<KEY>` (FORMAT.md), or as
plain text when no Jira URL is configured. (**verify** the macro's
parameters on both flavours.)

## The API, by flavour

One trait, `Confluence`, with two implementations. Every call Kiln makes is
listed here; nothing else is used.

| Operation | Data Center (`<url>/rest/api`) | Cloud (`<url>` = `https://<site>.atlassian.net/wiki`) |
|---|---|---|
| who am I | `GET /user/current` | `GET /rest/api/user/current` |
| page by id (title, version, parent, space) | `GET /content/{id}?expand=version,ancestors,space` | `GET /api/v2/pages/{id}` |
| page body (storage) | `GET /content/{id}?expand=body.storage,version` | `GET /api/v2/pages/{id}?body-format=storage` |
| children of a page | `GET /content/{id}/child/page?expand=version` (paged) | `GET /api/v2/pages/{id}/children` (paged) |
| descendants of the root | `GET /content/{id}/descendant/page?expand=version,ancestors&limit=…` (paged) | `GET /api/v2/pages/{id}/descendants` (paged; **verify** depth and fields), else children recursively |
| page by title in space | `GET /content?spaceKey=…&title=…&type=page` | `GET /api/v2/pages?space-id=…&title=…` (space id from `GET /api/v2/spaces?keys=…`) |
| create page | `POST /content` `{type:"page", title, space:{key}, ancestors:[{id}], body:{storage:{value, representation:"storage"}}}` | `POST /api/v2/pages` `{spaceId, status:"current", title, parentId, body:{representation:"storage", value}}` |
| update page | `PUT /content/{id}` with `version:{number: n+1, message}` (and `ancestors` to move) | `PUT /api/v2/pages/{id}` `{id, status:"current", title, body, version:{number: n+1, message}}` |
| move page | `ancestors` on update | `PUT /rest/api/content/{id}/move/append/{targetId}` (v1, **verify** still served) or `parentId` on the v2 update (**verify**) |
| read property | `GET /content/{id}/property/kiln` | `GET /api/v2/pages/{id}/properties?key=kiln` |
| write property | `POST`/`PUT /content/{id}/property/kiln` with `version:{number}` on update | `POST /api/v2/pages/{id}/properties`, `PUT /api/v2/pages/{id}/properties/{property-id}` with `version:{number}` |
| labels | `GET`/`POST /content/{id}/label`, `DELETE /content/{id}/label/{name}` | read `GET /api/v2/pages/{id}/labels`; write with v1 `POST /rest/api/content/{id}/label` and `DELETE /rest/api/content/{id}/label/{name}` (v2 has no label writes; Atlassian is not adding them, CONFCLOUD-76866) |
| list attachments | `GET /content/{id}/child/attachment` | `GET /api/v2/pages/{id}/attachments` |
| upload attachment | `POST /content/{id}/child/attachment`, multipart field `file`, header `X-Atlassian-Token: no-check` | `POST /rest/api/content/{id}/child/attachment` (v1; v2 attachments are read-only), same header and field |
| delete attachment | `DELETE /content/{attachmentId}` | `DELETE /api/v2/attachments/{id}` |
| trash a retired page | `--prune-removed` sends retired pages older than N days to the trash: `DELETE /content/{id}` | `DELETE /api/v2/pages/{id}` |
| restrictions | `PUT /content/{id}/restriction` (**verify**) | `PUT /rest/api/content/{id}/restriction` (**verify**) |
| footer comments (K6) | `GET /content/{id}/child/comment?expand=body.storage,extensions.resolution` (**verify**) | `GET /api/v2/pages/{id}/footer-comments` |

Notes:

- Attachments use content-addressed names, so Kiln only ever creates and
  deletes them, never uploads a new version. That avoids the attachment
  update endpoints, which differ the most between flavours. A document
  with many diagrams has them all on one page; unreferenced ones are
  deleted after the body that stopped referencing them is saved.
- `ureq` 2 has no multipart support; build the multipart body by hand (one
  part, a boundary that doesn't occur in the bytes). Tested against the
  fake server's parser.

### Errors, limits, retries

- **409 Conflict** on update (someone or another publish changed the page
  since the snapshot): re-read that page; if it is now drift, apply
  `on_drift`; otherwise recompute that page's op once and retry. A second
  409 fails the publish naming the page.
- **413** or a size error: the page is too large for the server; the
  error names the page and its size and recommends `layout = "sections"`.
- **429 Too Many Requests** and **503**: wait for `Retry-After` (seconds)
  when present, otherwise exponential backoff from 1 s with jitter, at most
  6 attempts per call, then fail with the endpoint named.
- Concurrency: at most `concurrency` requests in flight (default 4).
- Timeouts: 10 s connect, 60 s per request (120 s for attachments and for
  bodies over 1 MiB).
- Any 4xx other than these fails the publish with the server's message.
  Ops already applied stay applied; the next run's plan picks up from
  there, because every op is idempotent with respect to the desired state.

### Serial publishes

Two publishes of the same target at the same time would both see the
same snapshot. Kiln does not lock (Confluence has no primitive for it);
instead, run publishes serially: GitLab `resource_group: confluence`,
GitHub Actions `concurrency: confluence`. The 409 handling above makes an
accidental overlap fail safely rather than corrupt anything.

## Running it in CI

GitLab:

```yaml
kiln-check:
  stage: test
  script:
    - kiln check --format gitlab > gl-code-quality.json || true
    - kiln check
  artifacts:
    reports:
      codequality: gl-code-quality.json

export-plan:
  stage: test
  rules: [{ if: $CI_PIPELINE_SOURCE == "merge_request_event" }]
  variables:
    GIT_DEPTH: 0              # full history: EXPORT.md, "Which commit a link names"
  script:
    - kiln export --dry-run --format json > export-plan.json
    - kiln export spec-html   # a file export reviewers can open from the MR
  artifacts:
    paths: [export-plan.json, target/kiln/]

export-publish:
  stage: deploy
  rules: [{ if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH }]
  resource_group: confluence
  variables:
    GIT_DEPTH: 0
  script:
    - kiln export --require-clean --yes
```

`KILN_CONFLUENCE_TOKEN` (and, on Cloud, `KILN_CONFLUENCE_USER`) are
masked, protected CI variables. A merge request's dry run needs read
access only; consider a separate read-only token for it.

Ship these as `contrib/gitlab-ci.yml` and `contrib/github-workflow.yml`.

## Testing

- **Storage emitter**: golden storage files per document node and for a
  whole sample document in each layout; each parsed as XML; changes only
  by reviewed golden updates.
- **Normaliser**: bodies as returned by real Cloud and Data Center after a
  save (captured once into fixtures during K3, with the instance and date
  recorded), hashed equal to their pre-save form.
- **Planner**: table-driven over remote × desired state, every row of the
  plan table above, per layout, plus foreign pages, adopted pages, two
  targets under one parent, title collisions, retires, and the size
  warning and refusal.
- **Apply**: against an in-process fake Confluence implementing the calls
  above for both flavours (versions, properties, labels, attachments,
  children and descendants, 409, 413, 429 with `Retry-After`, 401).
  Includes interrupted publishes (fail after N calls, rerun, end state
  equals a clean run) and a layout change (`tree` → `document` retires the
  item pages).
- **Live**: `#[ignore]` tests reading `KILN_TEST_CONFLUENCE_URL`,
  `_SPACE`, `_PARENT`, `_TOKEN`, `_USER`, that publish a fixture project
  into a scratch parent page in each layout, publish again (expect all
  unchanged), follow an anchor link, edit a page through the API (expect
  drift), and retire an item. Cloud: a free Confluence Cloud site is
  enough. Data Center: Atlassian's `atlassian/confluence` container image
  with a timebomb licence; document the setup in `dev/confluence/README.md`,
  as Fenix does for GitLab in `dev/gitlab`.

## Milestone split

- **K2 (no writes)**: the storage emitter, normaliser, planner and the read
  half of both clients; `kiln export --dry-run` for Confluence targets
  against a live space and against a recorded snapshot file (`--snapshot
  <file>` for offline tests and demos). The `document` layout first, then
  `sections` and `tree`.
- **K3 (writes)**: apply, properties, labels, attachments, adopting a page,
  retires, drift, `restrict_edits`, size limits, retries, `auth
  confluence`, CI recipes, the fake server's write half, live tests on
  both flavours. Fill in every **verify** in this document with what the
  live instances did.
