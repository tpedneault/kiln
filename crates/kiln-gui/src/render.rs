//! Draws a [`Screen`]: the top bar, the page's rows (heading bands, the
//! cursor's row, status pills), the footer, and the which-key popup, in
//! the palette's colours; on Home, its bands, rules and lockup, placed in
//! cells as wide as the font's. It knows nothing of pages; [`layout`] is
//! pure and tested.
//!
//! Text goes through glyphon as in Fenix and Flint, in the monospace
//! family Fenix would pick unless settings name one, so columns padded
//! with spaces line up. Loading the system's fonts is the slowest part
//! (a few hundred milliseconds on Windows), done once when the window
//! opens.

use glyphon::{
    Attrs, Buffer, Cache, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport, Wrap,
};

use crate::gpu::{Gpu, Surface};
use crate::image::Picture;
use crate::rect::Rects;
use crate::screen::{Fill, Line, Screen};
use crate::theme::{text_color, Palette};

/// Line height as a multiple of the font size, Fenix's ratio.
const LINE: f32 = 1.25;
/// Padding around the bars, in logical pixels.
const PAD: f32 = 8.0;
/// Columns of text the popup is wide.
const POPUP_COLUMNS: f32 = 26.0;
/// How opaque a pill's band is: its role's colour, faint enough that
/// the text in the same colour reads on it.
const PILL_ALPHA: f32 = 0.22;
/// A pill's height, as a share of the line.
const PILL_HEIGHT: f32 = 0.84;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Where everything goes, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub line: f32,
    pub pad: f32,
    pub top: Rect,
    pub footer: Rect,
    body_top: f32,
    /// How many rows fit.
    pub rows: usize,
    width: f32,
}

/// The layout of a `width`×`height` window at `scale` with `font_size`
/// point text.
pub fn layout(width: f32, height: f32, scale: f32, font_size: f32) -> Layout {
    let line = (font_size * scale * LINE).max(1.0);
    let pad = PAD * scale;
    let bar = line + 2.0 * pad;
    let top = Rect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: bar,
    };
    let footer = Rect {
        x: 0.0,
        y: (height - bar).max(bar),
        w: width,
        h: bar,
    };
    let body_top = bar + pad / 2.0;
    let rows = (((footer.y - body_top) / line).floor() as usize).max(1);
    Layout {
        line,
        pad,
        top,
        footer,
        body_top,
        rows,
        width,
    }
}

impl Layout {
    /// Row `i` of the page.
    pub fn row(&self, i: usize) -> Rect {
        Rect {
            x: 0.0,
            y: self.body_top + i as f32 * self.line,
            w: self.width,
            h: self.line,
        }
    }

    /// How many cells of `cell` pixels fit across the rows.
    pub fn columns(&self, cell: f32) -> usize {
        ((self.width - 2.0 * self.pad) / cell.max(1.0))
            .floor()
            .max(1.0) as usize
    }

    /// The cell at `x`, for cells `cell` pixels wide.
    pub fn column_at(&self, x: f32, cell: f32) -> usize {
        ((x - self.pad) / cell.max(1.0)).floor().max(0.0) as usize
    }

    /// The row at height `y`, if any.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        let i = ((y - self.body_top) / self.line).floor();
        (i >= 0.0 && (i as usize) < self.rows).then_some(i as usize)
    }
}

pub struct Renderer {
    rects: Rects,
    font_system: FontSystem,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text: TextRenderer,
    family: String,
    buffers: Vec<Buffer>,
    /// Home's lockup.
    picture: Picture,
    /// The font's advance at a size: (size, advance).
    cell: Option<(f32, f32)>,
}

