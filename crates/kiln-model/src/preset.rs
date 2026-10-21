//! Presets (DESIGN.md, "Presets"): document templates Kiln ships, each
//! with the types and link roles it places and an export target, which
//! `kiln preset add` copies into a project.
//!
//! A preset is copied, not referenced: once added, its files are the
//! project's own, edited like any other, and a newer Kiln never changes
//! them (ADR-015). This module is only the data; merging it into
//! `kiln.toml` is `kiln_parse::add_preset`, writing the files is the
//! binary's.

/// One preset, as shipped in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// What `kiln preset add` takes.
    pub name: &'static str,
    /// One line for `kiln preset list`.
    pub summary: &'static str,
    /// `[types.*]` and `[links.*]` tables, one after another, each
    /// preceded by its comment lines. `{dir}` is the first of `paths`.
    pub types: &'static str,
    /// The `[[export]]` target (and a commented-out Confluence one),
    /// appended to `kiln.toml`. `{name}` is the project's name, `{dir}`
    /// the first of `paths`.
    pub export: &'static str,
    /// The export template, written to `{dir}/templates/<name>.md`.
    pub template: &'static str,
    /// Item body templates (`[types.*] template`), written to
    /// `{dir}/templates/<file>` only when the preset adds that type: a
    /// type the project already has keeps its own.
    pub item_templates: &'static [ItemTemplate],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTemplate {
    pub type_name: &'static str,
    pub file: &'static str,
    pub text: &'static str,
}

impl Preset {
    /// Where the export template goes, relative to the project.
    pub fn template_path(&self, dir: &str) -> String {
        match dir_prefix(dir) {
            "" => format!("templates/{}.md", self.name),
            d => format!("{d}/templates/{}.md", self.name),
        }
    }
}

/// `dir` as a path prefix: `.` (items at the root) is no prefix.
pub fn dir_prefix(dir: &str) -> &str {
    if dir == "." {
        ""
    } else {
        dir
    }
}

/// Every preset, in the order `kiln preset list` shows them.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "arc42",
        summary: "arc42 architecture documentation: twelve sections, one page each",
        types: include_str!("../presets/arc42/types.toml"),
        export: include_str!("../presets/arc42/export.toml"),
        template: include_str!("../presets/arc42/template.md"),
        item_templates: &[
            ItemTemplate {
                type_name: "quality-scenario",
                file: "quality-scenario.md",
                text: include_str!("../presets/arc42/quality-scenario.md"),
            },
            ItemTemplate {
                type_name: "block",
                file: "building-block.md",
                text: include_str!("../presets/arc42/building-block.md"),
            },
        ],
    },
    Preset {
        name: "srs",
        summary: "a software requirements specification after ISO/IEC/IEEE 29148, one document",
        types: include_str!("../presets/srs/types.toml"),
        export: include_str!("../presets/srs/export.toml"),
        template: include_str!("../presets/srs/template.md"),
        item_templates: &[ItemTemplate {
            type_name: "requirement",
            file: "requirement.md",
            text: include_str!("../presets/srs/requirement.md"),
        }],
    },
    Preset {
        name: "design-doc",
        summary: "a design document: requirements, design, decisions, milestones, risks",
        types: include_str!("../presets/design-doc/types.toml"),
        export: include_str!("../presets/design-doc/export.toml"),
        template: include_str!("../presets/design-doc/template.md"),
        item_templates: &[],
    },
];

/// The preset called `name`.
pub fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}
