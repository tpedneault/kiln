+++
id = "ADR-012"
type = "decision"
title = "The GUI edits front matter, never prose"
status = "accepted"
+++

K7 made the GUI a reader: it opened items in Fenix and edited nothing,
leaving `kiln set`, `link`, `new` and `renumber` to the terminal, the
language server and Flint. In use that sent people back to the terminal
for every status change, link and new item, and for every export, plan
and comment check, which is what the window was meant to save them.

From K9 the GUI edits what `kiln set`, `link`, `unlink`, `new` and
`renumber` edit: one-line values (a status, a title, a parent, an owner,
a date), links, new items and IDs. It never edits an item's body: prose
is written in an editor, and Fenix is that editor. Every value is picked
from a list or typed on the footer's single line; there is no text area.

Three things keep the two front doors alike. The checks and the writes
are `kiln-index`'s (`plan_set`, `check_link`, `apply_set`,
`apply_link`, `apply_renumber`, `create_item`), shared with the command
line and the Flint provider, so a value one accepts the others accept and
a file an editor saved in the meantime is refused by all three. Exports
and comments run the commands' own code through the binary, so the
window shows what the terminal prints and never handles a token.
Publishing asks first and publishes only the plan it showed; overriding a
refused plan (`--force`, `--adopt`) and pruning stay on the command
line, where the flag is written down.
