# Kiln's own design

Kiln's requirements, decisions, milestones and risks, kept the way Kiln
keeps any project's: one Markdown file per item with a `+++` TOML front
matter block (docs/FORMAT.md). `kiln check` runs on this folder in CI.

```
kiln list                 # everything
kiln show REQ-003         # one item, links both ways
kiln trace                # what implements and verifies each requirement
kiln check                # the rules
```

Code references in `crates/` (comments naming `implements` or
`verifies` and an ID) link Kiln's code and tests to the requirements here.
