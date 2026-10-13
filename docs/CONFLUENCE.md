# Publishing to Confluence

`kiln publish confluence` puts a Kiln project into a Confluence space as a
tree of pages, so people who never open the repository (managers,
reviewers, other teams, customers with Confluence access) read the same
requirements, decisions and progress the team works from. It is a
one-way, idempotent publisher: the repository is the source, Confluence is
a view, and running it twice in a row changes nothing the second time.

Implemented in `kiln-confluence`. Endpoint details marked **verify** were
checked against Atlassian's documentation and community answers on
2026-10-12 but must be confirmed against live Cloud and Data Center
instances in K2/K3 before being relied on; record what was found here.

## What readers get

```
<parent page you chose>
└── Flint — requirements and design          root page: overview
    ├── Requirements                         one page per type…
    │   ├── REQ-001 Flint is a launcher…     …items nested by `parent`
    │   │   ├── REQ-002 Fast enough to be a reflex
    │   │   │   └── REQ-014 Flint appears within 50 ms of the hotkey
    │   │   └── …
    ├── Design
    ├── Decisions
    ├── Milestones
    ├── Risks
    ├── Tests                                (or a single table page, per config)
    ├── Traceability                         report pages
    ├── Open findings
    ├── Roadmap
    ├── Changes since v1.0                   only with --since
    └── Removed from the repository          items whose files are gone
```

**The root page** shows the project name, the commit published and its
date (linked to the repository), counts by type and status, coverage
and milestone progress at a glance, and links to every section.

**An item page**:

1. A banner (Confluence `info` macro): "Published by Kiln from
   `design/requirements/REQ-014-….md` at `a1b2c3d`. Edit it in the
   repository; changes made here are overwritten. Comment at the bottom of
   the page." `a1b2c3d` is the last commit that changed that file (see
   "Which commit a link names"), and the path links to the file there.
2. A properties table: type, status (a `status` lozenge, colour by status
   group), parent, tags, owner, due, Jira (the `jira` macro, or a plain
   link; see "Jira"), every outgoing link by role, every computed inverse
   ("Verified by", "Implemented by" with code locations linked to the
   repository), and "Mentioned by".
3. The body, rendered (see "Rendering").
4. Coverage, for types that have it: covered / gaps, in words.

**Report pages**:

| Page | Content |
|---|---|
| Traceability | one row per non-closed requirement: ID and title, status, implemented by, verified by, gap lozenges. Split into one child page per top-level requirement when it passes `max_rows` (default 300) |
| Open findings | `kiln check` warnings and errors at the publish commit, grouped by rule, linked to the repository |
| Roadmap | milestones in `due` order, status, progress ("7 of 10 done"), included items |
| Changes since REV | with `--since REV`: items added, removed, and changed (status, title, links, body) between REV and the publish commit; body changes as a short summary with a link to the diff on the repository's web |

Configured types can be published as `mode = "pages"` (one page per item,
the default) or `mode = "table"` (a single page listing all items of that
type with their properties and bodies collapsed in `expand` macros), which
suits many small items such as tests and tasks.

## Configuration

In `kiln.toml` (committed, so no secrets):

```toml
[confluence]
url = "https://confluence.example.com"     # Data Center base, including any context path
# url = "https://example.atlassian.net/wiki" # Cloud
flavour = "auto"            # auto | cloud | data-center; auto: *.atlassian.net is cloud
space = "FLINT"
parent = 123456789          # page id the root page goes under (required)
root_title = "Flint — requirements and design"
user = ""                   # Cloud only: the account e-mail the token belongs to
auth = "auto"               # auto | bearer | basic; auto: bearer on Data Center, basic on Cloud
labels = ["flint"]          # added to every page Kiln publishes
types = ["requirement", "design", "decision", "milestone", "risk", "test"]
jira_macro = true           # use the Jira macro for `jira` keys (needs a Jira application link)
restrict_edits = false      # restrict editing of published pages to the publishing account
on_drift = "stop"           # stop | skip | overwrite (see "Drift")
max_rows = 300
concurrency = 4            # requests in flight
diagrams = "image"          # image | source (source: no rasterising, see "Fonts")
jira_server_id = ""         # Data Center with several Jira links (see "Jira")

[confluence.mode]
test = "table"

[confluence.title]
item = "{id} {title}"       # page titles; must keep the id to stay unique in the space
section = "{type_plural}"   # e.g. "Requirements"; prefixed with the root title when not unique
```

`[confluence]` may be absent: publishing then exits 2 naming the missing
keys, and nothing else in Kiln cares.

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

## How a publish runs

```
render (pure) ──► desired pages
                                  ╲
                                   plan (pure) ──► ops ──► apply ──► Confluence
                                  ╱                         │
snapshot (reads) ──► remote pages                         read-back, property
```

