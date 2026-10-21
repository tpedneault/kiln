//! Regenerates the committed `kiln.ico` at the repository root from the
//! brand geometry, and the PNGs of the icon, mark and lockups under
//! `docs/brand/`. Builds never need this (the `kiln` crate's build script
//! draws the icon it embeds itself); it keeps the checked-in copies, for
//! the README, the wiki and anything outside a build, current.
//!
//! cargo run -p kiln-brand --example write-icon

use std::path::Path;

fn main() -> std::io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::write(root.join("kiln.ico"), kiln_brand::ico_bytes())?;

    let docs = root.join("docs/brand");
    std::fs::create_dir_all(&docs)?;
    let save = |name: &str, image: kiln_brand::Image| {
        image::save_buffer(
            docs.join(name),
            &image.rgba,
            image.width,
            image.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(std::io::Error::other)
    };
    save("kiln-icon-256.png", kiln_brand::app_icon(256))?;
    save("kiln-mark-512.png", kiln_brand::mark(512))?;
    save(
        "kiln-lockup-dark-bg.png",
        kiln_brand::lockup(160, [0xF4, 0xF4, 0xF6]),
    )?;
    save(
        "kiln-lockup-light-bg.png",
        kiln_brand::lockup(160, [0x17, 0x18, 0x1B]),
    )?;
    println!("wrote kiln.ico and docs/brand/*.png");
    Ok(())
}
