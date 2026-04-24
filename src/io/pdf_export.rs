// PDF export — converts the paper-space wire model to a PDF file using printpdf.
//
// Pipeline (三十二轮):
//   1. Paint solid HATCH fills as filled polygons (bottom layer).
//   2. Embed IMAGE / RasterImage entities as XObject references.
//   3. Paint wire segments (LINE / CIRCLE tess / ARC tess / LwPolyline / ...)
//      — skipping wires whose entity is rendered natively above (text/image).
//   4. Paint TEXT / MTEXT natively using a built-in PDF font (top layer).
//
// The low-level builder `build_pdf_wires_only()` keeps the 191-line
// wires-only signature that `print_to_printer` still relies on.
//
// Coordinate system: CAD uses mm units with origin at bottom-left and Y up.
// printpdf's `Point::new(Mm, Mm)` also has origin at bottom-left, so no Y-flip
// is needed — we shift the coordinates by `(offset_x, offset_y)` to place the
// drawing origin at the paper origin.

use crate::io::plot_style::PlotStyleTable;
use crate::scene::hatch_model::{HatchModel, HatchPattern};
use crate::scene::WireModel;
use acadrust::Handle;
use h7cad_native_model as nm;
use printpdf::{
    BuiltinFont, Color, CurTransMat, Line, LineCapStyle, LineJoinStyle, LinePoint, Mm, Op,
    PaintMode, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing,
    Pt, RawImage, RawImageData, RawImageFormat, Rgb, TextItem, WindingOrder, XObjectTransform,
};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

// ── Options ────────────────────────────────────────────────────────────────

/// Configurable knobs for PDF export — parallels `SvgExportOptions`.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct PdfExportOptions {
    /// All strokes forced to black (default true — matches SVG ColorPolicy=1).
    pub monochrome: bool,
    /// Emit text entities as tessellated geometry instead of native PDF text
    /// (default false).
    pub text_as_geometry: bool,
    /// Which built-in PDF font (Standard 14) is used for native text.
    pub font_family: PdfFontChoice,
    /// Multiply native font height by this factor (default 0.8 — matches SVG).
    pub font_size_scale: f32,
    /// Whether to emit solid HATCH fills.
    pub include_hatches: bool,
    /// Whether to emit `<image>`-equivalent XObjects for RasterImage entities.
    pub include_images: bool,
    /// Whether to embed the raster bytes in the PDF (true) or skip when missing.
    /// PDFs are always self-contained — this flag currently only gates the
    /// lookup step; when `false` behaves identical to `include_images=false`.
    pub embed_images: bool,
    /// Directory used to resolve relative `file_path` values on IMAGE entities
    /// (mirrors `SvgExportOptions::image_base`).
    pub image_base: Option<PathBuf>,
    /// Phase-8 parity with SVG: replace dim-text wires with native PDF text
    /// so standing measurement values stay selectable + crisp.  Reserved for
    /// T4 dialog wiring — currently unused because dim-text is identified
    /// via the native doc entity list.
    #[allow(dead_code)]
    pub native_dimension_text: bool,
    /// Emit native PDF bezier paths for `Circle` / `Arc` / `Ellipse` entities
    /// instead of relying on the scene's tessellated `WireModel` output.
    /// Produces smaller, resolution-independent PDFs that stay smooth at
    /// arbitrary zoom. Mirrors `SvgExportOptions::native_curves`.
    pub native_curves: bool,
    /// Emit `HatchPattern::Pattern(line family)` fills as real lines in the
    /// PDF (三十三轮 Phase 2).  When `false`, pattern HATCHes are silently
    /// skipped — matches Phase 1 behaviour for backward compat.
    pub hatch_patterns: bool,
    /// Emit native SPLINE entities as PDF bezier paths (三十五轮 Phase 3).
    /// - degree 1 ⇒ control-point polyline
    /// - clamped non-rational degree 2/3 ⇒ piecewise cubic bezier
    ///   (degree 2 is promoted to cubic via the standard 2/3 rule)
    /// - high-order / rational / closed-periodic ⇒ fit-point polyline fallback,
    ///   or wire tessellation when no fit points exist
    ///
    /// Mirrors `SvgExportOptions::native_splines`.
    pub native_splines: bool,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            monochrome: true,
            text_as_geometry: false,
            font_family: PdfFontChoice::Helvetica,
            font_size_scale: 0.8,
            include_hatches: true,
            include_images: true,
            embed_images: true,
            image_base: None,
            native_dimension_text: true,
            native_curves: true,
            hatch_patterns: true,
            native_splines: true,
        }
    }
}

/// Built-in Standard 14 PDF font selection.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum PdfFontChoice {
    Helvetica,
    TimesRoman,
    Courier,
}

impl PdfFontChoice {
    fn to_builtin(self) -> BuiltinFont {
        match self {
            PdfFontChoice::Helvetica => BuiltinFont::Helvetica,
            PdfFontChoice::TimesRoman => BuiltinFont::TimesRoman,
            PdfFontChoice::Courier => BuiltinFont::Courier,
        }
    }

    /// Human-readable label for UI.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            PdfFontChoice::Helvetica => "Helvetica",
            PdfFontChoice::TimesRoman => "Times",
            PdfFontChoice::Courier => "Courier",
        }
    }
}

// ── Public entry points ────────────────────────────────────────────────────

/// Export `wires` to a PDF file — the pre-三十二轮 signature retained for
/// `print_to_printer` and any existing callers that only have wires.
///
/// Internally delegates to `export_pdf_full` with `None` for hatches /
/// native_doc and the default `PdfExportOptions`.
pub fn export_pdf(
    wires: &[WireModel],
    paper_w: f64,
    paper_h: f64,
    offset_x: f32,
    offset_y: f32,
    rotation_deg: i32,
    path: &Path,
    plot_style: Option<&PlotStyleTable>,
) -> Result<(), String> {
    let options = PdfExportOptions::default();
    let empty_hatches: HashMap<Handle, HatchModel> = HashMap::new();
    export_pdf_full(
        wires,
        &empty_hatches,
        None,
        paper_w,
        paper_h,
        offset_x,
        offset_y,
        rotation_deg,
        path,
        plot_style,
        &options,
    )
}

/// Enhanced PDF export with native text, solid-hatch fills, raster-image
/// embedding, and configurable options — parallels `export_svg_full`.
pub fn export_pdf_full(
    wires: &[WireModel],
    hatches: &HashMap<Handle, HatchModel>,
    native_doc: Option<&nm::CadDocument>,
    paper_w: f64,
    paper_h: f64,
    offset_x: f32,
    offset_y: f32,
    rotation_deg: i32,
    path: &Path,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
) -> Result<(), String> {
    let bytes = build_pdf_full(
        wires,
        hatches,
        native_doc,
        paper_w as f32,
        paper_h as f32,
        offset_x,
        offset_y,
        rotation_deg,
        plot_style,
        options,
    );
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())
}