### 1. Render

From the index at the working tree (refuse with `--require-clean` when the
tree is dirty; CI passes it), produce the **desired state**: for every
page, a stable key, a title, a parent key, the storage-format body, the
labels, the attachments (name + bytes + content type) and the Kiln
property value. Keys: `root`, `section:<type>`, `item:<ID>`,
`report:<name>`, `report:traceability:<ID>`, `removed`.

Rendering is deterministic: same input, byte-identical output, on Windows
and Linux. Sort everything that has no inherent order, and never put the
time of the publish in a body: the root page shows the commit and the
commit's own date, so publishing the same commit twice changes nothing.

#### Which commit a link names

Every link into the repository (an item's source, a code location, a
relative link) names **the last commit that changed that file**, never
the commit being published. Otherwise every merge would change every page
that links to code, and page history would fill with versions that change
nothing a reader cares about. With this rule a page changes only when
something it shows changed; the root page, which names the published
commit, is the one page that changes on every new commit.

`kiln-git` gets these in one pass (`git log --format=%H --name-only` over
the paths involved). A shallow clone can't answer for files older than its
boundary, so `publish` checks `git rev-parse --is-shallow-repository` and
fails with "the clone is shallow: fetch full history (GitLab: GIT_DEPTH:
0; GitHub: fetch-depth: 0)" unless `--allow-shallow`, which falls back to
the published commit for those files.

### 2. Snapshot

Read what's there, under the root page only (Kiln never looks at or
touches pages outside it):

1. Find the root page: the Kiln property on the configured `parent`'s
   children, else by title under `parent`. First publish: absent.
2. List all descendants of the root with id, title, parent id, version
   number and version author.
3. For each, read the `kiln` content property, but only when the page's
   version differs from the local cache (state folder,
   `confluence/<space>-<root id>.json`: page id → version, property). In
   CI there is no cache and every property is read; at concurrency 4 that
   is a few seconds for hundreds of pages.
4. Pages under the root without a `kiln` property are **foreign**: never
   modified, moved or deleted, and reported in the plan so people know
   they are there.

### 3. Plan

A pure function of desired and remote state, unit-tested table-driven.
For each desired page, matched to a remote page by the `item`/`key` in its
property:

| Situation | Op |
|---|---|
| no remote page | `create` |
| remote exists, `renderHash`, title, parent, labels and attachment set all equal | `unchanged` (no call) |
| remote exists, something differs, no drift | `update` (and `move`, `label`, `attach`, `unattach` as needed) |
| remote exists, drift (below) | per `on_drift`: `stop` refuses the whole plan; `skip` leaves that page; `overwrite` (or `--force`) updates it |
| a remote Kiln page with no desired page | `retire`: move under "Removed from the repository", replace the body with a banner and the last published content, label `kiln-removed` |
| title collides with a foreign page in the space | error naming the page; nothing applied |