impl Renderer {
    /// Builds the pipelines and loads the fonts; `font` is the family
    /// settings ask for, used when installed.
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, font: Option<&str>) -> Renderer {
        let font_system = FontSystem::new();
        let family = font
            .filter(|f| installed(&font_system, f, false))
            .map(str::to_owned)
            .unwrap_or_else(|| monospace_family(&font_system));
        let cache = Cache::new(&gpu.device);
        let viewport = Viewport::new(&gpu.device, &cache);
        let mut atlas = TextAtlas::new(&gpu.device, &gpu.queue, &cache, format);
        let text = TextRenderer::new(
            &mut atlas,
            &gpu.device,
            wgpu::MultisampleState::default(),
            None,
        );
        Renderer {
            picture: Picture::new(gpu, format),
            cell: None,
            rects: Rects::new(&gpu.device, format),
            font_system,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text,
            family,
            buffers: Vec::new(),
        }
    }

    /// How wide a cell is at `l`'s size: the monospace font's advance,
    /// measured once a size, so Home's bands sit exactly under its text.
    pub fn cell_width(&mut self, l: &Layout) -> f32 {
        let size = l.line / LINE;
        if let Some((at, w)) = self.cell {
            if at == size {
                return w;
            }
        }
        const N: usize = 20;
        let mut b = Buffer::new(&mut self.font_system, Metrics::new(size, l.line));
        b.set_wrap(Wrap::None);
        b.set_size(None, None);
        let attrs = Attrs::new().family(Family::Name(&self.family));
        b.set_text(&"0".repeat(N), &attrs, Shaping::Advanced, None);
        b.shape_until_scroll(&mut self.font_system, false);
        let w = b.layout_runs().map(|r| r.line_w).fold(0.0_f32, f32::max) / N as f32;
        // A font that shapes nothing: monospace fonts' usual ratio.
        let w = if w > 0.0 { w } else { size * 0.6 };
        self.cell = Some((size, w));
        w
    }

    /// Draws `screen` and presents it; `false` when the surface wasn't
    /// available (the caller redraws later).
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        surface: &mut Surface,
        screen: &Screen,
        l: &Layout,
        palette: &Palette,
    ) -> bool {
        let Some(frame) = surface.frame(gpu) else {
            return false;
        };
        let (width, height) = (surface.config.width as f32, surface.config.height as f32);
        let cell = self.cell_width(l);
        // Home's lockup: three rows tall, drawn at the height it shows at.
        let logo = screen.logo.map(|(row, col)| {
            let h = (3.0 * l.line * 0.8).round() as u32;
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let fg = palette.foreground;
            let (w, h) = self
                .picture
                .lockup(gpu, h, [byte(fg[0]), byte(fg[1]), byte(fg[2])]);
            let (w, h) = (w as f32, h as f32);
            Rect {
                x: (l.pad + col as f32 * cell).round(),
                y: (l.row(row).y + (3.0 * l.line - h) / 2.0).round(),
                w,
                h,
            }
        });
        if let Some(r) = logo {
            self.picture.place(gpu, r, width, height);
        }
        self.rects.clear();
        let mut push = |r: Rect, c: [f32; 4]| self.rects.push(r, c, width, height);
        push(l.top, palette.surface);
        push(l.footer, palette.surface);
        for &h in &screen.headings {
            push(l.row(h), palette.border);
        }
        if let Some(c) = screen.cursor {
            push(l.row(c), palette.selection);
        }
        // Home's bands, in cells; the cursor's with Fenix's focus rail.
        let rail = (l.pad / 4.0).max(2.0).round();
        for b in &screen.bands {
            let r = Rect {
                x: l.pad + b.cols.start as f32 * cell,
                y: l.row(b.row).y,
                w: b.cols.len() as f32 * cell,
                h: b.rows as f32 * l.line,
            };
            match b.fill {
                Fill::Panel => push(r, palette.surface),
                Fill::Key => push(r, palette.border),
                Fill::Cursor => {
                    push(r, palette.selection);
                    push(Rect { w: rail, ..r }, palette.accent);
                }
            }
        }
        let hairline = (l.pad / 8.0).max(1.0).round();
        for (row, cols) in &screen.rules {
            let mut color = palette.muted;
            color[3] *= 0.35;
            push(
                Rect {
                    x: l.pad + cols.start as f32 * cell,
                    y: (l.row(*row).y + (l.line - hairline) / 2.0).round(),
                    w: cols.len() as f32 * cell,
                    h: hairline,
                },
                color,
            );
        }
        // The which-key popup: bottom right, above the footer.
        let popup_w = POPUP_COLUMNS * l.line * 0.6 + 2.0 * l.pad;
        let popup_h = screen.popup.len() as f32 * l.line + 2.0 * l.pad;
        let popup = Rect {
            x: (width - popup_w - l.pad).max(0.0),
            y: (l.footer.y - popup_h - l.pad).max(l.top.h),
            w: popup_w.min(width),
            h: popup_h,
        };
        if !screen.popup.is_empty() {
            push(popup, palette.surface);
            let edge = (1.0_f32).max(l.pad / 8.0);
            push(Rect { h: edge, ..popup }, palette.accent);
        }

        // One buffer per line: the bar, the rows, the footer, the popup.
        let mut lines: Vec<(&Line, f32, f32, f32)> = Vec::new();
        let text_top = |r: Rect| r.y + (r.h - l.line) / 2.0;
        lines.push((&screen.top, l.pad, text_top(l.top), width - l.pad));
        for (i, row) in screen.rows.iter().enumerate() {
            lines.push((row, l.pad, l.row(i).y, width - l.pad));
        }
        lines.push((&screen.footer, l.pad, text_top(l.footer), width - l.pad));
        for (i, line) in screen.popup.iter().enumerate() {
            lines.push((
                line,
                popup.x + l.pad,
                popup.y + l.pad + i as f32 * l.line,
                popup.x + popup.w - l.pad,
            ));
        }
        let size = l.line / LINE;
        while self.buffers.len() < lines.len() {
            let mut b = Buffer::new(&mut self.font_system, Metrics::new(size, l.line));
            b.set_wrap(Wrap::None);
            self.buffers.push(b);
        }
        let attrs = Attrs::new().family(Family::Name(&self.family));
        for (buffer, (line, _, _, _)) in self.buffers.iter_mut().zip(&lines) {
            buffer.set_metrics(Metrics::new(size, l.line));
            buffer.set_size(None, None);
            // Each span's index rides on its glyphs, so a pill finds its
            // own extent after shaping, whatever the font's advances.
            let spans: Vec<(&str, Attrs)> = line
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    (
                        s.text.as_str(),
                        attrs
                            .clone()
                            .color(text_color(palette.of(s.role)))
                            .metadata(i),
                    )
                })
                .collect();
            buffer.set_rich_text(spans, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.font_system, false);
        }
        // Pills: a rounded band behind each, over the row's bands.
        for (buffer, (line, left, top, right)) in self.buffers.iter().zip(&lines) {
            for (i, s) in line.iter().enumerate().filter(|(_, s)| s.pill) {
                let Some((x0, x1)) = extent(buffer, i) else {
                    continue;
                };
                let (x0, x1) = (left + x0, (left + x1).min(*right));
                if x1 <= x0 {
                    continue;
                }
                let h = l.line * PILL_HEIGHT;
                let mut color = palette.of(s.role);
                color[3] *= PILL_ALPHA;
                self.rects.push_rounded(
                    Rect {
                        x: x0,
                        y: top + (l.line - h) / 2.0,
                        w: x1 - x0,
                        h,
                    },
                    color,
                    h / 2.0,
                    width,
                    height,
                );
            }
        }
        let areas = self
            .buffers
            .iter()
            .zip(&lines)
            .map(|(buffer, (_, left, top, right))| TextArea {
                buffer,
                left: *left,
                top: *top,
                scale: 1.0,
                bounds: TextBounds {
                    left: *left as i32,
                    top: 0,
                    right: right.max(*left) as i32,
                    bottom: height as i32,
                },
                default_color: text_color(palette.foreground),
                custom_glyphs: &[],
            });
        self.viewport.update(
            &gpu.queue,
            Resolution {
                width: surface.config.width,
                height: surface.config.height,
            },
        );
        if self
            .text
            .prepare(
                &gpu.device,
                &gpu.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash,
            )
            .is_err()
        {
            // The atlas is full: a frame missing some text beats none.
            self.atlas.trim();
        }
        self.rects.upload(&gpu.device, &gpu.queue);
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kiln-frame"),
            });
        {
            let [r, g, b, a] = palette.background.map(f64::from);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kiln-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.rects.draw(&mut pass);
            if logo.is_some() {
                self.picture.draw(&mut pass);
            }
            let _ = self.text.render(&self.atlas, &self.viewport, &mut pass);
        }
        gpu.queue.submit(Some(encoder.finish()));
        gpu.queue.present(frame);
        self.atlas.trim();
        true
    }
}

