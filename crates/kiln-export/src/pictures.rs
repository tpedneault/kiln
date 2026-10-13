//! Mermaid diagrams as pictures (EXPORT.md, "Diagrams"): drawn with
//! `fenix-diagram` in its `default` theme, rasterised at scale 2 on
//! white, cached by source hash.
//!
//! Behind a trait so back ends and their golden tests never depend on the
//! fonts a machine has: the PNG's pixels do (Windows and Linux draw text
//! differently), so they can't be part of byte-identical output. Picture
//! names are content-addressed by the source, not the pixels, so they are
//! the same everywhere.

use std::path::PathBuf;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// The `fenix-diagram` revision pictures are drawn with: part of the
/// cache key and the name, so moving the pin redraws. Keep it equal to the
/// workspace `Cargo.toml`'s pin.
pub const FENIX_DIAGRAM_REV: &str = "ce118a2af1a0b70733d0a2dd426f94365cf5eb1a";

/// The scale pictures are rasterised at.
pub const SCALE: f32 = 2.0;

/// The fix named when a machine has no fonts (INTEGRATION.md, G2).
pub const NO_FONTS: &str =
    "no fonts found for diagrams: install fonts-dejavu (Debian/Ubuntu), or set diagrams = \"source\"";

/// A rasterised diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// `diagram-<12 hex>.png`.
    pub name: String,
    pub png: Vec<u8>,
}

/// Draws diagrams.
pub trait Pictures: Sync {
    /// The picture of `source`, or why it can't be drawn (one line).
    fn diagram(&self, source: &str) -> Result<Picture, String>;
}

/// `diagram-<first 12 hex of sha256(source, rev, scale)>.png`.
pub fn picture_name(source: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(source.as_bytes());
    hash.update([0]);
    hash.update(FENIX_DIAGRAM_REV.as_bytes());
    hash.update([0]);
    hash.update(SCALE.to_string().as_bytes());
    let digest = hash.finalize();
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("diagram-{hex}.png")
}

/// The real thing: `fenix-diagram`, with an optional cache folder.
pub struct FenixPictures {
    pub cache: Option<PathBuf>,
}

fn have_fonts() -> bool {
    static FONTS: OnceLock<bool> = OnceLock::new();
    *FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        !db.is_empty()
    })
}

impl Pictures for FenixPictures {
    fn diagram(&self, source: &str) -> Result<Picture, String> {
        let name = picture_name(source);
        let cached = self.cache.as_ref().map(|dir| dir.join(&name));
        if let Some(png) = cached.as_ref().and_then(|p| std::fs::read(p).ok()) {
            return Ok(Picture { name, png });
        }
        // Without fonts the picture would have no text and no error.
        if !have_fonts() {
            return Err(NO_FONTS.to_owned());
        }
        let rendered =
            fenix_diagram::render(source, &fenix_diagram::Themes::default(), Some("default"))
                .map_err(|d| format!("line {}: {}", d.line + 1, d.message))?;
        let png = fenix_diagram::png(&rendered.svg, SCALE, fenix_diagram::Background::White)?;
        if let Some(path) = &cached {
            // A cache that can't be written only costs time next run.
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = fenix_storage::write(path, &png);
        }
        Ok(Picture { name, png })
    }
}

/// Pictures for tests: a fixed tiny PNG per source, or an error for a
/// source starting with `broken`.
pub struct FakePictures;

impl Pictures for FakePictures {
    fn diagram(&self, source: &str) -> Result<Picture, String> {
        if source.trim_start().starts_with("broken") {
            return Err("line 1: not a diagram".into());
        }
        Ok(Picture {
            name: picture_name(source),
            png: b"\x89PNG fake".to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_content_addressed() {
        let a = picture_name("flowchart TD\n a --> b\n");
        assert_eq!(a.len(), "diagram-".len() + 12 + ".png".len());
        assert_eq!(a, picture_name("flowchart TD\n a --> b\n"));
        assert_ne!(a, picture_name("flowchart TD\n a --> c\n"));
    }

    #[test]
    fn real_pictures_are_pngs_and_cached() {
        if !have_fonts() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let pictures = FenixPictures {
            cache: Some(dir.path().to_path_buf()),
        };
        let picture = pictures.diagram("flowchart TD\n  a --> b\n").unwrap();
        assert!(picture.png.starts_with(b"\x89PNG"));
        assert!(dir.path().join(&picture.name).is_file());
        assert!(pictures.diagram("flowchart TD\n  a -->\n").is_err());
    }
}
