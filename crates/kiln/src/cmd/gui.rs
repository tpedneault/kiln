//! `kiln gui` (and `kiln` alone): the window (docs/GUI.md). Loads the
//! project, the baseline for the Changes page (`--since REV`, else the
//! most recent tag) and the user's `settings.toml`, then hands over.

use std::path::Path;

use crate::project::{at_revision, folders, load};
use crate::{Fail, EXIT_USAGE};

pub fn run(
    project: Option<&Path>,
    since: Option<String>,
    item: Option<String>,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let root = loaded.source.root().to_path_buf();
    let select = item.as_deref().map(super::parse_id).transpose()?;
    let baseline_rev = since.or_else(|| kiln_git::latest_tag(&root));
    let baseline = match baseline_rev {
        Some(rev) => {
            let (old, commit) = at_revision(&root, &rev)?;
            let short = &commit[..commit.len().min(7)];
            Some(kiln_gui::Baseline {
                label: if rev.starts_with(short) {
                    short.to_owned()
                } else {
                    format!("{rev} ({short})")
                },
                project: old,
            })
        }
        None => None,
    };
    let (settings, notes) = folders()
        .map(|f| f.settings.join("settings.toml"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| kiln_gui::parse_settings(&text))
        .unwrap_or_default();
    kiln_gui::run(kiln_gui::Options {
        root,
        project: loaded.project,
        baseline,
        settings,
        notes,
        select,
    })
    .map(|()| 0)
    .map_err(|e| Fail::new(EXIT_USAGE, format!("gui: {e}")))
}
