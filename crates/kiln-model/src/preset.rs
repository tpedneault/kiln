//! Presets (DESIGN.md, "Presets"): document templates Kiln ships, which
//! `kiln preset add` copies into a project's `templates/` folder.
//!
//! A preset is only the template. The types it places, their links and
//! the export target that uses it are configuration, which the project
//! writes in its own `kiln.toml`, choosing among the options the wiki's
//! "Presets" page describes: a project adopting arc42 decides what its
//! building blocks link to, and Kiln doesn't decide it for it by editing
//! its configuration (ADR-015). Copied, not referenced: once added, the
//! template is the project's file, and a newer Kiln never changes it.

/// One preset, as shipped in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// What `kiln preset add` takes, and the template's file name.
    pub name: &'static str,
    /// One line for `kiln preset list`.
    pub summary: &'static str,
    /// The export template.
    pub template: &'static str,
}

impl Preset {
    /// Where the template goes, relative to the project: the `templates/`
    /// folder of `dir`, the first of `paths` (`.` is the project root).
    pub fn template_path(&self, dir: &str) -> String {
        match dir {
            "." | "" => format!("templates/{}.md", self.name),
            d => format!("{d}/templates/{}.md", self.name),
        }
    }
}

/// Every preset, in the order `kiln preset list` shows them.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "arc42",
        summary: "arc42 architecture documentation: twelve sections, one page each",
        template: include_str!("../presets/arc42/template.md"),
    },
    Preset {
        name: "srs",
        summary: "a software requirements specification after ISO/IEC/IEEE 29148",
        template: include_str!("../presets/srs/template.md"),
    },
    Preset {
        name: "design-doc",
        summary: "a design document: requirements, design, decisions, milestones, risks",
        template: include_str!("../presets/design-doc/template.md"),
    },
];

/// The preset called `name`.
pub fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_go_in_the_items_folder() {
        let arc42 = preset("arc42").unwrap();
        assert_eq!(arc42.template_path("design"), "design/templates/arc42.md");
        assert_eq!(arc42.template_path("."), "templates/arc42.md");
        assert!(preset("arc43").is_none());
    }
}