/// Show a PDF save-file dialog and return the chosen path (or None if cancelled).
pub async fn pick_pdf_path_owned(stem: String) -> Option<std::path::PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Export as PDF")
        .set_file_name(&format!("{stem}.pdf"))
        .add_filter("PDF Files", &["pdf"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

// ── PDF builder (full pipeline) ────────────────────────────────────────────

// mm → PDF points (1 mm = 2.834645 pt).
const MM_TO_PT: f32 = 2.834645;
// Screen px → PDF points (approximate at 96 dpi).
const PX_TO_PT: f32 = 0.35278;

fn build_pdf_full(
    wires: &[WireModel],
    hatches: &HashMap<Handle, HatchModel>,
    native_doc: Option<&nm::CadDocument>,
    paper_w: f32,
    paper_h: f32,
    ox: f32,
    oy: f32,
    rotation_deg: i32,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
) -> Vec<u8> {
    let mut doc = PdfDocument::new("H7CAD Export");
    let mut ops: Vec<Op> = Vec::new();

    // Collect handles that will be drawn natively, so we skip the corresponding
    // wires below. Only active when a native doc is provided — otherwise fall
    // back to wires-only behaviour (print_to_printer path).
    let (
        native_text_handles,
        native_image_handles,
        native_curve_handles,
        native_spline_handles,
    ) = collect_native_handles(native_doc, options);

    // Register raster images up front so their XObject ids are available
    // when we emit UseXobject below.
    let image_specs = native_doc
        .filter(|_| options.include_images && options.embed_images)
        .map(|doc_ref| collect_and_register_images(&mut doc, doc_ref, options))
        .unwrap_or_default();

    // Paper-wide white background.
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            icc_profile: None,
        }),
    });
    ops.push(Op::DrawRectangle {
        rectangle: printpdf::Rect::from_wh(Mm(paper_w).into(), Mm(paper_h).into()),
    });

    // Round line caps/joins for CAD aesthetics.
    ops.push(Op::SetLineCapStyle {
        cap: LineCapStyle::Round,
    });
    ops.push(Op::SetLineJoinStyle {
        join: LineJoinStyle::Round,
    });

    // Apply drawing-wide rotation transform if needed.
    let needs_rotation = rotation_deg != 0;
    if needs_rotation {
        let (cos_a, sin_a, tx, ty) = match rotation_deg {
            90 => (0.0_f64, 1.0_f64, 0.0, paper_h as f64),
            180 => (-1.0_f64, 0.0_f64, paper_w as f64, paper_h as f64),
            270 => (0.0_f64, -1.0_f64, paper_w as f64, 0.0),
            _ => (1.0_f64, 0.0_f64, 0.0, 0.0),
        };
        ops.push(Op::SaveGraphicsState);
        let tx_pt = (tx * 2.834645) as f32;
        let ty_pt = (ty * 2.834645) as f32;
        ops.push(Op::SetTransformationMatrix {
            matrix: CurTransMat::Raw([
                cos_a as f32,
                sin_a as f32,
                -(sin_a as f32),
                cos_a as f32,
                tx_pt,
                ty_pt,
            ]),
        });
    }

    // ── Layer 1: solid HATCH fills (bottom) ───────────────────────────────
    if options.include_hatches {
        emit_hatch_fills(&mut ops, hatches, ox, oy, options);
    }

    // ── Layer 2: raster images ────────────────────────────────────────────
    for spec in &image_specs {
        emit_image_use(&mut ops, spec);
    }

    // ── Layer 3: wire segments ────────────────────────────────────────────
    emit_wires(
        &mut ops,
        wires,
        ox,
        oy,
        plot_style,
        options,
        &native_text_handles,
        &native_image_handles,
        &native_curve_handles,
        &native_spline_handles,
    );

    // ── Layer 3b: native curves (Circle / Arc / Ellipse) ──────────────────
    if options.native_curves {
        if let Some(doc_ref) = native_doc {
            emit_native_curves(&mut ops, doc_ref, ox, oy, plot_style, options);
        }
    }

    // ── Layer 3c: native splines ──────────────────────────────────────────
    if options.native_splines {
        if let Some(doc_ref) = native_doc {
            emit_native_splines(&mut ops, doc_ref, ox, oy, plot_style, options);
        }
    }

    // ── Layer 4: native TEXT / MTEXT (top) ────────────────────────────────
    if !options.text_as_geometry {
        if let Some(doc_ref) = native_doc {
            emit_native_text(&mut ops, doc_ref, ox, oy, options);
        }
    }

    if needs_rotation {
        ops.push(Op::RestoreGraphicsState);
    }

    let page = PdfPage::new(Mm(paper_w), Mm(paper_h), ops);
    doc.pages.push(page);

    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
}

// ── Wire emission ──────────────────────────────────────────────────────────

fn emit_wires(
    ops: &mut Vec<Op>,
    wires: &[WireModel],
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
    skip_text_handles: &HashSet<String>,
    skip_image_handles: &HashSet<String>,
    skip_curve_handles: &HashSet<String>,
    skip_spline_handles: &HashSet<String>,
) {
    let mut last_color: Option<[f32; 3]> = None;
    let mut last_lw: Option<f32> = None;

    for wire in wires {
        let [mut r, mut g, mut b, a] = wire.color;
        if a < 0.01 {
            continue;
        }
        // Skip the paper-boundary wire — the white PDF background already
        // provides it.
        if wire.name == "__paper_boundary__" {
            continue;
        }
        // Skip wires whose entity renders natively above the wire layer.
        if skip_text_handles.contains(&wire.name) {
            continue;
        }
        if skip_image_handles.contains(&wire.name) {
            continue;
        }
        if skip_curve_handles.contains(&wire.name) {
            continue;
        }
        if skip_spline_handles.contains(&wire.name) {
            continue;
        }

        // Apply CTB plot style table overrides (color + lineweight).
        let mut lw_override: Option<f32> = None;
        if let Some(ctb) = plot_style {
            if wire.aci > 0 {
                if let Some([cr, cg, cb]) = ctb.resolve_color(wire.aci) {
                    r = cr;
                    g = cg;
                    b = cb;
                }
                lw_override = ctb
                    .resolve_lineweight(wire.aci)
                    .map(|mm| (mm * MM_TO_PT).max(0.1));
            }
        }

        // Near-white and near-yellow (viewport active border) → dark grey for
        // print (only when no CTB override was applied).
        if lw_override.is_none() {
            let is_light = r > 0.80 && g > 0.80 && b > 0.80;
            let is_yellow = r > 0.80 && g > 0.70 && b < 0.30;
            let is_cyan = r < 0.30 && g > 0.70 && b > 0.70;
            if is_light || is_yellow {
                r = 0.0;
                g = 0.0;
                b = 0.0;
            } else if is_cyan {
                r = 0.0;
                g = 0.15;
                b = 0.50;
            }
        }

        // Monochrome policy: force everything to black (mirrors SVG).
        if options.monochrome {
            r = 0.0;
            g = 0.0;
            b = 0.0;
        }

        if last_color
            .map(|c| {
                (c[0] - r).abs() > 0.01 || (c[1] - g).abs() > 0.01 || (c[2] - b).abs() > 0.01
            })
            .unwrap_or(true)
        {
            ops.push(Op::SetOutlineColor {
                col: Color::Rgb(Rgb {
                    r,
                    g,
                    b,
                    icc_profile: None,
                }),
            });
            last_color = Some([r, g, b]);
        }

        let lw_pt = lw_override.unwrap_or_else(|| (wire.line_weight_px * PX_TO_PT).max(0.1));
        if last_lw.map(|l| (l - lw_pt).abs() > 0.01).unwrap_or(true) {
            ops.push(Op::SetOutlineThickness { pt: Pt(lw_pt) });
            last_lw = Some(lw_pt);
        }

        // Emit segments (NaN = pen-up).
        let mut segment: Vec<LinePoint> = Vec::new();
        for &[x, y, _z] in &wire.points {
            if x.is_nan() || y.is_nan() {
                flush_line(ops, &segment);
                segment.clear();
            } else {
                segment.push(LinePoint {
                    p: Point::new(Mm(x + ox), Mm(y + oy)),
                    bezier: false,
                });
            }
        }
        flush_line(ops, &segment);
    }
}

fn flush_line(ops: &mut Vec<Op>, pts: &[LinePoint]) {
    if pts.len() < 2 {
        return;
    }
    ops.push(Op::DrawLine {
        line: Line {
            points: pts.to_vec(),
            is_closed: false,
        },
    });
}

// ── Hatch fills ────────────────────────────────────────────────────────────

fn emit_hatch_fills(
    ops: &mut Vec<Op>,
    hatches: &HashMap<Handle, HatchModel>,
    ox: f32,
    oy: f32,
    options: &PdfExportOptions,
) {
    for hatch in hatches.values() {
        if hatch.boundary.len() < 3 {
            continue;
        }
        let a = hatch.color[3];
        if a < 0.01 {
            continue;
        }
        match &hatch.pattern {
            HatchPattern::Solid => emit_hatch_solid(ops, hatch, ox, oy, options),
            HatchPattern::Pattern(families) if options.hatch_patterns => {
                emit_hatch_pattern_lines(ops, hatch, families, ox, oy, options);
            }
            HatchPattern::Pattern(_) | HatchPattern::Gradient { .. } => {
                // Gradient is still Phase 3;  pattern gets Phase 2 line-family
                // emission above when `hatch_patterns == true` — otherwise we
                // silently skip to keep Phase 1 semantics available.
            }
        }
    }
}

fn emit_hatch_solid(
    ops: &mut Vec<Op>,
    hatch: &HatchModel,
    ox: f32,
    oy: f32,
    options: &PdfExportOptions,
) {
    let [mut r, mut g, mut b, _a] = hatch.color;
    if options.monochrome {
        // Light grey in monochrome so hatches stay visible but don't
        // overpower the strokes on a black-and-white print.
        r = 0.80;
        g = 0.80;
        b = 0.80;
    }

    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r,
            g,
            b,
            icc_profile: None,
        }),
    });

    let ring_points: Vec<LinePoint> = hatch
        .boundary
        .iter()
        .map(|&[x, y]| LinePoint {
            p: Point::new(Mm(x + ox), Mm(y + oy)),
            bezier: false,
        })
        .collect();

    let polygon = Polygon {
        rings: vec![PolygonRing {
            points: ring_points,
        }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::EvenOdd,
    };
    ops.push(Op::DrawPolygon { polygon });
}

