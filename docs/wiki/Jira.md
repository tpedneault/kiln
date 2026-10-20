This page explains how Kiln works with Jira: linking items to tickets,
how those links appear in exported documents and Confluence, the
read-only mirror that shows each ticket's status, the `kiln check` rule
that compares items with their tickets, and tokens. Read it if your team
tracks work in Jira and designs with Kiln.

Kiln works without Jira. Everything on this page is optional, and when
Jira can't be reached, Kiln says why and carries on.

## Kiln is not a second Jira

Kiln holds the design: requirements, decisions, milestones, and the work
items that implement them. Jira holds the day-to-day work: who does what,
in which sprint, with which workflow. Kiln links the two and reads Jira,
and that is all:

- It **never creates, edits or transitions** a ticket.
- It has **no assignment and no sprints**. An item's `owner` is
  informational only.
- It shows a ticket's status and assignee next to the item, so you can
  see where the work stands without leaving the design.

Change tickets in Jira, or in Fenix's Jira page if you use Fenix.

## Linking an item to tickets

Add a `jira` key to an item's front matter: one key as a string, or
several as an array.

```markdown
+++
id = "TASK-001"
type = "task"
title = "Implement plugin loader"
status = "doing"
jira = "DEMO-12"

[links]
part-of = ["MS-001"]
implements = ["REQ-002"]
+++
```

```toml
jira = ["DEMO-12", "DEMO-14"]
```

Any item type can carry `jira` keys. Kiln doesn't check that a key
exists when you write it (Jira may not be reachable); the mirror reports
unknown keys (see below).

A Jira key in an item's body is just text. Kiln treats a bare ID in prose
as a reference only when its prefix is one your project declares (`REQ`,
`TASK`…), so `DEMO-12` is not mistaken for an item. If a Jira project's
key happens to equal one of your prefixes, write the ticket in a
`code span` to keep `kiln check` from reporting it as a missing item.

## `[jira] url`

Tell Kiln where your Jira is, in `kiln.toml`:

```toml
[jira]
url = "https://jira.example.com"
```

It is used for links (`<url>/browse/<KEY>`) and for the mirror. Never put
a token here; `kiln check` reports anything that looks like a secret in
`kiln.toml` (K004).

## How keys show in exports

In every export format (see [Templates and exports](Templates-and-exports)),
an item's Jira keys appear in its properties. Real lines from a Markdown
export:

```markdown
### 5.1 TASK-001 Implement plugin loader

**doing**\
**Part-of:** [MS-001 Beta](#41-ms-001-beta)\
**Implements:** [REQ-002 Cold start under one second](#12-req-002-cold-start-under-one-second)\
**Jira:** [DEMO-12](https://jira.example.com/browse/DEMO-12)
```

Without `[jira] url`, the key is plain text.

A template can select items by Jira key with the `jira` attribute:

```markdown
<!-- kiln:items jira="DEMO-*" -->
```

### In Confluence: the Jira macro

In a Confluence export, each key renders as the **Jira macro** by default
(`[confluence] jira_macro = true`), which shows readers the ticket's live
status. The macro needs an application link between Confluence and Jira.
On Data Center with several Jira servers linked, set
`[confluence] jira_server_id`. With `jira_macro = false`, keys are links
to `<[jira] url>/browse/<KEY>`. See
[Publishing to Confluence](Publishing-to-Confluence#the-jira-macro).

## The mirror: ticket status, read-only

The **mirror** is Kiln's read-only copy of the tickets your items link
to: each ticket's summary, status, assignee, and whether it is resolved.
`kiln list --jira`, `kiln check` (rule K207) and the GUI all use it.

- Kiln asks Jira only about the keys your items name, in searches of up
  to 100 keys at a time. When Jira refuses a batch because one key
  doesn't exist, Kiln asks again key by key and reports the unknown keys
  as missing.
- The result is cached for **10 minutes** in Kiln's state folder, as
  `state/jira/<host>.json` (under `~/.local/state/kiln` on Linux,
  `%LocalAppData%\kiln` on Windows, or `KILN_HOME` when set). So
  `kiln list --jira`, `kiln check` and the GUI share one fetch. You can
  delete the file at any time to force a fresh read.
- **Resolved** means the ticket's status is in Jira's **Done** status
  category, whatever the status is called in your workflow ("Done",
  "Closed", "Released"…). A workflow that resolves tickets outside the
  Done category would read differently.

## `kiln list --jira`

Adds each linked ticket after the item: its status and assignee, or
"(not in Jira)" for a key Jira doesn't know. Run against a test Jira:

```console
$ kiln list --jira
TASK-001  done   Implement plugin loader  [DEMO-12 In Review · Alice]
TASK-002  doing  Benchmark cold start  [DEMO-13 Done]
TASK-003  todo   Old spike  [DEMO-99 (not in Jira)]
```

The usual filters combine with it: `kiln list --type task --jira`.

With `--format json`, each item gets a `jira` array of
`{key, summary, status, resolved, assignee}`, or `{key, missing: true}`
for an unknown key:

```console
$ kiln list --jira --format json
[
  {
    "id": "TASK-001",
    "jira": [
      {
        "assignee": "Alice",
        "key": "DEMO-12",
        "resolved": false,
        "status": "In Review",
        "summary": "Implement the plugin loader"
      }
    ],
    "line": 2,
    "parent": null,
    "path": "design/tasks/TASK-001-implement-plugin-loader.md",
    "status": "done",
    "tags": [],
    "title": "Implement plugin loader",
    "type": "task"
  },
  …
  {
    "id": "TASK-003",
    "jira": [
      {
        "key": "DEMO-99",
        "missing": true
      }
    ],
    …
  }
]
```

`kiln list --jira` needs `[jira] url` and a token. When Jira can't be
read, it fails with one line and exit code 3:

```console
$ kiln list --jira
kiln: Jira at https://jira.example.com: request failed: … Dns Failed: resolve dns name 'jira.example.com:443': failed to lookup address information: Name or service not known
```

## Rule K207: items and tickets that disagree

K207 is a `kiln check` rule (see [Checking](Checking)) that compares items
with their tickets. It applies to items with `jira` keys whose type has
"done" statuses (in the default configuration: requirements, whose done
status is `verified`, milestones and tasks) and reports:

- an item that is **done while one of its tickets isn't resolved**; or
- an item whose tickets are **all resolved while the item is neither done
  nor closed**.

Tickets Jira doesn't know are left out of the comparison.

### Turning it on

K207 is off by default. Turn it on in `kiln.toml` as a warning, or as an
error to fail CI:

```toml
[check]
K207 = "warning"
```

It then reports, for example:

```console
$ kiln check
design/tasks/TASK-001-implement-plugin-loader.md:5:10: K207 warning: TASK-001 is done but its Jira ticket isn't resolved: DEMO-12 (In Review)
design/tasks/TASK-002-benchmark-cold-start.md:5:10: K207 warning: TASK-002 is doing but its Jira ticket is resolved: DEMO-13; set it to done
kiln: 2 warnings
```

The fix is in whichever side is wrong: `kiln set TASK-002 status=done`
in Kiln, or move the ticket in Jira.

### When Jira is unreachable

K207 never breaks `kiln check`. When Jira can't be read (no token, no
network, a Jira Cloud URL), stderr says why K207 wasn't checked, the
other rules run, and they alone decide the exit code:

```console
$ kiln check
kiln: K207 not checked: couldn't read `jira:https://jira.example.com` from the credential store: Platform secure storage failure: no secret service provider or dbus session found (or set KILN_JIRA_TOKEN)
design/requirements/REQ-003-works-offline.md:2:7: K201 warning: REQ-003 is proposed but nothing verifies it
…
```

So you can turn K207 on in a shared `kiln.toml` even if some machines or
CI jobs have no Jira token.

## Tokens

The mirror signs in with a Jira **personal access token**. Kiln looks for
it in this order:

1. **`KILN_JIRA_TOKEN`**, for CI (as a masked variable) or a one-off.
2. **The OS credential store**: Windows Credential Manager, or the Secret
   Service on Linux (KDE Wallet, GNOME Keyring), where `kiln auth jira`
   puts it.

`kiln auth jira` asks for the token without echoing it, checks it with
one search, and stores it under service `fenix-suite`, account
`jira:<url>`. That is the same entry Flint uses, so one stored token
serves both.

```console
$ kiln auth jira
Personal access token for https://jira.example.com:
stored the token for https://jira.example.com in the credential store as fenix-suite/jira:https://jira.example.com
```

`kiln auth jira --forget` removes it. Create a personal access token in
Jira under your profile, "Personal Access Tokens". A token is never
written to a file, a log or an error message.

## In the GUI

With `[jira] url` set and items that link tickets, every item row in
Kiln's window ends with its tickets once the mirror has arrived, for
example `FLINT-12 In Review · Alice`: green when resolved, muted
otherwise, and `FLINT-12 ?` when Jira doesn't know the key. The mirror
loads in the background (from the cache when it is fresh), so a slow or
absent Jira never holds up the window; when it can't be read, the footer
says why. It is read once when the window opens, not refreshed while it
stays open. See [The GUI](The-GUI).

## Limits

- **Jira Server and Data Center only.** Jira Cloud isn't supported yet:
  the Jira client Kiln uses (from the Fenix editor) signs in only with a
  personal access token. A `*.atlassian.net` URL is refused by name:

  ```console
  $ kiln list --jira
  kiln: https://demo.atlassian.net is Jira Cloud, which fenix-jira can't sign in to yet (INTEGRATION.md, G6); the mirror works with Jira Server and Data Center
  ```

  `kiln auth jira` refuses it too (exit code 2); `kiln check` skips K207
  with that reason. Links and the Confluence Jira macro still work with
  Cloud, since they don't sign in.
- **No proxy yet.** Jira requests go out directly; they don't use
  `HTTPS_PROXY`. A Jira inside your network, the usual Data Center case,
  needs none, but a Jira reachable only through a corporate proxy can't be
  read yet. (Confluence calls do use the proxy.)
- **"Resolved" means the Done status category**, not Jira's `resolution`
  field.
- **The mirror has been tested against a fake Jira** that follows the
  Data Center search API, not yet against a live instance. If your Jira
  answers differently, please report it.

See also: [Checking](Checking) · [Publishing to Confluence](Publishing-to-Confluence) ·
[The GUI](The-GUI) · [Command reference](Command-reference)
