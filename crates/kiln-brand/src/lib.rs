//! The Kiln identity as code: the kiln mark, the wordmark and the app
//! icon, all drawn from one vector definition by a small rasteriser, so
//! the window icon, the `.ico` embedded in every Windows build and the
//! committed `kiln.ico` and `docs/brand/` images can't drift apart.
//!
//! It is drawn the way Fenix's `fenix-brand` draws the Forged F, so the two
//! sit together as a suite: a 48-unit grid, every slanted edge rising
//! 1 : 2, flat colour, the same graphite tile and the same wordmark face.
//! A 1 : 2 slope is also the slope of an isometric drawing, and the mark
//! is a fired block on that grid: three faces in clay (brick, terracotta,
//! bisque) standing a little apart, as Fenix's blades do, with Fenix's own
//! Flame burning in its mouth. A letter K was tried first; with arms held
//! to 1 : 2 it could only be Fenix's F with a leg turned down, and read as
//! an F at a glance. The wordmark is "kiln" in Martian Mono SemiBold at
//! −4 % tracking, kept as outlines (`wordmark.rs`) so no font ships.
//!
//! It deliberately draws nothing else: no splash, no animation, no
//! in-window logo. The GUI only needs its window icon.

mod raster;
mod wordmark;

pub use raster::Image;
use raster::{rounded_rect, Canvas};

pub const BRICK: [u8; 3] = [0xB4, 0x46, 0x2E];
pub const TERRACOTTA: [u8; 3] = [0xD9, 0x82, 0x4A];
pub const BISQUE: [u8; 3] = [0xED, 0xC0, 0x8C];
/// Fenix's Flame (`fenix_brand::FLAME`): the one colour the two marks
/// share, as the fire the kiln holds.
pub const FLAME: [u8; 3] = [0xFF, 0xB5, 0x47];
/// The app icon's tile and its hairline edge, Fenix's (graphite 800/700).
const TILE: [u8; 3] = [0x1D, 0x1E, 0x22];
const TILE_EDGE: [u8; 3] = [0x2F, 0x30, 0x36];

/// One face of the mark: its colour and its four corners.
type Face = ([u8; 3], [(f32, f32); 4]);

/// The block on the 48-unit grid. Unsplit, its top vertex is (24, 3.5),
/// its side vertices at x 6 and 42, its front edge at x 24, 20 units tall;
/// each face is then pushed 1.5 units out along its own direction (the top
/// up, the sides out and down a half), which opens the seams between them.
/// The mouth is one blade wide (8 units), 10 tall, standing on the left
/// face's foot.
const MARK: [Face; 4] = [
    (
        BISQUE,
        [(24.0, 2.0), (42.0, 11.0), (24.0, 20.0), (6.0, 11.0)],
    ),
    (
        BRICK,
        [(4.5, 12.25), (22.5, 21.25), (22.5, 41.25), (4.5, 32.25)],
    ),
    (
        TERRACOTTA,
        [(25.5, 21.25), (43.5, 12.25), (43.5, 32.25), (25.5, 41.25)],
    ),
    (
        FLAME,
        [(9.5, 24.75), (17.5, 28.75), (17.5, 38.75), (9.5, 34.75)],
    ),
];
const GRID: f32 = 48.0;
/// Shifts the faces above down so the drawing sits centred in the box:
/// it spans x 4.5..43.5 and, dropped, y 4.375..43.625.
const DROP: f32 = 2.375;
/// Where the mark stands within the grid: its top and its foot (the front
/// edge's lowest point), and its right edge.
const MARK_TOP: f32 = 2.0 + DROP;
const MARK_FOOT: f32 = 41.25 + DROP;
const MARK_RIGHT: f32 = 43.5;
/// The wordmark's baseline. The block's foot is one vertex, not a flat
/// edge like Fenix's stem, so a baseline on it looks low; this sits the
/// word on the block optically.
const BASELINE: f32 = 43.0;
const BLADE: f32 = 8.0;