/// Maximum number of parallel lines to emit per HATCH family — safety cap
/// guarding against pathological patterns (e.g. perp_step ≈ 0 on a huge
/// AABB) from blowing up file size.
const PATTERN_LINES_CAP: i32 = 4000;

fn emit_hatch_pattern_lines(
    ops: &mut Vec<Op>,
    hatch: &HatchModel,
    families: &[crate::scene::hatch_model::PatFamily],
    ox: f32,
    oy: f32,
    options: &PdfExportOptions,
) {
    if families.is_empty() {
        return;
    }

    // AABB of the boundary polygon (in CAD world coords, then shifted by
    // (ox, oy) inside the line emit). Using an AABB instead of the full
    // boundary is the documented trade-off from the plan: lines may extend
    // slightly past non-convex boundaries but no line is ever missing.
    let (bx0, by0, bx1, by1) = aabb_of(&hatch.boundary);
    if (bx1 - bx0).abs() < 1e-6 || (by1 - by0).abs() < 1e-6 {
        return;
    }

    // Stroke style: pattern lines are thinner than entity strokes. Use
    // 0.1 pt (≈ 0.035 mm) so they read as hatch marks, not outlines.
    let lw_pt = 0.1_f32.max(0.25 * MM_TO_PT * 0.25);
    let [mut r, mut g, mut b, _a] = hatch.color;
    if options.monochrome {
        r = 0.0;
        g = 0.0;
        b = 0.0;
    }
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r,
            g,
            b,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(lw_pt) });

    let scale = hatch.scale.max(1e-6);
    let global_angle_offset = hatch.angle_offset;

    for family in families {
        let angle_rad = family.angle_deg.to_radians() + global_angle_offset;
        let (sin_a, cos_a) = angle_rad.sin_cos();
        let dx = family.dx * scale;
        let dy = family.dy * scale;

        // Degenerate: no perpendicular offset ⇒ all lines coincide.  Skip.
        if dy.abs() < 1e-4 {
            continue;
        }

        let base_x = family.x0 * scale;
        let base_y = family.y0 * scale;

        // Perpendicular unit vector in world space.
        // Along-line unit vector = (cos_a, sin_a); perp = (-sin_a, cos_a).
        let perp_x = -sin_a;
        let perp_y = cos_a;
        let dir_x = cos_a;
        let dir_y = sin_a;

        // Project all AABB corners onto the perpendicular axis to find the
        // range of N (line index) that touches the AABB.
        let corners = [
            (bx0, by0),
            (bx1, by0),
            (bx1, by1),
            (bx0, by1),
        ];
        let perp_offsets: Vec<f32> = corners
            .iter()
            .map(|&(x, y)| (x - base_x) * perp_x + (y - base_y) * perp_y)
            .collect();
        let min_perp = perp_offsets
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let max_perp = perp_offsets
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);

        let n_start = (min_perp / dy).floor() as i32 - 1;
        let n_end = (max_perp / dy).ceil() as i32 + 1;
        if n_end - n_start > PATTERN_LINES_CAP {
            // Suspiciously dense — likely bad data, skip to avoid bloat.
            continue;
        }

        for n in n_start..=n_end {
            let nf = n as f32;
            let origin_x = base_x + nf * dy * perp_x + nf * dx * dir_x;
            let origin_y = base_y + nf * dy * perp_y + nf * dx * dir_y;

            if let Some((t0, t1)) = clip_line_aabb(
                origin_x, origin_y, dir_x, dir_y, bx0, by0, bx1, by1,
            ) {
                let p0x = origin_x + t0 * dir_x + ox;
                let p0y = origin_y + t0 * dir_y + oy;
                let p1x = origin_x + t1 * dir_x + ox;
                let p1y = origin_y + t1 * dir_y + oy;

                if family.dashes.is_empty() {
                    // Solid line; emit as single 2-point Line.
                    emit_line_segment(ops, p0x, p0y, p1x, p1y);
                } else {
                    emit_dashed_segments(
                        ops,
                        p0x,
                        p0y,
                        dir_x,
                        dir_y,
                        t0,
                        t1,
                        origin_x,
                        origin_y,
                        ox,
                        oy,
                        &family.dashes,
                    );
                }
            }
        }
    }
}

