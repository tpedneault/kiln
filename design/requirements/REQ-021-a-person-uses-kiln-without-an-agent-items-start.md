+++
id = "REQ-021"
type = "requirement"
title = "A person uses Kiln without an agent: items start with a shape, the first export works, mistakes are said"
status = "verified"
+++

Someone who has never used Kiln, without an agent or Fenix, starts a
project and writes a first requirement and decision. `kiln new` gives
each default type a short body to fill in rather than an empty file;
`kiln export` works straight after `kiln init`; a filter that matches
nothing because it names a status no type has says so instead of
printing nothing; `--open` uses the editor they set; and `kiln --help`
shows the eight everyday commands before the integrations.

From DESIGN.md, "Milestones", K16.