`--dry-run` prints the plan and exits: one line per op with page title and
reason ("body changed", "parent REQ-002 → REQ-003", "drift: edited by
alice at 2026-10-10 14:02, version 7"), then totals. `--format json` gives
the plan as data, for CI artifacts. Without `--yes`, an interactive run
shows the totals and asks before applying; non-interactive runs without
`--yes` stop after the plan.

### 4. Apply

Order: create pages parent-first (so parents exist), then updates and
moves, then labels and attachments, then retires, then report pages, then
the root page last (so the commit the root page names means everything
under it is at that commit). Each page's property is written **after** its body.

After each create or update, read the page's storage body back, normalise
it (below) and store its hash in the property as `remoteHash`. That makes
drift detection independent of whatever Confluence does to storage on
save.

`--yes` on a plan whose snapshot is older than 2 minutes re-snapshots
first.

### The `kiln` content property

Key `kiln`, value (JSON, well under the property size limit; comments
here are for the reader only):

```jsonc
{
  "schema": 1,
  "project": "Flint",
  "key": "item:REQ-014",
  "source": "design/requirements/REQ-014-flint-appears-within-50-ms.md",
  "commit": "a1b2c3d4e5f6…",              // last commit that changed `source`
  "published": "9f8e7d6c5b4a…",            // commit of the publish that last wrote this page
  "renderHash": "sha256:…",
  "remoteHash": "sha256:…",
  "publishedVersion": 8,
  "attachments": { "diagram-3f9a1c2b7e44.png": "sha256:…" }
}
```

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
  to the page's history so the edit can be read and carried into the
  repository by hand.
- `on_drift = "stop"` (the default) refuses to publish anything until the
  edit is dealt with: re-run with `--force` after copying it into the repo,
  or set `skip` to publish everything else.
- `restrict_edits = true` adds an edit restriction for the publishing
  account to each page Kiln creates, which prevents most drift. Off by
  default: some organisations disallow restrictions or need admins to keep
  edit rights. (**verify** the restrictions endpoints on both flavours.)

**Normalising storage** before hashing: parse as XML; drop
`ac:macro-id`, `ac:local-id`, `local-id` and `data-*` attributes; drop
`ac:inline-comment-marker` elements (keeping their text); sort attributes;
collapse whitespace between block elements; serialise canonically. The
normaliser is tested on bodies captured from both flavours.

## Comments

Footer comments live outside the page body and survive every update.
Inline comments anchor to markers in the body; Kiln's update would drop
those markers and orphan the comments, so in v1 the banner asks readers to
comment at the bottom. A later improvement (not before K6): carry existing
`ac:inline-comment-marker` elements over into the new body wherever the
marked text still appears verbatim in the same block.

`kiln confluence comments` (K6) lists unresolved footer comments per item,
with links, so feedback from Confluence readers reaches the team.

## Rendering

The emitter turns `kiln-render`'s document tree into Confluence storage
format (XHTML with `ac:`/`ri:` elements). It always produces well-formed
XML (a test parses every golden file), escapes all text, and never passes
raw HTML through.

| Source | Storage |
|---|---|
| headings, paragraphs, emphasis, strong, strikethrough, inline code | `h1`–`h6` (shifted so the item title is `h1`), `p`, `em`, `strong`, `del`, `code` |
| lists, GitHub task lists | `ul`/`ol`; task items as text `☐`/`☑` (Confluence task lists are interactive, which invites edits) |
| tables | `table`/`tbody`/`tr`/`th`/`td` |
| fenced code | `code` macro with `language` parameter, body in CDATA (split any `]]>`) |
| ` ```mermaid ` | rendered with `fenix-diagram` (`render`, theme `default`, then `png` at scale 2 on `Background::White`), attached as `diagram-<sha256 first 12 hex>.png`, shown with `ac:image`; the source follows in a collapsed `expand` macro titled "Diagram source" |
| a diagram that fails to render | a `warning` macro with the error and line, and the source in a code macro; also finding K111 |
| `[[REQ-014]]`, bare IDs | `ac:link` to `ri:page` with the target's published title and the space key; text "REQ-014 Title" or the custom text. A reference to an item of a type not published becomes plain text plus a repository link |
| callouts `[!NOTE]` … | `info` (NOTE, IMPORTANT), `tip` (TIP), `note` (WARNING), `warning` (CAUTION) macros |
| relative links to repo files | `a href` to the repository web URL at the last commit that changed the target |
| relative images | attached (content-addressed name, as diagrams) and shown with `ac:image` |
| status | `status` macro, `colour` by group: initial Grey, done Green, closed Grey with strike, others Blue; `kiln-gap` Red |
| code locations | `a href` to the repository web URL with `{line}` |

Because links go by title, a title change re-renders every page that
links to the renamed item; the planner sees those as body changes and
updates them in the same run.

### Fonts

Rasterising needs fonts. `fenix-diagram` loads system fonts
only, and slim CI images may have none, which produces diagrams without
text. Before rasterising, Kiln checks the font database is non-empty and
otherwise fails with "no fonts found for diagrams: install fonts-dejavu
(Debian/Ubuntu) or set [confluence] diagrams = "source"" (which publishes
the source in a code macro instead of a picture). INTEGRATION.md G2 asks
`fenix-diagram` for a bundled fallback font.

### Caching

Rendered PNGs are cached in the state folder by
`sha256(source + fenix-diagram rev + scale)`; CI can cache that folder.

## Jira

With `jira_macro = true`, each `jira` key renders as the Jira macro
(`ac:structured-macro ac:name="jira"` with `key`), which shows the live
status to readers and needs an application link between Confluence and
Jira. On Data Center with several Jira servers linked the macro also needs
a `serverId`; set `[confluence] jira_server_id`. With `jira_macro = false`,
keys render as links to `<[jira] url>/browse/<KEY>` (FORMAT.md), or as
plain text when no Jira URL is configured. (**verify** the macro's parameters on
both flavours.)

## The API, by flavour

One trait, `Confluence`, with two implementations. Every call Kiln makes is
listed here; nothing else is used.

| Operation | Data Center (`<url>/rest/api`) | Cloud (`<url>` = `https://<site>.atlassian.net/wiki`) |
|---|---|---|
| who am I | `GET /user/current` | `GET /rest/api/user/current` |
| page by id (title, version, parent, space) | `GET /content/{id}?expand=version,ancestors,space` | `GET /api/v2/pages/{id}` |
| page body (storage) | `GET /content/{id}?expand=body.storage,version` | `GET /api/v2/pages/{id}?body-format=storage` |
| descendants of the root | `GET /content/{id}/descendant/page?expand=version,ancestors&limit=…` (paged) | `GET /api/v2/pages/{id}/descendants` (paged; **verify** depth and fields), else children recursively `GET /api/v2/pages/{id}/children` |
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
| retire / trash page | moved under "Removed…" by update; `--prune-removed` sends retired pages older than N days to the trash: `DELETE /content/{id}` | `DELETE /api/v2/pages/{id}` |
| restrictions | `PUT /content/{id}/restriction` (**verify**) | `PUT /rest/api/content/{id}/restriction` (**verify**) |

Notes:

- Attachments use content-addressed names, so Kiln only ever creates and
  deletes them, never uploads a new version. That avoids the attachment
  update endpoints, which differ the most between flavours.
- `ureq` 2 has no multipart support; build the multipart body by hand (one
  part, a boundary that doesn't occur in the bytes). Tested against the
  fake server's parser.
- Page version messages read `kiln 9f8e7d6: <commit subject>` (the
  published commit), so page history shows which merge produced each
  version.
- Titles are limited to 255 characters; truncate the item title with `…`,
  never the ID.

### Errors, limits, retries

- **409 Conflict** on update (someone or another publish changed the page
  since the snapshot): re-read that page; if it is now drift, apply
  `on_drift`; otherwise recompute that page's op once and retry. A second
  409 fails the publish naming the page.
- **429 Too Many Requests** and **503**: wait for `Retry-After` (seconds)
  when present, otherwise exponential backoff from 1 s with jitter, at most
  6 attempts per call, then fail with the endpoint named.
- Concurrency: at most 4 requests in flight (`[confluence] concurrency`).
- Timeouts: 10 s connect, 60 s per request (attachments 120 s).
- Any 4xx other than these fails the publish with the server's message.
  Ops already applied stay applied; the next run's plan picks up from
  there, because every op is idempotent with respect to the desired state.

### Serial publishes

Two publishes of the same project at the same time would both see the
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

confluence-plan:
  stage: test
  rules: [{ if: $CI_PIPELINE_SOURCE == "merge_request_event" }]
  variables:
    GIT_DEPTH: 0
  script:
    - kiln publish confluence --dry-run --format json > confluence-plan.json
  artifacts: { paths: [confluence-plan.json] }

confluence-publish:
  stage: deploy
  rules: [{ if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH }]
  resource_group: confluence
  variables:
    GIT_DEPTH: 0              # full history; see "Which commit a link names"
  script:
    - kiln publish confluence --require-clean --yes
```

`KILN_CONFLUENCE_TOKEN` (and, on Cloud, `KILN_CONFLUENCE_USER`) are
masked, protected CI variables. A merge request's dry run needs read
access only; consider a separate read-only token for it.

Ship these as `contrib/gitlab-ci.yml` and `contrib/github-workflow.yml`.

## Testing

- **Emitter**: golden storage files per construct and for whole sample
  items; each parsed as XML; snapshot-reviewed changes only.
- **Normaliser**: bodies as returned by real Cloud and Data Center after a
  save (captured once into fixtures during K3, with the instance and date
  recorded), hashed equal to their pre-save form.
- **Planner**: table-driven over remote × desired state, every row of the
  plan table above, plus foreign pages, title collisions and retires.
- **Apply**: against an in-process fake Confluence implementing the calls
  above for both flavours (versions, properties, labels, attachments,
  descendants, 409, 429 with `Retry-After`, 401). Includes interrupted
  publishes (fail after N calls, rerun, end state equals a clean run).
- **Live**: `#[ignore]` tests reading `KILN_TEST_CONFLUENCE_URL`,
  `_SPACE`, `_PARENT`, `_TOKEN`, `_USER`, that publish a fixture project
  into a scratch parent page, publish again (expect all unchanged), edit a
  page through the API (expect drift), and retire an item. Cloud: a free
  Confluence Cloud site is enough. Data Center: Atlassian's
  `atlassian/confluence` container image with a timebomb licence; document
  the setup in `dev/confluence/README.md`, as Fenix does for GitLab in
  `dev/gitlab`.

## Milestone split

- **K2 (no writes)**: render, emitter, normaliser, planner, the read half
  of both clients, `--dry-run` against a live space and against a recorded
  snapshot file (`--snapshot <file>` for offline tests and demos).
- **K3 (writes)**: apply, properties, labels, attachments, retires, drift,
  `restrict_edits`, retries, `auth confluence`, CI recipes, the fake
  server's write half, live tests on both flavours. Fill in every
  **verify** in this document with what the live instances did.
