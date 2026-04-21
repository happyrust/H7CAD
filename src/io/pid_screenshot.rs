//! Deterministic PNG export of a PID preview document.
//!
//! First-version screenshot pipeline: does NOT read back from GPU. Instead
//! it scans `h7cad_native_model::CadDocument` entities and rasterises them
//! to a fixed-size RGB image using the `image` crate. Kept deterministic
//! on purpose so the 2026-04-21 pid-real-sample plan Task 5 regression
//! test can anchor a stable baseline without depending on wgpu/iced
//! initialisation.
//!
//! **What gets drawn**:
//! - `Line`    → antialiasing-free Bresenham
//! - `Circle`  → midpoint circle rasterisation
//! - `Arc`     → polyline sampling
//! - `MText` / `Text` → 3x3 pixel cross at the insertion anchor (avoids
//!   pulling in a font rasteriser)
//!
//! **What gets skipped** (intentionally):
//! - All other entity kinds (Hatch / Insert / Polyline / Spline / ...) —
//!   the PID preview pipeline only emits lines / circles / text, so the
//!   rasteriser has full coverage of what the preview produces.
//!
//! **Layer filtering**: all entities from every PID layer contribute to
//! the bounding box and render; the caller can prune side-panel layers
//! (`PID_META`, `PID_FALLBACK`, ...) from the input document first if
//! they want a "main drawing only" shot.

use h7cad_native_model::{CadDocument, EntityData};
use image::{ImageBuffer, Rgb, RgbImage};
use std::path::Path;

/// Fixed output dimensions (plan recommends 1600×900 for baseline
/// stability). Keeping this as a const rather than a parameter so the
/// Task 5 regression test gets a byte-identical file across runs.
pub const SCREENSHOT_WIDTH: u32 = 1600;
pub const SCREENSHOT_HEIGHT: u32 = 900;

/// Inset in pixels so drawn geometry never touches the PNG edge.
const MARGIN: i32 = 40;

const BLACK: Rgb<u8> = Rgb([0, 0, 0]);
const WHITE: Rgb<u8> = Rgb([255, 255, 255]);

#[derive(Debug, Clone, Copy)]
struct WorldBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl WorldBounds {
    fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn include(&mut self, x: f64, y: f64) {
        if x < self.min_x {
            self.min_x = x;
        }
        if y < self.min_y {
            self.min_y = y;
        }
        if x > self.max_x {
            self.max_x = x;
        }
        if y > self.max_y {
            self.max_y = y;
        }
    }

    fn is_degenerate(&self) -> bool {
        !self.min_x.is_finite()
            || !self.min_y.is_finite()
            || (self.max_x - self.min_x).abs() < 1e-9
            || (self.max_y - self.min_y).abs() < 1e-9
    }
}

fn compute_bounds(doc: &CadDocument) -> WorldBounds {
    let mut b = WorldBounds::empty();
    for entity in &doc.entities {
        match &entity.data {
            EntityData::Line { start, end } => {
                b.include(start[0], start[1]);
                b.include(end[0], end[1]);
            }
            EntityData::Circle { center, radius } => {
                b.include(center[0] - radius, center[1] - radius);
                b.include(center[0] + radius, center[1] + radius);
            }
            EntityData::Arc {
                center, radius, ..
            } => {
                b.include(center[0] - radius, center[1] - radius);
                b.include(center[0] + radius, center[1] + radius);
            }
            EntityData::Text { insertion, .. } => {
                b.include(insertion[0], insertion[1]);
            }
            EntityData::MText { insertion, .. } => {
                b.include(insertion[0], insertion[1]);
            }
            EntityData::Point { position } => {
                b.include(position[0], position[1]);
            }
            _ => {}
        }
    }
    b
}

/// Compute the world→pixel transform so the input world bbox maps into
/// the `[MARGIN, W-MARGIN] × [MARGIN, H-MARGIN]` drawable area. Flips the
/// Y axis so world-Y-up lands on pixel-Y-down, matching how `fit_all`
/// orients a canvas viewport.
fn fit_transform(bounds: WorldBounds) -> (f64, f64, f64) {
    let w = SCREENSHOT_WIDTH as f64 - 2.0 * MARGIN as f64;
    let h = SCREENSHOT_HEIGHT as f64 - 2.0 * MARGIN as f64;
    let wx = bounds.max_x - bounds.min_x;
    let wy = bounds.max_y - bounds.min_y;
    let scale = (w / wx).min(h / wy).min(f64::INFINITY);
    let cx = 0.5 * (bounds.min_x + bounds.max_x);
    let cy = 0.5 * (bounds.min_y + bounds.max_y);
    let px_cx = SCREENSHOT_WIDTH as f64 / 2.0;
    let px_cy = SCREENSHOT_HEIGHT as f64 / 2.0;
    // return (scale, world→pixel offset_x, world→pixel offset_y),
    // with world_y flip happening where world_to_pixel uses the offset.
    (scale, px_cx - scale * cx, px_cy + scale * cy)
}