fn draw_mark(canvas: &mut Canvas, x: f32, y: f32, box_size: f32) {
    let unit = box_size / GRID;
    for (color, points) in MARK {
        let contour: Vec<(f32, f32)> = points
            .iter()
            .map(|&(px, py)| (x + px * unit, y + (py + DROP) * unit))
            .collect();
        canvas.fill(&[contour], color, 1.0);
    }
}

/// The bare mark, `size` pixels square, on transparency.
pub fn mark(size: u32) -> Image {
    let mut canvas = Canvas::new(size, size);
    draw_mark(&mut canvas, 0.0, 0.0, size as f32);
    canvas.into_image()
}

/// The app icon: the mark at 60 % on a graphite tile with a 22.5 % corner
/// radius and a hairline edge, as Fenix's. At 16 px and below the tile
/// goes: there are too few pixels for both, and the mark is what has to
/// read.
pub fn app_icon(size: u32) -> Image {
    if size <= 16 {
        return mark(size);
    }
    let s = size as f32;
    let mut canvas = Canvas::new(size, size);
    let radius = s * 0.225;
    let edge = (s / 128.0).max(1.0);
    canvas.fill(&[rounded_rect(0.0, 0.0, s, s, radius)], TILE_EDGE, 1.0);
    canvas.fill(
        &[rounded_rect(
            edge,
            edge,
            s - 2.0 * edge,
            s - 2.0 * edge,
            radius - edge,
        )],
        TILE,
        1.0,
    );
    let box_size = s * 0.6;
    draw_mark(
        &mut canvas,
        (s - box_size) / 2.0,
        (s - box_size) / 2.0,
        box_size,
    );
    canvas.into_image()
}

/// The horizontal lockup, mark then "kiln" in `text`, `height` pixels tall
/// (the mark's full 48-unit box). Fenix's proportions: the mark is 1.4 ×
/// the wordmark's cap height, one blade width between them.
pub fn lockup(height: u32, text: [u8; 3]) -> Image {
    let unit = height as f32 / GRID;
    let em = ((MARK_FOOT - MARK_TOP) * unit / 1.4) / wordmark::CAP_HEIGHT;
    let origin_x = (MARK_RIGHT + BLADE) * unit - wordmark::MIN_X * em;
    let baseline = BASELINE * unit;
    let width = (origin_x + wordmark::MAX_X * em + BLADE * unit / 2.0).ceil() as u32;

    let mut canvas = Canvas::new(width, height);
    draw_mark(&mut canvas, 0.0, 0.0, height as f32);
    let contours: Vec<Vec<(f32, f32)>> = wordmark::CONTOURS
        .iter()
        .map(|c| {
            c.iter()
                .map(|&(x, y)| (origin_x + x * em, baseline + y * em))
                .collect()
        })
        .collect();
    canvas.fill(&contours, text, 1.0);
    canvas.into_image()
}

/// The sizes packed into `kiln.ico`: what Windows picks from for the
/// taskbar, Alt-Tab, Explorer's views and the title bar.
pub const ICO_SIZES: [u32; 6] = [256, 64, 48, 32, 24, 16];

