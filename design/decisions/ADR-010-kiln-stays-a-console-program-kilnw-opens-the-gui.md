+++
id = "ADR-010"
type = "decision"
title = "kiln stays a console program; kilnw opens the GUI"
status = "accepted"
+++

DESIGN.md planned the Fenix and Flint way for the GUI on Windows: one
GUI-subsystem executable that attaches to the terminal it was typed in,
with `--console` to force one. Fenix and Flint are GUIs first; Kiln is a
command-line and CI tool first, and a GUI-subsystem program isn't waited
for by `cmd.exe` and loses its exit code in batch files, which is what
CI and scripts rely on. Found when the GUI arrived (K7).

`kiln.exe` stays a console program: `kiln` alone or `kiln gui` opens the
window from a terminal. `kilnw.exe`, a GUI-subsystem program next to it,
starts `kiln gui` with no console window: what a shortcut or Flint's
"Open in Kiln" runs. On Linux and macOS there is one `kiln`; `kilnw`
does the same there.
