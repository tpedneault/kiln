+++
id = "ADR-015"
type = "decision"
title = "A preset is a template copied in; its configuration is documented"
status = "accepted"
+++

People asked for ready-made documents (arc42 first) that they could add
with one command and then adapt. Two questions: whether a preset is
referenced or copied, and whether adding one also configures the
project.

**Copied.** `kiln preset add` writes the template into the project's
`templates/` folder, and from then on it is the project's file, changed
in its merge requests; a newer Kiln never touches it. A referenced
template (`template = "preset:arc42"`) would change under its authors
with an upgrade, in the published document of all places, and every team
that adapts arc42 to itself (the normal case) would copy it anyway. The
cost is that improvements to a template reach only projects that add it
afterwards; `kiln init --preset NAME` in an empty folder writes the
current one to compare with.

**Only the template.** A first version also merged the template's types,
link roles and an export target into `kiln.toml`. But those are design
choices with real options: whether tests verify quality scenarios,
whether code implements building blocks, whether concepts are items or
prose, where the document is published. Editing `kiln.toml` would make
them for the project, or need a flag for each, and would touch the one
file every other command reads. So `kiln preset add` writes the template
and names on stderr what it needs that `kiln.toml` lacks; the wiki's
"Presets" page has the configuration for each preset and its options,
every block of it tested against the template in CI, and `kiln check`
(K112) names anything still missing once an export uses the template.

The templates follow the structure of arc42 and ISO/IEC/IEEE 29148 with
wording of Kiln's own, since arc42's help text is CC BY-SA and Kiln is
MIT OR Apache-2.0.
