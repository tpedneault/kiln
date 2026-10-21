+++
id = "ADR-015"
type = "decision"
title = "Presets are copied into the project and stack without duplicates"
status = "accepted"
+++

People asked for ready-made documents (arc42 first) that they could add
with one command and then adapt, several to one project.

**Copied, not referenced.** `kiln preset add` writes the template into
the project's `templates/` folder and its configuration into
`kiln.toml`, and from then on all of it is the project's own, changed in
its merge requests; a newer Kiln never touches it. A referenced template
(`template = "preset:arc42"`) would change under its authors with an
upgrade, in the published document of all places, and every team that
adapts arc42 to itself (the normal case) would copy it anyway. The cost
is that improvements to a preset reach only projects that add it
afterwards; `kiln init --preset NAME` in an empty folder writes the
current one to compare with.

**Configuration included, the minimum, nothing twice.** A preset adds
the types and link roles its template places and an export target, so
it works the moment it is added. It adds only what is missing: a type
the project already declares, by hand or from another preset, is kept
as it is; a role it has gains only the `from` and `to` types it lacks;
each preset's export has the preset's name. So `arc42` and `srs` share
`constraint` and `term`, in either order. A prefix clash, a taken export
name or a file already there stops it before anything is written, and
`kiln.toml` is spliced so its comments survive (REQ-013). The links that
go further than the document needs (tests verifying quality scenarios,
code implementing building blocks, blocks using blocks) are choices with
alternatives, so the wiki's "Presets" page offers them to add by hand
rather than the preset making them; a first version that left all
configuration to the wiki made every project paste the same tables.

The presets follow the structure of arc42 and ISO/IEC/IEEE 29148 with
wording of Kiln's own, since arc42's help text is CC BY-SA and Kiln is
MIT OR Apache-2.0.
