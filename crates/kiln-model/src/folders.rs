//! Kiln's own folders (DESIGN.md, "Kiln's own folders"): settings, and
//! state with logs beside it, or all of them in one folder under
//! `KILN_HOME`. The same rules as `fenix_storage::paths`, which hard-codes
//! `fenix`, so they are mirrored here as Flint's `flint-config` does.
//!
//! Pure: the caller passes the platform's folders and `KILN_HOME` (from
//! the `dirs` crate and the environment), so the rules are tested without
//! touching either.

use std::path::{Path, PathBuf};

/// The folders, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folders {
    /// The user's `settings.toml` (roams on Windows).
    pub settings: PathBuf,
    /// What Kiln noticed on this machine: caches, the Confluence page
    /// cache. Never roams.
    pub state: PathBuf,
    pub logs: PathBuf,
}

/// What the platform says, as `dirs` reports it.
#[derive(Debug, Clone, Default)]
pub struct Platform {
    /// `%AppData%`, `~/.config`.
    pub config_dir: Option<PathBuf>,
    /// `~/.local/state` (Linux only).
    pub state_dir: Option<PathBuf>,
    /// `%LocalAppData%`, `~/.local/share`.
    pub data_local_dir: Option<PathBuf>,
}

impl Folders {
    /// All of them under `home`.
    pub fn portable(home: &Path) -> Self {
        Folders {
            settings: home.to_path_buf(),
            state: home.join("state"),
            logs: home.join("logs"),
        }
    }

    /// `KILN_HOME` when set and not empty, else the platform's folders;
    /// `None` when the platform has no notion of them.
    pub fn resolve(kiln_home: Option<&Path>, platform: &Platform) -> Option<Self> {
        if let Some(home) = kiln_home.filter(|h| !h.as_os_str().is_empty()) {
            return Some(Folders::portable(home));
        }
        let settings = platform.config_dir.as_ref()?.join("kiln");
        let local = platform
            .state_dir
            .as_ref()
            .or(platform.data_local_dir.as_ref())
            .map(|d| d.join("kiln"))
            .unwrap_or_else(|| settings.clone());
        // A local folder that is the settings folder (macOS) keeps the
        // two apart by a subfolder.
        let local = if local == settings {
            settings.join("local")
        } else {
            local
        };
        Some(Folders {
            settings,
            state: local.join("state"),
            logs: local.join("logs"),
        })
    }

    /// The cache of rasterised diagrams.
    pub fn diagram_cache(&self) -> PathBuf {
        self.state.join("diagrams")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kiln_home_holds_everything() {
        let f = Folders::resolve(Some(Path::new("/k")), &Platform::default()).unwrap();
        assert_eq!(f.state, Path::new("/k/state"));
        assert_eq!(f.diagram_cache(), Path::new("/k/state/diagrams"));
    }

    #[test]
    fn platform_folders_like_fenix() {
        let linux = Platform {
            config_dir: Some("/h/.config".into()),
            state_dir: Some("/h/.local/state".into()),
            data_local_dir: Some("/h/.local/share".into()),
        };
        let f = Folders::resolve(None, &linux).unwrap();
        assert_eq!(
            (f.settings.as_path(), f.state.as_path()),
            (
                Path::new("/h/.config/kiln"),
                Path::new("/h/.local/state/kiln/state")
            )
        );
        let windows = Platform {
            config_dir: Some("C:/U/AppData/Roaming".into()),
            state_dir: None,
            data_local_dir: Some("C:/U/AppData/Local".into()),
        };
        assert_eq!(
            Folders::resolve(Some(Path::new("")), &windows)
                .unwrap()
                .state,
            Path::new("C:/U/AppData/Local/kiln/state")
        );
        assert_eq!(Folders::resolve(None, &Platform::default()), None);
    }
}