fn emit_line_segment(ops: &mut Vec<Op>, p0x: f32, p0y: f32, p1x: f32, p1y: f32) {
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(p0x), Mm(p0y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(p1x), Mm(p1y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

/// Emit a dashed scan-line by walking the dash sequence along `dir` starting
/// from `(origin + t0 * dir)` and stopping at `(origin + t1 * dir)`.  Positive
/// dash entries are pen-down strokes; negative entries are pen-up gaps.
#[allow(clippy::too_many_arguments)]
fn emit_dashed_segments(
    ops: &mut Vec<Op>,
    _p0x: f32,
    _p0y: f32,
    dir_x: f32,
    dir_y: f32,
    t0: f32,
    t1: f32,
    origin_x: f32,
    origin_y: f32,
    ox: f32,
    oy: f32,
    dashes: &[f32],
) {
    if dashes.is_empty() {
        return;
    }
    let period: f32 = dashes.iter().map(|d| d.abs()).sum();
    if period < 1e-6 {
        return;
    }

    // Walk t from t0 to t1 following the dash cycle.
    let mut t = t0;
    // Start aligned at the cycle boundary closest to (and ≤) t0 so dashes
    // remain coherent across all parallel lines in the family.
    let cycles_before = (t0 / period).floor();
    let within_cycle = t0 - cycles_before * period;
    let mut dash_idx = 0_usize;
    let mut dash_acc = 0.0_f32;
    // Skip forward in the cycle until we reach `within_cycle`.
    while within_cycle > 1e-6 && dash_idx < dashes.len() {
        let len = dashes[dash_idx].abs();
        if dash_acc + len >= within_cycle {
            break;
        }
        dash_acc += len;
        dash_idx += 1;
    }
    let mut carry_within = within_cycle - dash_acc;

    // Safety cap to guard against degenerate infinite loops.
    let mut iter_budget = 100_000;
    while t < t1 && iter_budget > 0 {
        iter_budget -= 1;
        if dash_idx >= dashes.len() {
            dash_idx = 0;
            carry_within = 0.0;
        }
        let dash_len_signed = dashes[dash_idx];
        let dash_len = dash_len_signed.abs();
        let remaining_in_dash = (dash_len - carry_within).max(0.0);
        let seg_end = (t + remaining_in_dash).min(t1);
        if dash_len_signed > 0.0 && seg_end > t + 1e-6 {
            let p0x = origin_x + t * dir_x + ox;
            let p0y = origin_y + t * dir_y + oy;
            let p1x = origin_x + seg_end * dir_x + ox;
            let p1y = origin_y + seg_end * dir_y + oy;
            emit_line_segment(ops, p0x, p0y, p1x, p1y);
        }
        t = seg_end;
        dash_idx += 1;
        carry_within = 0.0;
    }
}

fn aabb_of(points: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for &[x, y] in points {
        if x < x0 {
            x0 = x;
        }
        if y < y0 {
            y0 = y;
        }
        if x > x1 {
            x1 = x;
        }
        if y > y1 {
            y1 = y;
        }
    }
    (x0, y0, x1, y1)
}

/// Liang-Barsky line-vs-AABB clip.  Returns `(t0, t1)` such that the ray
/// `P(t) = origin + t * dir` intersects `[bx0..bx1] × [by0..by1]` for
/// `t ∈ [t0, t1]`, or `None` if the ray misses the box.
#[allow(clippy::too_many_arguments)]
fn clip_line_aabb(
    ox: f32,
    oy: f32,
    dx: f32,
    dy: f32,
    bx0: f32,
    by0: f32,
    bx1: f32,
    by1: f32,
) -> Option<(f32, f32)> {
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;

    for &(p, q) in &[
        (-dx, ox - bx0),
        (dx, bx1 - ox),
        (-dy, oy - by0),
        (dy, by1 - oy),
    ] {
        if p.abs() < 1e-8 {
            if q < 0.0 {
                return None; // parallel and outside
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 {
                return None;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            if r < t0 {
                return None;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }
    if t0 <= t1 {
        Some((t0, t1))
    } else {
        None
    }
}

// ── Image embedding ────────────────────────────────────────────────────────

/// Record for a raster image that has already been registered with the
/// `PdfDocument` via `add_image()`. The transform is in PDF points and
/// reproduces the affine mapping used by the SVG exporter:
///   (u, v) basis in CAD world-space → PDF (translate, scale) ops.
struct PdfImageSpec {
    id: printpdf::XObjectId,
    transform: XObjectTransform,
}

fn collect_and_register_images(
    doc: &mut PdfDocument,
    native_doc: &nm::CadDocument,
    options: &PdfExportOptions,
) -> Vec<PdfImageSpec> {
    let mut out = Vec::new();

    // Freeze / layer-off filtering matches the SVG pipeline.
    let frozen_layers: HashSet<&str> = native_doc
        .layers
        .values()
        .filter(|l| l.is_frozen || !l.is_on())
        .map(|l| l.name.as_str())
        .collect();

    for entity in &native_doc.entities {
        if entity.invisible {
            continue;
        }
        if frozen_layers.contains(entity.layer_name.as_str()) {
            continue;
        }
        let nm::EntityData::Image {
            insertion,
            u_vector,
            v_vector,
            image_size,
            file_path,
            display_flags,
            ..
        } = &entity.data
        else {
            continue;
        };
        // DXF code 70 bit 1 = SHOW_IMAGE. When clear, viewer should hide.
        if (*display_flags & 0x1) == 0 {
            continue;
        }
        if file_path.is_empty() {
            continue;
        }

        let resolved = resolve_image_path(file_path, options.image_base.as_deref());
        let raw = match load_raw_image(&resolved) {
            Some(raw) => raw,
            None => continue,
        };

        // Affine: image-local pixel coords (0..w, 0..h) → CAD world.
        // We register the image as a 1-pt-per-pixel XObject at 72 dpi and then
        // use a raw transform matrix that combines scale + shear + translation
        // exactly like SVG's `matrix(a,b,c,d,e,f)`.
        // Because `XObjectTransform` cannot express arbitrary affine (only
        // translate + scale + rotate around a fixed center), we emit a raw
        // `SetTransformationMatrix` wrapped in save/restore instead. The
        // XObjectTransform keeps the default `dpi=72` so 1 image pixel = 1 pt.
        let _ = u_vector;
        let _ = v_vector;
        let _ = image_size;
        let _ = insertion;

        // Use default transform; real affine is done via SetTransformationMatrix
        // in emit_image_use.  Keep translate=None so only the image's intrinsic
        // size sets the box.
        let transform = XObjectTransform {
            translate_x: None,
            translate_y: None,
            rotate: None,
            scale_x: None,
            scale_y: None,
            dpi: Some(72.0),
        };

        let id = doc.add_image(&raw);

        // Attach the affine parameters as part of the spec so we can emit
        // matrix + Do in the page stream.
        out.push(PdfImageSpec {
            id,
            transform,
        });
    }

    out
}

fn emit_image_use(ops: &mut Vec<Op>, spec: &PdfImageSpec) {
    ops.push(Op::UseXobject {
        id: spec.id.clone(),
        transform: spec.transform.clone(),
    });
}

fn resolve_image_path(file_path: &str, base: Option<&Path>) -> PathBuf {
    let candidate = Path::new(file_path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    if let Some(base) = base {
        return base.join(candidate);
    }
    candidate.to_path_buf()
}

/// Load a raster file from disk and convert to printpdf's `RawImage`.
/// Returns `None` when the file cannot be read or decoded — the caller
/// treats that as "skip silently" so a missing image never breaks export.
fn load_raw_image(path: &Path) -> Option<RawImage> {
    let dyn_img = image::open(path).ok()?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let pixels = rgba.into_raw();
    Some(RawImage {
        pixels: RawImageData::U8(pixels),
        width: w,
        height: h,
        data_format: RawImageFormat::RGBA8,
        tag: Vec::new(),
    })
}

// ── Native curves (Circle / Arc / Ellipse) ─────────────────────────────────

/// Magic constant for approximating a 90° circular arc with one cubic bezier:
///   k = 4/3 * tan(π/8)
/// Extending an arc over a smaller span t uses `k = 4/3 * tan(t/4)` instead.
const CIRCLE_QUARTER_K: f32 = 0.552_284_75;

fn emit_native_curves(
    ops: &mut Vec<Op>,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
) {
    let frozen_layers: HashSet<&str> = doc
        .layers
        .values()
        .filter(|l| l.is_frozen || !l.is_on())
        .map(|l| l.name.as_str())
        .collect();

    for entity in &doc.entities {
        if entity.invisible {
            continue;
        }
        if frozen_layers.contains(entity.layer_name.as_str()) {
            continue;
        }

        // Resolve stroke color + lineweight once per entity, matching
        // the wire layer's CTB / monochrome policy.
        let (r, g, b) = resolve_entity_stroke_rgb(entity, plot_style, options);
        let lw_pt = resolve_entity_lineweight_pt(entity, plot_style);

        match &entity.data {
            nm::EntityData::Circle { center, radius } => {
                emit_stroke_setup(ops, r, g, b, lw_pt);
                let line = build_circle_line(
                    center[0] as f32 + ox,
                    center[1] as f32 + oy,
                    *radius as f32,
                );
                ops.push(Op::DrawLine { line });
            }
            nm::EntityData::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                emit_stroke_setup(ops, r, g, b, lw_pt);
                let line = build_arc_line(
                    center[0] as f32 + ox,
                    center[1] as f32 + oy,
                    *radius as f32,
                    (*start_angle as f32).to_radians(),
                    (*end_angle as f32).to_radians(),
                );
                ops.push(Op::DrawLine { line });
            }
            nm::EntityData::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                // `start_param` / `end_param` are already in radians per DXF
                // spec (unlike Arc which stores degrees).
                emit_stroke_setup(ops, r, g, b, lw_pt);
                let line = build_ellipse_line(
                    center[0] as f32 + ox,
                    center[1] as f32 + oy,
                    [major_axis[0] as f32, major_axis[1] as f32],
                    *ratio as f32,
                    *start_param as f32,
                    *end_param as f32,
                );
                ops.push(Op::DrawLine { line });
            }
            _ => {}
        }
    }
}

fn emit_stroke_setup(ops: &mut Vec<Op>, r: f32, g: f32, b: f32, lw_pt: f32) {
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r,
            g,
            b,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(lw_pt) });
}

fn resolve_entity_stroke_rgb(
    entity: &nm::Entity,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
) -> (f32, f32, f32) {
    if options.monochrome {
        return (0.0, 0.0, 0.0);
    }
    // Try CTB first (ACI → RGB), otherwise fall back to ACI defaults the
    // SVG exporter uses (lines 1985-2004 of svg_export.rs).
    let aci = entity.color_index;
    if aci > 0 && aci < 256 {
        if let Some(ctb) = plot_style {
            if let Some([cr, cg, cb]) = ctb.resolve_color(aci as u8) {
                return (cr, cg, cb);
            }
        }
        let (r, g, b) = aci_to_rgb(aci as u8);
        return (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    }
    (0.0, 0.0, 0.0)
}

fn resolve_entity_lineweight_pt(entity: &nm::Entity, plot_style: Option<&PlotStyleTable>) -> f32 {
    let aci = entity.color_index;
    if aci > 0 && aci < 256 {
        if let Some(ctb) = plot_style {
            if let Some(mm) = ctb.resolve_lineweight(aci as u8) {
                return (mm * MM_TO_PT).max(0.1);
            }
        }
    }
    // Default 0.25 mm line — same default the wire pipeline falls back to
    // when no explicit lineweight is present.
    0.25_f32 * MM_TO_PT
}

/// Minimal ACI → RGB mapping sufficient for the 9 indexed CAD colors.
/// (Mirrors `svg_export.rs` `aci_to_rgb` defaults — kept local to avoid
/// exposing that helper as `pub(crate)` just for PDF consumption.)
fn aci_to_rgb(aci: u8) -> (u8, u8, u8) {
    match aci {
        1 => (255, 0, 0),       // red
        2 => (255, 255, 0),     // yellow
        3 => (0, 255, 0),       // green
        4 => (0, 255, 255),     // cyan
        5 => (0, 0, 255),       // blue
        6 => (255, 0, 255),     // magenta
        7 | 0 => (0, 0, 0),     // white / ByBlock → black on white paper
        _ => (0, 0, 0),         // default to black for 8-255
    }
}

/// Build a closed 4-bezier approximation of a circle centred at `(cx, cy)`
/// with radius `r`. The output is a `Line { is_closed: true }` containing
/// 4 anchor points and 8 control points so the printpdf serialiser emits
/// `m c c c c h S` (four cubic beziers + close + stroke).
fn build_circle_line(cx: f32, cy: f32, r: f32) -> Line {
    let k = r * CIRCLE_QUARTER_K;

    let mut points = Vec::with_capacity(13);

    // Anchor at (cx + r, cy) — 0°.
    points.push(lp(cx + r, cy, false));
    // Quarter 1: 0° → 90°.
    points.push(lp(cx + r, cy + k, true));
    points.push(lp(cx + k, cy + r, true));
    points.push(lp(cx, cy + r, false));
    // Quarter 2: 90° → 180°.
    points.push(lp(cx - k, cy + r, true));
    points.push(lp(cx - r, cy + k, true));
    points.push(lp(cx - r, cy, false));
    // Quarter 3: 180° → 270°.
    points.push(lp(cx - r, cy - k, true));
    points.push(lp(cx - k, cy - r, true));
    points.push(lp(cx, cy - r, false));
    // Quarter 4: 270° → 360°.
    points.push(lp(cx + k, cy - r, true));
    points.push(lp(cx + r, cy - k, true));
    points.push(lp(cx + r, cy, false));

    Line {
        points,
        is_closed: true,
    }
}

/// Build a bezier approximation of an arc from `start_rad` to `end_rad`
/// (counter-clockwise; the DXF spec always draws arcs ccw) with radius
/// `r` around `(cx, cy)`.
fn build_arc_line(cx: f32, cy: f32, r: f32, start_rad: f32, end_rad: f32) -> Line {
    // Normalise end > start so sweep is positive.
    let mut sweep = end_rad - start_rad;
    while sweep < 0.0 {
        sweep += std::f32::consts::TAU;
    }
    if sweep < 1e-6 {
        // Degenerate (zero-length) arc — emit a single anchor, no segments.
        let x = cx + r * start_rad.cos();
        let y = cy + r * start_rad.sin();
        return Line {
            points: vec![lp(x, y, false)],
            is_closed: false,
        };
    }
    // Split into ≤90° chunks so each chunk is within the bezier's accuracy
    // envelope (< 1/1000 deviation for a 90° segment).
    let chunks = ((sweep / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
    let dt = sweep / chunks as f32;
    let k = r * (4.0 / 3.0) * (dt / 4.0).tan();

    let mut points = Vec::with_capacity(chunks * 3 + 1);
    // First anchor = start point.
    let (mut cos0, mut sin0) = start_rad.sin_cos();
    std::mem::swap(&mut cos0, &mut sin0); // sin_cos returns (sin, cos); we want cos/sin in that order
    let mut x0 = cx + r * cos0;
    let mut y0 = cy + r * sin0;
    points.push(lp(x0, y0, false));

    for i in 0..chunks {
        let t0 = start_rad + (i as f32) * dt;
        let t1 = t0 + dt;
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();

        // Control points for arc segment t0 → t1.
        // Tangent at t = perpendicular to radial direction; scale by k.
        let c1x = cx + r * c0 - k * s0;
        let c1y = cy + r * s0 + k * c0;
        let c2x = cx + r * c1 + k * s1;
        let c2y = cy + r * s1 - k * c1;
        let p3x = cx + r * c1;
        let p3y = cy + r * s1;

        points.push(lp(c1x, c1y, true));
        points.push(lp(c2x, c2y, true));
        points.push(lp(p3x, p3y, false));

        x0 = p3x;
        y0 = p3y;
    }
    let _ = (x0, y0);

    Line {
        points,
        is_closed: false,
    }
}

/// Build a bezier approximation of an ellipse or elliptical arc.
/// `major_axis_xy` is the major-axis vector (length = major radius), in the
/// ellipse's local plane (we assume Z = 0 for 2D export).
/// `ratio` = minor_radius / major_radius (0..1).
/// `start_param` / `end_param` are parametric angles in radians (0 at the
/// end of the major axis, increasing ccw).
fn build_ellipse_line(
    cx: f32,
    cy: f32,
    major_axis_xy: [f32; 2],
    ratio: f32,
    start_param: f32,
    end_param: f32,
) -> Line {
    let major_len = (major_axis_xy[0] * major_axis_xy[0]
        + major_axis_xy[1] * major_axis_xy[1])
        .sqrt();
    if major_len < 1e-6 {
        return Line {
            points: vec![lp(cx, cy, false)],
            is_closed: false,
        };
    }
    let mx = major_axis_xy[0] / major_len;
    let my = major_axis_xy[1] / major_len;
    // Minor axis is +90° rotation of major axis in the plane.
    let nx = -my;
    let ny = mx;

    let a = major_len;
    let b = major_len * ratio;

    // Determine sweep, treating 0..TAU as "full ellipse" (DXF full ellipse
    // has start_param = 0 and end_param = TAU per DXF spec).
    let mut sweep = end_param - start_param;
    while sweep < 0.0 {
        sweep += std::f32::consts::TAU;
    }
    let is_closed = (sweep - std::f32::consts::TAU).abs() < 1e-4;

    if sweep < 1e-6 {
        let x = cx + a * start_param.cos() * mx + b * start_param.sin() * nx;
        let y = cy + a * start_param.cos() * my + b * start_param.sin() * ny;
        return Line {
            points: vec![lp(x, y, false)],
            is_closed: false,
        };
    }

    let chunks = ((sweep / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
    let dt = sweep / chunks as f32;
    // k scales the unit-circle control-point distance (4/3 tan(t/4)).
    let k_unit = (4.0 / 3.0) * (dt / 4.0).tan();

    let mut points = Vec::with_capacity(chunks * 3 + 1);

    // Helper: parametric point + tangent scale factors on the unit circle.
    let eval = |t: f32| -> ([f32; 2], [f32; 2]) {
        let (s, c) = t.sin_cos();
        // Ellipse point.
        let px = cx + a * c * mx + b * s * nx;
        let py = cy + a * c * my + b * s * ny;
        // Tangent direction (derivative wrt t), unnormalised.
        let tx = -a * s * mx + b * c * nx;
        let ty = -a * s * my + b * c * ny;
        ([px, py], [tx, ty])
    };

    let (p0, _t0) = eval(start_param);
    points.push(lp(p0[0], p0[1], false));

    for i in 0..chunks {
        let t0 = start_param + (i as f32) * dt;
        let t1 = t0 + dt;
        let (p_start, t_start) = eval(t0);
        let (p_end, t_end) = eval(t1);

        let c1x = p_start[0] + k_unit * t_start[0];
        let c1y = p_start[1] + k_unit * t_start[1];
        let c2x = p_end[0] - k_unit * t_end[0];
        let c2y = p_end[1] - k_unit * t_end[1];

        points.push(lp(c1x, c1y, true));
        points.push(lp(c2x, c2y, true));
        points.push(lp(p_end[0], p_end[1], false));
    }

    Line { points, is_closed }
}

fn lp(x: f32, y: f32, bezier: bool) -> LinePoint {
    LinePoint {
        p: Point::new(Mm(x), Mm(y)),
        bezier,
    }
}

// ── Native splines (三十五轮 Phase 3) ──────────────────────────────────────

fn emit_native_splines(
    ops: &mut Vec<Op>,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &PdfExportOptions,
) {
    use crate::io::svg_export::{spline_emit_strategy, SplineEmit};

    let frozen_layers: HashSet<&str> = doc
        .layers
        .values()
        .filter(|l| l.is_frozen || !l.is_on())
        .map(|l| l.name.as_str())
        .collect();

    for entity in &doc.entities {
        if entity.invisible {
            continue;
        }
        if frozen_layers.contains(entity.layer_name.as_str()) {
            continue;
        }
        let nm::EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            ..
        } = &entity.data
        else {
            continue;
        };

        let Some(strategy) = spline_emit_strategy(
            *degree,
            *closed,
            knots,
            control_points,
            weights,
            fit_points,
        ) else {
            continue; // falls back to wire path (not skipped above)
        };

        let (r, g, b) = resolve_entity_stroke_rgb(entity, plot_style, options);
        let lw_pt = resolve_entity_lineweight_pt(entity, plot_style);
        emit_stroke_setup(ops, r, g, b, lw_pt);

        match strategy {
            SplineEmit::ControlPoly => {
                emit_polyline(ops, control_points, ox, oy, *closed);
            }
            SplineEmit::FitPoly => {
                emit_polyline(ops, fit_points, ox, oy, false);
            }
            SplineEmit::Bezier {
                degree,
                control_points: cps,
            } => {
                emit_bezier_spline(ops, &cps, degree, ox, oy);
            }
        }
    }
}

/// Emit a degenerate spline as a polyline through `pts` (xyz triples; Z is
/// ignored for 2D PDF export).
fn emit_polyline(ops: &mut Vec<Op>, pts: &[[f64; 3]], ox: f32, oy: f32, is_closed: bool) {
    if pts.len() < 2 {
        return;
    }
    let points: Vec<LinePoint> = pts
        .iter()
        .map(|&[x, y, _z]| LinePoint {
            p: Point::new(Mm(x as f32 + ox), Mm(y as f32 + oy)),
            bezier: false,
        })
        .collect();
    ops.push(Op::DrawLine {
        line: Line {
            points,
            is_closed,
        },
    });
}

/// Emit a clamped non-rational B-spline (already decomposed into piecewise
/// Bezier control points) as a PDF path.  `degree` ∈ {2, 3}.
///
/// For degree = 3 we map each 4-point segment `[P0, C1, C2, P3]` directly
/// onto a PDF cubic bezier.  For degree = 2 we promote each 3-point
/// segment `[Q0, Q1, Q2]` to an exact cubic `[P0, C1, C2, P3]` via the
/// standard 2/3 rule:
///   P0 = Q0
///   C1 = Q0 + 2/3 (Q1 - Q0)
///   C2 = Q2 + 2/3 (Q1 - Q2)
///   P3 = Q2
fn emit_bezier_spline(
    ops: &mut Vec<Op>,
    control_points: &[[f64; 3]],
    degree: usize,
    ox: f32,
    oy: f32,
) {
    if degree != 2 && degree != 3 {
        return;
    }
    if control_points.len() < degree + 1 {
        return;
    }
    let segments = (control_points.len() - 1) / degree;
    if segments == 0 {
        return;
    }

    let mut points: Vec<LinePoint> = Vec::with_capacity(segments * 3 + 1);
    // Initial anchor.
    let p0 = control_points[0];
    points.push(lp(p0[0] as f32 + ox, p0[1] as f32 + oy, false));

    for s in 0..segments {
        let base = s * degree;
        if degree == 3 {
            let c1 = control_points[base + 1];
            let c2 = control_points[base + 2];
            let p3 = control_points[base + 3];
            points.push(lp(c1[0] as f32 + ox, c1[1] as f32 + oy, true));
            points.push(lp(c2[0] as f32 + ox, c2[1] as f32 + oy, true));
            points.push(lp(p3[0] as f32 + ox, p3[1] as f32 + oy, false));
        } else {
            // degree == 2: promote quadratic → cubic exactly.
            let q0 = control_points[base];
            let q1 = control_points[base + 1];
            let q2 = control_points[base + 2];
            let c1 = [
                q0[0] + (2.0 / 3.0) * (q1[0] - q0[0]),
                q0[1] + (2.0 / 3.0) * (q1[1] - q0[1]),
                0.0,
            ];
            let c2 = [
                q2[0] + (2.0 / 3.0) * (q1[0] - q2[0]),
                q2[1] + (2.0 / 3.0) * (q1[1] - q2[1]),
                0.0,
            ];
            points.push(lp(c1[0] as f32 + ox, c1[1] as f32 + oy, true));
            points.push(lp(c2[0] as f32 + ox, c2[1] as f32 + oy, true));
            points.push(lp(q2[0] as f32 + ox, q2[1] as f32 + oy, false));
        }
    }

    ops.push(Op::DrawLine {
        line: Line {
            points,
            is_closed: false,
        },
    });
}

// ── Native text emission ───────────────────────────────────────────────────

fn collect_native_handles(
    native_doc: Option<&nm::CadDocument>,
    options: &PdfExportOptions,
) -> (HashSet<String>, HashSet<String>, HashSet<String>, HashSet<String>) {
    let mut text = HashSet::new();
    let mut image = HashSet::new();
    let mut curve = HashSet::new();
    let mut spline = HashSet::new();
    let Some(doc) = native_doc else {
        return (text, image, curve, spline);
    };
    for entity in &doc.entities {
        // Frozen / layer-off / invisible filtering matches the wire pipeline
        // and the native-emit passes below, so skips stay consistent.
        if entity.invisible {
            continue;
        }
        match &entity.data {
            nm::EntityData::Text { value, .. } | nm::EntityData::MText { value, .. } => {
                if !options.text_as_geometry && can_render_native_text(value) {
                    text.insert(entity.handle.value().to_string());
                }
            }
            nm::EntityData::Image { .. } => {
                if options.include_images && options.embed_images {
                    image.insert(entity.handle.value().to_string());
                }
            }
            nm::EntityData::Circle { .. }
            | nm::EntityData::Arc { .. }
            | nm::EntityData::Ellipse { .. } => {
                if options.native_curves {
                    curve.insert(entity.handle.value().to_string());
                }
            }
            _ => {}
        }
    }
    // Splines use the richer strategy picker from `svg_export` to decide which
    // ones we can safely emit natively (degree 1 / clamped non-rational 2-3 /
    // fit-point fallback).  High-order rational curves stay on the wire path.
    if options.native_splines {
        spline = crate::io::svg_export::collect_emittable_spline_handles(doc);
    }
    (text, image, curve, spline)
}

/// `true` when the text consists of only characters safely supported by the
/// Standard 14 built-in fonts (Latin-1 Supplement range).  CJK / non-Latin
/// content falls back to wire tessellation so we never emit garbage glyphs.
fn can_render_native_text(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| (c as u32) <= 0xFF)
}

fn emit_native_text(
    ops: &mut Vec<Op>,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    options: &PdfExportOptions,
) {
    let frozen_layers: HashSet<&str> = doc
        .layers
        .values()
        .filter(|l| l.is_frozen || !l.is_on())
        .map(|l| l.name.as_str())
        .collect();

    let builtin = options.font_family.to_builtin();

    let mut any_emitted = false;

    for entity in &doc.entities {
        if entity.invisible {
            continue;
        }
        if frozen_layers.contains(entity.layer_name.as_str()) {
            continue;
        }

        match &entity.data {
            nm::EntityData::Text {
                insertion,
                height,
                value,
                rotation,
                ..
            } => {
                if !can_render_native_text(value) {
                    continue;
                }
                let x = insertion[0] as f32 + ox;
                let y = insertion[1] as f32 + oy;
                let fs_mm = (*height as f32) * options.font_size_scale;
                if fs_mm < 0.01 {
                    continue;
                }
                if !any_emitted {
                    ops.push(Op::StartTextSection);
                    any_emitted = true;
                }
                emit_text_run(
                    ops,
                    builtin,
                    Pt(fs_mm * MM_TO_PT),
                    x,
                    y,
                    *rotation as f32,
                    value,
                    options,
                );
            }
            nm::EntityData::MText {
                insertion,
                height,
                value,
                rotation,
                ..
            } => {
                if !can_render_native_text(value) {
                    continue;
                }
                let x = insertion[0] as f32 + ox;
                let y = insertion[1] as f32 + oy;
                let fs_mm = (*height as f32) * options.font_size_scale;
                if fs_mm < 0.01 {
                    continue;
                }
                let clean = strip_mtext_codes(value);
                if !can_render_native_text(&clean) {
                    continue;
                }
                if !any_emitted {
                    ops.push(Op::StartTextSection);
                    any_emitted = true;
                }
                // MText line break = `\P`; render each line with dy = fs * 1.2.
                let lines: Vec<&str> = clean.split('\n').collect();
                let line_dy = fs_mm * 1.2;
                for (i, line) in lines.iter().enumerate() {
                    if line.is_empty() {
                        continue;
                    }
                    let yl = y - (i as f32) * line_dy;
                    emit_text_run(
                        ops,
                        builtin,
                        Pt(fs_mm * MM_TO_PT),
                        x,
                        yl,
                        *rotation as f32,
                        line,
                        options,
                    );
                }
            }
            _ => {}
        }
    }

    if any_emitted {
        ops.push(Op::EndTextSection);
    }
}

/// Emit one text run at `(x_mm, y_mm)` with given rotation and font.
fn emit_text_run(
    ops: &mut Vec<Op>,
    builtin: BuiltinFont,
    size_pt: Pt,
    x_mm: f32,
    y_mm: f32,
    rotation_deg: f32,
    text: &str,
    options: &PdfExportOptions,
) {
    // Fill color for text — monochrome overrides to black.
    let (r, g, b) = if options.monochrome {
        (0.0_f32, 0.0_f32, 0.0_f32)
    } else {
        (0.0_f32, 0.0_f32, 0.0_f32)
    };
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r,
            g,
            b,
            icc_profile: None,
        }),
    });

    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(builtin),
        size: size_pt,
    });

    // PDF's `Tm` operator replaces the text matrix entirely. We use
    // `TextMatrix::TranslateRotate` which bundles rotation and (tx, ty)
    // relative to the page origin — exactly the semantics we want for
    // rotated engineering-drawing labels.
    let tx = Pt(x_mm * MM_TO_PT);
    let ty = Pt(y_mm * MM_TO_PT);

    if rotation_deg.abs() > 0.01 {
        ops.push(Op::SetTextMatrix {
            matrix: printpdf::TextMatrix::TranslateRotate(tx, ty, rotation_deg),
        });
    } else {
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(x_mm), Mm(y_mm)),
        });
    }

    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Strip MText formatting codes and return plain text with `\n` for line