/// `kiln.ico`: every size in `ICO_SIZES`, PNG-compressed.
#[cfg(feature = "ico")]
pub fn ico_bytes() -> Vec<u8> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let frames: Vec<IcoFrame> = ICO_SIZES
        .iter()
        .map(|&size| {
            let icon = app_icon(size);
            IcoFrame::as_png(&icon.rgba, size, size, image::ExtendedColorType::Rgba8)
                .expect("an RGBA frame of a valid icon size encodes")
        })
        .collect();
    let mut out = Vec::new();
    IcoEncoder::new(&mut out)
        .encode_images(&frames)
        .expect("encoding into memory cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * image.width + x) * 4) as usize;
        image.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn the_mark_puts_each_face_where_the_grid_says() {
        let m = mark(48);
        assert_eq!(&pixel(&m, 24, 14)[..3], &BISQUE, "top");
        assert_eq!(&pixel(&m, 8, 22)[..3], &BRICK, "left face");
        assert_eq!(&pixel(&m, 36, 30)[..3], &TERRACOTTA, "right face");
        assert_eq!(&pixel(&m, 13, 35)[..3], &FLAME, "the mouth");
        assert_eq!(pixel(&m, 2, 2)[3], 0, "outside the block is transparent");
        assert_eq!(
            pixel(&m, 24, 38)[3],
            0,
            "the seam between the sides is open"
        );
    }

    #[test]
    fn the_mark_sits_centred_in_its_box() {
        let m = mark(96);
        let inked = |x: u32, y: u32| pixel(&m, x, y)[3] > 0;
        let rows: Vec<u32> = (0..96).filter(|&y| (0..96).any(|x| inked(x, y))).collect();
        let cols: Vec<u32> = (0..96).filter(|&x| (0..96).any(|y| inked(x, y))).collect();
        let (top, bottom) = (rows[0], 95 - rows[rows.len() - 1]);
        let (left, right) = (cols[0], 95 - cols[cols.len() - 1]);
        assert!(top.abs_diff(bottom) <= 1, "top {top}, bottom {bottom}");
        assert!(left.abs_diff(right) <= 1, "left {left}, right {right}");
    }

    #[test]
    fn edges_are_antialiased() {
        let m = mark(96);
        let alphas: Vec<u8> = (0..96).map(|x| pixel(&m, x, 30)[3]).collect();
        assert!(
            alphas.iter().any(|&a| a > 0 && a < 255),
            "no partial coverage along a row"
        );
    }

    #[test]
    fn the_icon_tile_is_opaque_graphite_and_rounded() {
        let icon = app_icon(256);
        assert_eq!(pixel(&icon, 0, 0)[3], 0, "the corner is cut away");
        assert_eq!(pixel(&icon, 128, 250), [TILE[0], TILE[1], TILE[2], 255]);
        assert_eq!(
            &pixel(&icon, 128, 85)[..3],
            &BISQUE,
            "the mark sits in the middle"
        );
    }

    #[test]
    fn tiny_icons_drop_the_tile() {
        assert_eq!(app_icon(16).rgba, mark(16).rgba);
    }

    #[test]
    fn the_lockup_is_the_mark_then_the_word() {
        let l = lockup(96, [0xF2, 0xF2, 0xF2]);
        assert!(
            l.width > 96 * 2,
            "the word is wider than the mark: {}",
            l.width
        );
        let word_ink = (100..l.width)
            .flat_map(|x| (0..96).map(move |y| (x, y)))
            .filter(|&(x, y)| pixel(&l, x, y)[3] == 255)
            .count();
        assert!(
            word_ink > 1000,
            "the wordmark was drawn: {word_ink} solid pixels"
        );
    }

    #[cfg(feature = "ico")]
    #[test]
    fn the_ico_carries_every_size() {
        let bytes = ico_bytes();
        let count = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(count as usize, ICO_SIZES.len());
    }

    /// The committed `kiln.ico` (for anyone using the icon outside a
    /// build) is the one this crate draws. Compared as pixels, not bytes,
    /// so a PNG encoder update can't fail it. Regenerate with `cargo run
    /// -p kiln-brand --example write-icon`.
    #[cfg(feature = "ico")]
    #[test]
    fn the_committed_kiln_ico_is_the_current_icon() {
        let committed = image::load_from_memory_with_format(
            include_bytes!("../../../kiln.ico"),
            image::ImageFormat::Ico,
        )
        .expect("kiln.ico decodes")
        .to_rgba8();
        let expected = app_icon(committed.width());
        let worst = committed
            .as_raw()
            .iter()
            .zip(&expected.rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(
            worst <= 2,
            "kiln.ico differs from kiln-brand's icon (worst channel diff {worst}); run the write-icon example"
        );
    }
}
