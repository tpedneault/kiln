# Kiln

Requirements, design decisions and project progress, kept as Markdown
files in the repository they describe.

Kiln links requirements to the design, tasks, tests and code that answer
them, checks those links in CI (an approved requirement nothing verifies,
a broken reference, a duplicate ID), shows what a change affects, and
exports a design document (one Confluence page by default, or a page
tree, Markdown or printable HTML) for the people who don't use Kiln. It
edits nothing you don't ask it to, keeps no database, and needs no
server: the repository is the source of truth.

It is part of a suite with [Fenix](https://github.com/tpedneault/fenix)
(the editor, where Kiln's files are written, with completion and
navigation through Kiln's language server) and
[Flint](https://github.com/tpedneault/flint) (the launcher, where any item
is a few keystrokes away).

**Status:** the file format, `kiln check`, the editing commands,
exports to Markdown and HTML, publishing to Confluence (Cloud and Data
Center) and the language server for Fenix and other editors work
(K1–K4). Kiln's own requirements and decisions live
in [design/](design/), and its generated design document in
[docs/generated/DESIGN.md](docs/generated/DESIGN.md).

- [Design](docs/DESIGN.md)
- [File format](docs/FORMAT.md)
- [Exporting](docs/EXPORT.md): one design document, sections or a page tree; Confluence, Markdown or HTML
- [Publishing to Confluence](docs/CONFLUENCE.md)
- [Fenix, Flint and Jira](docs/INTEGRATION.md)
- [Architecture](docs/ARCHITECTURE.md): the crates and which way they depend
- [Command line](docs/CLI.md)
- [Configuration](docs/CONFIGURATION.md)
- [Continuous integration](docs/CI.md): the gates, and running them locally

## Licence

Kiln is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