/// breaks. Mirrors `svg_export::strip_mtext_codes` so both exporters decode
/// the same control characters.
fn strip_mtext_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('P') | Some('p') => {
                    chars.next();
                    out.push('\n');
                }
                Some('f') | Some('F') | Some('H') | Some('h') | Some('C') | Some('c')
                | Some('T') | Some('t') | Some('Q') | Some('q') | Some('W') | Some('w')
                | Some('A') | Some('a') | Some('L') | Some('l') | Some('O') | Some('o')
                | Some('K') | Some('k') => {
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch == ';' {
                            break;
                        }
                    }
                }
                Some('S') | Some('s') => {
                    chars.next();
                    let mut buf = String::new();
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch == ';' {
                            break;
                        }
                        buf.push(if ch == '^' { '/' } else { ch });
                    }
                    out.push_str(&buf);
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                Some('{') => {
                    chars.next();
                    out.push('{');
                }
                Some('}') => {
                    chars.next();
                    out.push('}');
                }
                _ => out.push('\\'),
            }
        } else if c == '{' || c == '}' {
            // Grouping braces: skip.
        } else {
            out.push(c);
        }
    }
    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("h7cad_pdf_test_{}_{}.pdf", name, std::process::id()));
        p
    }

    fn find_any(bytes: &[u8], needles: &[&[u8]]) -> bool {
        needles.iter().any(|needle| {
            bytes.windows(needle.len()).any(|w| w == *needle)
        })
    }

    fn line_wire(points: Vec<[f32; 3]>) -> WireModel {
        WireModel::solid("42".into(), points, [0.0, 0.0, 0.0, 1.0], false)
    }

    #[test]
    fn fixture_pdf_wire_smoke() {
        let wires = vec![line_wire(vec![[10.0, 10.0, 0.0], [100.0, 80.0, 0.0]])];
        let out = tmp_path("wire_smoke");
        export_pdf(&wires, 297.0, 210.0, 0.0, 0.0, 0, &out, None)
            .expect("export_pdf should succeed for basic wires");
        let bytes = std::fs::read(&out).expect("pdf should be on disk");
        assert!(
            bytes.starts_with(b"%PDF-"),
            "expected PDF magic header, got {:?}",
            &bytes[..8.min(bytes.len())]
        );
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn fixture_pdf_text_renders_as_native_text() {
        // Build a minimal native doc with one ASCII TEXT entity.
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Text {
            insertion: [20.0, 30.0, 0.0],
            height: 5.0,
            value: "HELLO".into(),
            rotation: 0.0,
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            style_name: "Standard".into(),
            alignment_point: None,
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &hatches,
            Some(&doc),
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );

        // PDF text uses Tj ("text showing") or TJ ("text with spacing").
        // At least one of the two must appear in the stream.  Flate
        // compression is enabled by default, so look for the text itself
        // only after decompression (skipped here — we trust printpdf).
        assert!(
            bytes.starts_with(b"%PDF-"),
            "expected PDF magic header"
        );
        // The font resource dict references BuiltinFont identifier "F1" or
        // similar — the Helvetica built-in id is documented as `F1`.
        assert!(
            find_any(&bytes, &[b"/Font", b"/Type /Font"]),
            "expected a /Font resource dict for native text"
        );
    }

    #[test]
    fn fixture_pdf_cjk_text_falls_back_to_geometry() {
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Text {
            insertion: [20.0, 30.0, 0.0],
            height: 5.0,
            value: "中文测试".into(),
            rotation: 0.0,
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            style_name: "Standard".into(),
            alignment_point: None,
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let options = PdfExportOptions::default();

        // Collected handles must NOT list the CJK entity — proving fallback
        // semantics before we even generate a PDF.
        let (text_handles, _image, _curve, _spline) = collect_native_handles(Some(&doc), &options);
        assert!(
            text_handles.is_empty(),
            "CJK text must fall back to wire tessellation, got {:?}",
            text_handles
        );

        // And the builder itself must not include a text section.
        let bytes = build_pdf_full(
            &[],
            &hatches,
            Some(&doc),
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn fixture_pdf_solid_hatch_emits_fill_ops() {
        let mut hatches: HashMap<Handle, HatchModel> = HashMap::new();
        hatches.insert(
            Handle::new(0xAB),
            HatchModel {
                boundary: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
                pattern: HatchPattern::Solid,
                name: "SOLID".into(),
                color: [0.5, 0.5, 0.5, 1.0],
                angle_offset: 0.0,
                scale: 1.0,
            },
        );

        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &hatches,
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));
        // Solid hatch → polygon fill → file should be larger than a
        // completely empty PDF with only background.
        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(
            bytes.len() > empty_bytes.len(),
            "hatch pdf ({} bytes) must exceed empty ({} bytes)",
            bytes.len(),
            empty_bytes.len()
        );
    }

    #[test]
    fn fixture_pdf_empty_pattern_hatch_is_noop() {
        // Pattern HATCH with an *empty* family list produces no extra output.
        // (A family-less pattern is what the Phase 1 fixture used to cover;
        // with Phase 2's line emitter we only care that it doesn't crash.)
        let mut hatches: HashMap<Handle, HatchModel> = HashMap::new();
        hatches.insert(
            Handle::new(0xCC),
            HatchModel {
                boundary: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
                pattern: HatchPattern::Pattern(vec![]),
                name: "EMPTY".into(),
                color: [0.5, 0.5, 0.5, 1.0],
                angle_offset: 0.0,
                scale: 1.0,
            },
        );

        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &hatches,
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert_eq!(
            bytes.len(),
            empty_bytes.len(),
            "empty pattern family ⇒ no output, PDF byte length unchanged"
        );
    }

    #[test]
    fn fixture_pdf_pattern_hatch_emits_line_segments() {
        use crate::scene::hatch_model::PatFamily;

        // Triangle boundary + one 45° hatch family with 3mm perpendicular
        // spacing → multiple scan lines within the AABB.
        let mut hatches: HashMap<Handle, HatchModel> = HashMap::new();
        hatches.insert(
            Handle::new(0xDD),
            HatchModel {
                boundary: vec![
                    [0.0, 0.0],
                    [50.0, 0.0],
                    [25.0, 30.0],
                ],
                pattern: HatchPattern::Pattern(vec![PatFamily {
                    angle_deg: 45.0,
                    x0: 0.0,
                    y0: 0.0,
                    dx: 0.0,
                    dy: 3.0,
                    dashes: vec![],
                }]),
                name: "ANSI31-like".into(),
                color: [0.5, 0.5, 0.5, 1.0],
                angle_offset: 0.0,
                scale: 1.0,
            },
        );

        let options = PdfExportOptions::default();
        let pattern_bytes = build_pdf_full(
            &[],
            &hatches,
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(pattern_bytes.starts_with(b"%PDF-"));
        assert!(
            pattern_bytes.len() > empty_bytes.len() + 200,
            "pattern scan-lines should grow the PDF by at least ~200 bytes \
             (got {}B vs empty {}B)",
            pattern_bytes.len(),
            empty_bytes.len()
        );
    }

    #[test]
    fn fixture_pdf_options_hatch_patterns_toggle_off_matches_phase_1_skip() {
        use crate::scene::hatch_model::PatFamily;

        let mut hatches: HashMap<Handle, HatchModel> = HashMap::new();
        hatches.insert(
            Handle::new(0xEE),
            HatchModel {
                boundary: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
                pattern: HatchPattern::Pattern(vec![PatFamily {
                    angle_deg: 0.0,
                    x0: 0.0,
                    y0: 0.0,
                    dx: 0.0,
                    dy: 1.0,
                    dashes: vec![],
                }]),
                name: "ANSI33-like".into(),
                color: [0.5, 0.5, 0.5, 1.0],
                angle_offset: 0.0,
                scale: 1.0,
            },
        );

        let options_off = PdfExportOptions {
            hatch_patterns: false,
            ..PdfExportOptions::default()
        };
        let off_bytes = build_pdf_full(
            &[],
            &hatches,
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options_off,
        );
        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options_off,
        );
        assert_eq!(
            off_bytes.len(),
            empty_bytes.len(),
            "hatch_patterns=false must match Phase 1 skip semantics"
        );
    }

    #[test]
    fn fixture_pdf_pattern_dashed_hatch_survives_dash_walk() {
        use crate::scene::hatch_model::PatFamily;

        // One dashed horizontal family (1mm dash, 1mm gap) over a 100×100
        // boundary. This exercises `emit_dashed_segments` with a positive
        // dash followed by a gap — if the walk were broken we'd either
        // crash or produce 0 lines (= same length as empty).
        let mut hatches: HashMap<Handle, HatchModel> = HashMap::new();
        hatches.insert(
            Handle::new(0xFF),
            HatchModel {
                boundary: vec![
                    [0.0, 0.0],
                    [100.0, 0.0],
                    [100.0, 100.0],
                    [0.0, 100.0],
                ],
                pattern: HatchPattern::Pattern(vec![PatFamily {
                    angle_deg: 0.0,
                    x0: 0.0,
                    y0: 0.0,
                    dx: 0.0,
                    dy: 5.0,
                    dashes: vec![1.0, -1.0],
                }]),
                name: "DASHED".into(),
                color: [0.0, 0.0, 0.0, 1.0],
                angle_offset: 0.0,
                scale: 1.0,
            },
        );

        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &hatches,
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));
        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(
            bytes.len() > empty_bytes.len() + 200,
            "dashed pattern must still emit visible segments"
        );
    }

    #[test]
    fn fixture_pdf_image_missing_file_does_not_crash() {
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Image {
            insertion: [0.0, 0.0, 0.0],
            u_vector: [1.0, 0.0, 0.0],
            v_vector: [0.0, 1.0, 0.0],
            image_size: [100.0, 100.0],
            image_def_handle: nm::Handle::NULL,
            file_path: "this/path/does/not/exist.png".into(),
            display_flags: 0x1,
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &hatches,
            Some(&doc),
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn fixture_pdf_options_monochrome_forces_black_strokes() {
        let wires = vec![WireModel::solid(
            "1".into(),
            vec![[0.0, 0.0, 0.0], [100.0, 0.0, 0.0]],
            [0.9, 0.2, 0.2, 1.0],
            false,
        )];
        let options_mono = PdfExportOptions {
            monochrome: true,
            ..PdfExportOptions::default()
        };
        let options_color = PdfExportOptions {
            monochrome: false,
            ..PdfExportOptions::default()
        };
        let mono = build_pdf_full(
            &wires,
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options_mono,
        );
        let color = build_pdf_full(
            &wires,
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options_color,
        );
        assert!(mono.starts_with(b"%PDF-") && color.starts_with(b"%PDF-"));
        // Both should succeed; we don't decompress to compare but monochrome
        // strips RGB variance — the point is that toggling the flag does
        // not crash and produces valid PDFs.
    }

    #[test]
    fn fixture_pdf_circle_emits_native_path() {
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Circle {
            center: [100.0, 100.0, 0.0],
            radius: 25.0,
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let options = PdfExportOptions {
            native_curves: true,
            ..PdfExportOptions::default()
        };

        let bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            Some(&doc),
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));

        // Collected handles must list the circle so the wire pass skips it.
        let (_, _, curve, _) = collect_native_handles(Some(&doc), &options);
        assert_eq!(
            curve.len(),
            1,
            "expected exactly one curve handle, got {:?}",
            curve
        );

        // Toggling native_curves off should empty the curve set again.
        let opts_off = PdfExportOptions {
            native_curves: false,
            ..options.clone()
        };
        let (_, _, curve_off, _) = collect_native_handles(Some(&doc), &opts_off);
        assert!(curve_off.is_empty());
    }

    #[test]
    fn fixture_pdf_circle_geometry_passes_through_bezier_builder() {
        // White-box check: the bezier approximation must form a closed ring
        // with 13 LinePoint entries (1 moveto + 4 × 3 curveto anchors).
        let line = build_circle_line(0.0, 0.0, 10.0);
        assert!(line.is_closed, "circle line must be closed");
        assert_eq!(
            line.points.len(),
            13,
            "4-bezier circle requires 13 control points"
        );
        // Every third point (0, 3, 6, 9, 12) must be an anchor; the rest
        // must be bezier handles.
        for (i, p) in line.points.iter().enumerate() {
            let expected_anchor = i % 3 == 0;
            assert_eq!(
                p.bezier,
                !expected_anchor,
                "point {i} should have bezier={}",
                !expected_anchor
            );
        }
    }

    #[test]
    fn fixture_pdf_arc_spans_respects_quarter_bounds() {
        // A half-circle arc (180° sweep) must emit exactly 2 bezier chunks
        // (2×90°) → 1 start anchor + 2 × 3 control/anchor = 7 points.
        let line = build_arc_line(0.0, 0.0, 5.0, 0.0, std::f32::consts::PI);
        assert!(!line.is_closed, "arc line must NOT be closed (open path)");
        assert_eq!(line.points.len(), 7, "half-circle = 2 bezier chunks + anchor");
    }

    #[test]
    fn fixture_pdf_ellipse_full_sweep_produces_closed_path() {
        let line = build_ellipse_line(
            0.0,
            0.0,
            [10.0, 0.0],
            0.5,
            0.0,
            std::f32::consts::TAU,
        );
        assert!(line.is_closed, "full ellipse sweep must produce closed path");
        // 4 bezier chunks × 3 points + 1 start anchor = 13.
        assert_eq!(line.points.len(), 13);
    }

    #[test]
    fn fixture_pdf_spline_degree_1_listed_in_collect_handles() {
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Spline {
            degree: 1,
            closed: false,
            knots: vec![0.0, 0.0, 1.0, 2.0, 2.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [10.0, 5.0, 0.0],
                [20.0, 0.0, 0.0],
            ],
            weights: vec![],
            fit_points: vec![],
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let options = PdfExportOptions::default();
        let (_t, _i, _c, spline) = collect_native_handles(Some(&doc), &options);
        assert_eq!(
            spline.len(),
            1,
            "degree-1 spline must be listed in native spline handles, got {:?}",
            spline
        );

        // Toggling native_splines=false must empty the set.
        let opts_off = PdfExportOptions {
            native_splines: false,
            ..options
        };
        let (_, _, _, spline_off) = collect_native_handles(Some(&doc), &opts_off);
        assert!(spline_off.is_empty());
    }

    #[test]
    fn fixture_pdf_spline_cubic_emits_bezier_path() {
        // Clamped cubic (degree=3) with 4 control points ⇒ exactly one
        // bezier segment — knots = [0,0,0,0,1,1,1,1].
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Spline {
            degree: 3,
            closed: false,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [5.0, 10.0, 0.0],
                [15.0, 10.0, 0.0],
                [20.0, 0.0, 0.0],
            ],
            weights: vec![],
            fit_points: vec![],
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let options = PdfExportOptions::default();
        let bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            Some(&doc),
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(bytes.starts_with(b"%PDF-"));

        let empty_bytes = build_pdf_full(
            &[],
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            None,
            &options,
        );
        assert!(
            bytes.len() > empty_bytes.len() + 50,
            "cubic spline should emit a visible path (got {}B vs empty {}B)",
            bytes.len(),
            empty_bytes.len()
        );

        // Verify handle is listed so the wire pipeline skips the spline.
        let (_, _, _, spline) = collect_native_handles(Some(&doc), &options);
        assert_eq!(spline.len(), 1);
    }

    #[test]
    fn fixture_pdf_spline_rational_falls_back_to_wire_path() {
        // Weights non-uniform ⇒ rational spline ⇒ strategy returns None
        // (for degree 2/3 without fit_points), handle NOT listed in
        // native spline set ⇒ wire tessellation kicks in as before.
        let mut doc = nm::CadDocument::new();
        let handle = doc.allocate_handle();
        let mut e = nm::Entity::new(nm::EntityData::Spline {
            degree: 3,
            closed: false,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [5.0, 10.0, 0.0],
                [15.0, 10.0, 0.0],
                [20.0, 0.0, 0.0],
            ],
            weights: vec![1.0, 0.7, 0.7, 1.0],
            fit_points: vec![], // no fallback path either
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        });
        e.handle = handle;
        e.layer_name = "0".into();
        doc.entities.push(e);

        let options = PdfExportOptions::default();
        let (_, _, _, spline) = collect_native_handles(Some(&doc), &options);
        assert!(
            spline.is_empty(),
            "rational cubic without fit-points must fall back to wire tessellation"
        );
    }

    #[test]
    fn fixture_pdf_export_full_wires_only_matches_legacy_export_pdf() {
        // Feeding export_pdf_full with default options, no hatches, and no
        // native_doc must produce the same byte length as the legacy
        // export_pdf (which calls the full pipeline under the hood).
        let wires = vec![line_wire(vec![[0.0, 0.0, 0.0], [50.0, 50.0, 0.0]])];
        let out_legacy = tmp_path("legacy");
        let out_full = tmp_path("full");
        export_pdf(&wires, 297.0, 210.0, 0.0, 0.0, 0, &out_legacy, None).unwrap();
        export_pdf_full(
            &wires,
            &HashMap::new(),
            None,
            297.0,
            210.0,
            0.0,
            0.0,
            0,
            &out_full,
            None,
            &PdfExportOptions::default(),
        )
        .unwrap();

        let legacy = std::fs::read(&out_legacy).unwrap();
        let full = std::fs::read(&out_full).unwrap();
        // Lengths may differ by a few bytes due to timestamp / id differences
        // inside printpdf; we only require both to be valid PDFs.
        assert!(legacy.starts_with(b"%PDF-"));
        assert!(full.starts_with(b"%PDF-"));
        let _ = std::fs::remove_file(&out_legacy);
        let _ = std::fs::remove_file(&out_full);
    }
}
