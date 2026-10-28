+++
id = "ADR-019"
type = "decision"
title = "An editor the user names for Kiln comes before Fenix; VISUAL and EDITOR come after it"
status = "accepted"
+++

## Context

`kiln new --open` and the GUI's `Enter` opened items in Fenix, else the
operating system's default editor (REQ-021). At work, without Fenix, the
default editor for `.md` on Windows is often Notepad or nothing. People
already name their editor in `VISUAL` or `EDITOR`, and some want Kiln to
use one editor whatever else is installed.

## Decision

The order is `KILN_EDITOR`, the running Fenix, starting Fenix, `VISUAL`,
`EDITOR`, the OS default. `KILN_EDITOR` comes first because the user set
it for Kiln on purpose. `VISUAL` and `EDITOR` come after Fenix because
they are set for every tool, and a Fenix user's shell often sets them to
a terminal editor; putting them first would quietly stop opening Fenix.

A command is split on spaces with quotes, never run through a shell (a
path with `&` stays one argument), and `{path}` and `{line}` are
replaced, the path appended without `{path}`. From the command line the
editor gets the terminal and Kiln waits, as `git commit` does, so `vim`
works; the GUI has no terminal to lend and starts it in the background.

## Consequences

A machine without Fenix opens items where its user expects. A terminal
editor in `KILN_EDITOR` doesn't work from the GUI; the docs say to name a
windowed one there. Kiln still writes nothing of Fenix's.
