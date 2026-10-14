Problems people commonly hit, what causes them and how to fix them,
followed by frequently asked questions. Every Kiln error is one line on
stderr starting with `kiln:`. Search this page for words from that line.

## Problems

### `kiln: no kiln.toml …`

Kiln looks for `kiln.toml` in the current folder and each folder above
it. Run the command inside your project, pass `--project <dir>`, or
create a project with `kiln init`. The exit code is 2.

### `kiln check` fails, and I don't know the rule

Each finding names a rule (`K102`). [Checking](Checking) lists every
rule with an example and the fix. The most common ones:

| Rule | Usually means | Fix |
|---|---|---|
| K102 | a reference to an ID that doesn't exist, often a typo or a deleted item | correct the ID; for an ID from another project, put it in a code span |
| K101 | two items with the same ID, after merging two branches | `kiln renumber ID --to NEW --file <one of them>` |
| K107 | a status the type doesn't have | use one of the type's statuses (`kiln.toml` lists them) |
| K108 | a link role that this type can't use, or that can't point at the target's type | write the link on the other item, or change `from`/`to` in `kiln.toml` |
| K109 | a link written on its inverse side (`verified-by`) | write it on the source (`verifies` in the test) |
| K201, K202 | an approved requirement nothing verifies or implements | link a test or design, or mark the code (`// verifies: REQ-…`) |

### `kiln export --check` fails in CI

The committed document no longer matches what Kiln would generate:
someone edited an item, the template or `kiln.toml` without running
`kiln export`. Run `kiln export` and commit the result.

If you can't see why it changed, the document probably shows code
locations with line numbers, and editing code above a `// implements:`
comment moved them. That is the check doing its job.

### `the clone is shallow: fetch full history …`

Exports name, for each linked file, the commit that last changed it.
That needs full git history. In GitHub Actions, set `fetch-depth: 0` on
`actions/checkout`. In GitLab CI, set `GIT_DEPTH: 0`. If you really can't,
`kiln export --allow-shallow` links older files at the exported commit
instead.

### `no fonts found for diagrams …`

Drawing Mermaid diagrams as pictures (HTML, Word, Confluence) needs
fonts, and slim Linux images have none. Install some:
`apt-get install -y fonts-dejavu-core`.

HTML and Word exports always draw diagrams, so fonts are the only fix for
them. Markdown and Confluence targets can set `diagrams = "source"`
instead, to show the diagram's source as a code block.

### A command refuses because a file changed on disk

`kiln set`, `link`, `unlink` and `renumber` refuse to write a file that
changed since Kiln read it, so they never overwrite an editor's save.
Run the command again. If your editor has unsaved changes in that file,
save them first.

### A command refuses because an ID is duplicated

Editing commands won't touch an ID that two items share, because they
can't know which you meant. Renumber one of them first (K101 above).

### `kiln auth …` can't reach the credential store

On a Linux machine without a Secret Service (servers, containers, WSL
without a keyring), tokens can't be stored. Set the environment variable
instead, as CI does: `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER`
on Confluence Cloud), or `KILN_JIRA_TOKEN`.

### Confluence refuses the token

`Confluence refused the token for <url>` means a 401. Check:

- **Data Center**: you need a personal access token.
- **Cloud**: you need an API token, plus your account e-mail in
  `[confluence] user` or `KILN_CONFLUENCE_USER`.
- **The URL**: it must be exactly as configured. On Cloud it includes
  `/wiki`.

Run `kiln auth confluence` again to store a fresh token.

### The publish stops with "drift"

Someone edited a page Kiln publishes directly in Confluence. Kiln won't
overwrite their edit silently. Copy the edit into the item files, then
run `kiln export --force` for that target. To publish the other pages
and leave drifted ones alone, set `on_drift = "skip"`. See [Publishing to
Confluence](Publishing-to-Confluence).

### `K207 not checked: …`

Rule K207 compares items with their Jira tickets, and Jira couldn't be
read. Common causes:

- no token is set;
- the Jira URL is Jira Cloud, which isn't supported yet;
- the network is down;
- Jira is only reachable through a proxy, which the mirror doesn't use
  yet.

The other rules still ran. See [Jira](Jira).

### The window doesn't open

- With no display (SSH, CI), `kiln gui` fails with one line and exit code 2.
  That is expected.
- On Windows, `kilnw` has no console. When it can't start, it writes why
  to `kilnw.log` in your temporary folder (`%TEMP%`).
- Unknown settings in `settings.toml` don't stop the window. They are
  reported in its footer.

### The language server doesn't start in my editor

Check that `kiln lsp` runs from a terminal in the project: it should wait
silently for input. Then check the editor's configuration runs `kiln`
with the argument `lsp` for Markdown files ([Editors and
Flint](Editors-and-Flint)). The server answers only for files inside a
Kiln project. A Markdown file elsewhere gets empty answers.

### My code references aren't found

- Is the folder listed in `[code] paths`? It is commented out by default.
- Is the file ignored by `.gitignore`, or matched by `[code] exclude`?
- Is the role one whose `from` includes `code` (`implements`, `verifies`)?
- Is the text exactly `role: ID`, with a colon and a valid ID
  (`REQ-007`, not `REQ-7`)?
- Files over 2 MiB and binary files are skipped.

## Frequently asked questions

### Do I need Fenix, Flint, Confluence or Jira?

No. Kiln works on its own with any editor. Each integration adds
something when it's there and steps aside when it isn't.

### Can I use Kiln for a project that isn't software?

Yes. Types, statuses and link roles are yours to define in `kiln.toml`.
Hardware, research and process projects work the same way. Code
references are optional.

### Why TOML front matter and not YAML?

Kiln edits front matter (statuses, links) and must keep your comments
and layout. TOML can be edited that way reliably, it has no implicit
typing surprises, and it is what the rest of the suite uses.

### Why sequential IDs instead of random ones?

People say "REQ-14" in meetings and find it with a text search.
Sequential IDs occasionally collide when two branches add items at once.
`kiln check` catches that, and `kiln renumber` fixes it in one command.

### Can people edit the published Confluence page?

They can, but the repository stays the source. Kiln detects the edit
(drift) and stops before overwriting it, so the edit can be carried into
the repository. Ask readers to leave footer comments instead.
`kiln confluence comments` brings those back to the team.

### Can I have several Kiln projects in one repository?

Yes. Put a `kiln.toml` in each project's folder. Kiln uses the nearest
one above the current folder, or the one `--project` names. A project's
`paths` and `[code] paths` must stay inside its own folder.

### How do I move an existing Word or spreadsheet design into Kiln?

Create one item per requirement or decision with `kiln new` (a short
script calling it works for large lists), then paste in the text. Add
the links as you go, and let `kiln check` show the gaps. Keep the old
document's numbers in the title or the body ("Was 3.2.1 in the 2025
specification") if people still refer to them. `[extra]` keeps such
values in the file, but Kiln doesn't display them anywhere.

### Does Kiln replace Jira?

No. Kiln holds what the project must do and why. Jira holds who does what
and when. Items can name Jira keys, and Kiln can show their status, but
it has no assignees, estimates or sprints ([Jira](Jira)).

### Where are secrets kept?

Never in files. `kiln.toml` rejects anything that looks like a secret
(K004). Tokens come from environment variables or the OS credential
store, and never appear in logs, plans or error messages.

See also: [Checking](Checking) · [Command reference](Command-reference) ·
[Configuration](Configuration)