/// Where span `i`'s glyphs start and end on the line, from the left of
/// its text area.
fn extent(buffer: &Buffer, i: usize) -> Option<(f32, f32)> {
    let mut out: Option<(f32, f32)> = None;
    for run in buffer.layout_runs() {
        for g in run.glyphs.iter().filter(|g| g.metadata == i) {
            let (a, b) = (g.x, g.x + g.w);
            out = Some(out.map_or((a, b), |(x0, x1)| (x0.min(a), x1.max(b))));
        }
    }
    out
}

fn installed(fs: &FontSystem, family: &str, mono: bool) -> bool {
    fs.db().faces().any(|face| {
        (!mono || face.monospaced)
            && face
                .families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
    })
}

/// The monospace family Fenix would pick: the system's own, then the
/// common ones, whichever is installed.
fn monospace_family(fs: &FontSystem) -> String {
    let system = fs.db().family_name(&Family::Monospace).to_owned();
    for family in [
        system.as_str(),
        "Cascadia Mono",
        "Consolas",
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Noto Sans Mono",
        "Courier New",
    ] {
        if installed(fs, family, true) {
            return family.to_owned();
        }
    }
    system
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_between_the_bars() {
        let l = layout(1000.0, 600.0, 1.0, 16.0);
        assert_eq!(l.line, 20.0);
        assert_eq!(l.top.h, 36.0);
        assert_eq!(l.footer.y, 564.0);
        assert_eq!(l.rows, 26);
        assert_eq!(l.row_at(l.row(3).y + 1.0), Some(3));
        assert_eq!(l.row_at(5.0), None);
        let tiny = layout(100.0, 10.0, 2.0, 16.0);
        assert_eq!(tiny.rows, 1, "never zero rows");
    }
}
