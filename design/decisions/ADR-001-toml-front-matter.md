+++
id = "ADR-001"
type = "decision"
title = "TOML front matter"
status = "accepted"
+++

Items carry TOML between `+++` lines rather than YAML: Kiln edits front
matter (status, links) and `toml_edit` keeps the author's comments and
layout; TOML is the configuration language of the rest of the suite; and
it has no implicit typing surprises.
