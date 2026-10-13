# Command line

How to run `kiln`, as it stands. The commands planned for each milestone
are in [DESIGN.md](DESIGN.md), "Command line"; each is documented here
when it ships.

## Commands

| Command | Does |
|---|---|
| `kiln --help` | prints usage |
| `kiln --version` | prints `kiln <version>` |

No other commands exist yet; `kiln init`, `new`, `list`, `show`, `check`
and the rest of K1 come next. `kiln` with no arguments prints usage and
exits 2 (from K7 it opens the GUI).

## Errors and exit codes

An error is one line on stderr, starting `kiln: `, naming what failed and
why (with the file and line when there is one):

```
$ kiln frobnicate
kiln: unexpected argument 'frobnicate' found (see `kiln --help`)
```

| Exit code | Means |
|---|---|
| 0 | fine |
| 1 | findings, `--check` found changes, or a plan was refused |
| 2 | usage or configuration error |
| 3 | network or server error |