fn world_to_pixel(
    wx: f64,
    wy: f64,
    scale: f64,
    ox: f64,
    oy: f64,
) -> Option<(i32, i32)> {
    let px = scale * wx + ox;
    let py = oy - scale * wy;
    if !(px.is_finite() && py.is_finite()) {
        return None;
    }
    Some((px.round() as i32, py.round() as i32))
}

fn put(img: &mut RgbImage, x: i32, y: i32, color: Rgb<u8>) {
    if x < 0 || y < 0 {
        return;
    }
    let (ux, uy) = (x as u32, y as u32);
    if ux >= img.width() || uy >= img.height() {
        return;
    }
    img.put_pixel(ux, uy, color);
}

/// Bresenham line drawing. Deterministic, integer-only, antialiasing-free
/// (stable across platforms → good for pixel-diff baselines).
fn draw_line(img: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb<u8>) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        put(img, x, y, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
}

/// Midpoint circle raster (integer-only, 8-way symmetry).
fn draw_circle(img: &mut RgbImage, cx: i32, cy: i32, r: i32, color: Rgb<u8>) {
    if r <= 0 {
        put(img, cx, cy, color);
        return;
    }
    let mut x = r;
    let mut y = 0;
    let mut err = 1 - r;
    while y <= x {
        for &(dx, dy) in &[
            (x, y), (-x, y), (x, -y), (-x, -y),
            (y, x), (-y, x), (y, -x), (-y, -x),
        ] {
            put(img, cx + dx, cy + dy, color);
        }
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

fn draw_cross(img: &mut RgbImage, cx: i32, cy: i32, half_size: i32, color: Rgb<u8>) {
    for i in -half_size..=half_size {
        put(img, cx + i, cy, color);
        put(img, cx, cy + i, color);
    }
}

/// Render every supported entity from `doc` into a fixed-size RGB image
/// and write it to `path` as a PNG.
///
/// Returns Err on:
/// - doc carries no renderable geometry (entity-wires empty)
/// - `image` fails to save (I/O, encoding)
pub fn export_pid_preview_png(doc: &CadDocument, path: &Path) -> Result<(), String> {
    let bounds = compute_bounds(doc);
    if bounds.is_degenerate() {
        return Err(
            "cannot export PID screenshot: preview has no renderable bounding box".into(),
        );
    }

    let (scale, ox, oy) = fit_transform(bounds);
    let mut img: RgbImage =
        ImageBuffer::from_pixel(SCREENSHOT_WIDTH, SCREENSHOT_HEIGHT, WHITE);

    for entity in &doc.entities {
        match &entity.data {
            EntityData::Line { start, end } => {
                if let (Some(a), Some(b)) = (
                    world_to_pixel(start[0], start[1], scale, ox, oy),
                    world_to_pixel(end[0], end[1], scale, ox, oy),
                ) {
                    draw_line(&mut img, a.0, a.1, b.0, b.1, BLACK);
                }
            }
            EntityData::Circle { center, radius } => {
                if let Some(c) = world_to_pixel(center[0], center[1], scale, ox, oy) {
                    let r = (radius * scale).round() as i32;
                    draw_circle(&mut img, c.0, c.1, r.max(1), BLACK);
                }
            }
            EntityData::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                // Sample the arc at 1° steps between start and end and
                // stitch with Bresenham segments. Arc pixel radius is
                // implied by `radius * scale`; we don't need it
                // explicitly because each sample is resolved via
                // world_to_pixel against the arc equation.
                let sweep = if end_angle >= start_angle {
                    end_angle - start_angle
                } else {
                    end_angle + std::f64::consts::TAU - start_angle
                };
                let steps = (sweep.to_degrees().abs() as i32).max(2);
                let mut prev: Option<(i32, i32)> = None;
                for i in 0..=steps {
                    let t = start_angle + sweep * (i as f64 / steps as f64);
                    let wx = center[0] + radius * t.cos();
                    let wy = center[1] + radius * t.sin();
                    if let Some(p) = world_to_pixel(wx, wy, scale, ox, oy) {
                        if let Some(pr) = prev {
                            draw_line(&mut img, pr.0, pr.1, p.0, p.1, BLACK);
                        }
                        prev = Some(p);
                    }
                }
            }
            EntityData::Text { insertion, .. } | EntityData::MText { insertion, .. } => {
                if let Some(c) = world_to_pixel(insertion[0], insertion[1], scale, ox, oy) {
                    draw_cross(&mut img, c.0, c.1, 3, BLACK);
                }
            }
            EntityData::Point { position } => {
                if let Some(c) = world_to_pixel(position[0], position[1], scale, ox, oy) {
                    put(&mut img, c.0, c.1, BLACK);
                }
            }
            _ => {}
        }
    }

    img.save(path)
        .map_err(|e| format!("failed to write PNG to {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn target_sample_pid_path() -> Option<PathBuf> {
        let path = PathBuf::from(
            r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid",
        );
        path.exists().then_some(path)
    }

    #[test]
    fn export_pid_preview_png_writes_file() {
        let Some(path) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };
        let out = std::env::temp_dir().join("h7cad-pidshot-test.png");
        if out.exists() {
            std::fs::remove_file(&out).ok();
        }
        let bundle = crate::io::pid_import::open_pid(&path).expect("open pid");
        export_pid_preview_png(&bundle.native_preview, &out).expect("export png");
        assert!(out.exists(), "PNG must be written");
        let size = std::fs::metadata(&out).expect("stat").len();
        assert!(size > 1024, "PNG file must have non-trivial size, got {size} bytes");
    }

    #[test]
    fn export_rejects_empty_document() {
        let doc = CadDocument::new();
        let out = std::env::temp_dir().join("h7cad-pidshot-empty.png");
        let err = export_pid_preview_png(&doc, &out).expect_err("empty doc must error");
        assert!(err.contains("renderable bounding box"));
    }

    /// Task 5 regression anchor (plan
    /// `docs/plans/2026-04-21-pid-real-sample-display-and-screenshot-plan.md`).
    ///
    /// Rather than commit a binary baseline PNG (repo-pollution, opaque
    /// diffs), this test pins the **statistical signature** of the
    /// rendered image: exact dimensions, non-empty file size, and a
    /// non-white pixel count within a stable band. These invariants
    /// catch the two main regression modes (blank canvas / filled
    /// canvas) without being so tight that a cosmetic tweak
    /// (one extra label, a moved icon) forces re-baselining on every
    /// commit.
    #[test]
    fn target_pid_sample_screenshot_matches_baseline() {
        use image::GenericImageView;

        let Some(sample) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };

        let actual = std::env::temp_dir().join("h7cad-target-pid-regression.png");
        if actual.exists() {
            std::fs::remove_file(&actual).ok();
        }

        let bundle = crate::io::pid_import::open_pid(&sample).expect("open target pid");
        export_pid_preview_png(&bundle.native_preview, &actual).expect("export png");

        let meta = std::fs::metadata(&actual).expect("stat actual png");
        assert!(
            meta.len() > 1024,
            "PNG must be > 1 KB, got {} bytes",
            meta.len()
        );

        let img = image::open(&actual).expect("decode actual png");
        let (w, h) = img.dimensions();
        assert_eq!(
            (w, h),
            (SCREENSHOT_WIDTH, SCREENSHOT_HEIGHT),
            "PNG dimensions must match SCREENSHOT_WIDTH × SCREENSHOT_HEIGHT"
        );

        let mut non_white: u32 = 0;
        let rgb = img.to_rgb8();
        for pixel in rgb.pixels() {
            let image::Rgb([r, g, b]) = *pixel;
            if r < 250 || g < 250 || b < 250 {
                non_white += 1;
            }
        }
        eprintln!("target pid non_white pixel count = {non_white}");
        // Current observed baseline: ~800 non-white pixels across a
        // 1600×900 white canvas for the target sample (1-pixel-wide
        // Bresenham strokes + 3×3 text anchors × ~60 entities). The
        // [100, 500_000] band is wide enough to absorb modest label /
        // icon tweaks yet still catches the two dangerous regressions:
        //   - blank canvas (goes below 100)
        //   - filled / flood-fill (goes above 500k)
        assert!(
            (100..=500_000).contains(&non_white),
            "non-white pixel count {non_white} outside sanity band [100, 500_000]; \
             the PID preview rasteriser either produced a blank canvas or a filled one"
        );
    }
}
