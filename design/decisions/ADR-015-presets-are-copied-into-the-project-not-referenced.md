+++
id = "ADR-015"
type = "decision"
title = "Presets are copied into the project, not referenced"
status = "accepted"
+++

People asked for ready-made documents (arc42 first) that they could add
with one command and then adapt. Kiln could ship such a template and
have a target name it (`template = "preset:arc42"`), or copy it into the
repository.

It copies. `kiln preset add` writes the template into `design/templates/`,
adds the types and link roles it places to `kiln.toml`, and appends an
export target; from then on all of it is the project's own, changed in
its merge requests like any other file, and a newer Kiln never touches
it. A referenced template would change under its authors with an
upgrade, in the published document of all places, and every team that
adapts arc42 to itself (the normal case) would have to copy it anyway.
The cost is that improvements to a preset reach only projects that add
it afterwards; `kiln init --preset NAME` in an empty folder writes the
current one to compare with.

Adding never replaces: a type the project has is kept, a role it has
gains only what the preset links, and a prefix clash, a taken export name
or a file already there stops it before anything is written
(REQ-013). The presets follow the structure of arc42 and ISO/IEC/IEEE
29148 with wording of Kiln's own, since arc42's help text is CC BY-SA
and Kiln is MIT OR Apache-2.0.
