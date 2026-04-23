// SVG export — converts CAD drawings to SVG files.
//
// Phase 3 mirrors the ODA OdSvgExportEx.cpp algorithm end-to-end, with
// device properties `ColorPolicy=1`, `UseLineTypes=true`, `UseTextOut=true`,
// `ExplodeShxTexts=false`, `MinimalWidth=0.1`, `LineWeightScale`,
// `ImageBase` / `ImageUrl` / `DefaultImageExt`, and a responsive viewBox.
// The post-processing Python script bundled with ODA is no longer needed:
// H7CAD emits native SVG text / images / curves directly.
//
// Entry point:
// * `export_svg_full()` — full output with native text, hatch fills,
//   raster-image `<image>` elements, native `<circle>` / `<ellipse>`
//   / `<path A>` curves, block defs/use deduplication, and CTB plot
//   styles.  Behaviour is driven entirely by `SvgExportOptions`.
//
// Layer order inside the Y-flipped `<g>` (from bottom to top):
//   1. `<defs>` for block symbols (Insert/Block deduplication)
//   2. `<image>` for RasterImage entities (layer 0 — below everything)
//   3. `<polygon>` for hatch fills (layer 1)
//   4. `<polyline>` for WireModel curves (layer 2)
//   5. `<use>` for Insert references (layer 2b)
//   6. `<circle>` / `<ellipse>` / `<path A>` for native curves (layer 2c)
//   7. `<text>` for native Text / MText entities (layer 3 — top)

use crate::io::plot_style::PlotStyleTable;
use crate::scene::hatch_model::{HatchModel, HatchPattern};
use crate::scene::WireModel;
use acadrust::Handle;
use h7cad_native_model as nm;
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;

// ── Options ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SvgExportOptions {
    /// All strokes forced to black (default true — matches ODA ColorPolicy=1).
    pub monochrome: bool,
    /// Emit text entities as tessellated geometry instead of `<text>` (default false).
    pub text_as_geometry: bool,
    /// Font family string written into `<text>` elements.
    pub font_family: String,
    /// Multiply native font height by this factor (default 0.8 — matches ODA post-process).
    pub font_size_scale: f32,
    /// Floor for stroke-width in mm (default 0.1 — matches ODA MinimalWidth).
    pub min_stroke_width: f32,
    /// Whether to emit hatch fill polygons.
    pub include_hatches: bool,
    /// Collapse simple block references (Line/Circle/Arc/LwPolyline only) into
    /// `<defs>` + `<use>` to deduplicate repeated block geometry.  Blocks
    /// containing Text/MText/Hatch/nested-Insert fall back to wire output.
    pub use_block_defs: bool,
    /// Whether to emit `<image>` elements for RasterImage (IMAGE) entities.
    /// Mirrors ODA's raster pipeline driven by `ImageBase` / `ImageUrl`.
    pub include_images: bool,
    /// When true, image bytes are inlined as `data:<mime>;base64,...`
    /// so the resulting SVG is self-contained.  When false, the `<image>`
    /// element points at `{image_url_prefix}{filename}` (mirrors ODA's
    /// `ImageUrl` property, which defaults to `"./"`).
    pub embed_images: bool,
    /// Prefix prepended to the image filename when `embed_images=false`.
    /// Default `"./"` matches ODA's `ImageUrl` property.
    pub image_url_prefix: String,
    /// When set, image file paths are resolved relative to this directory
    /// before embedding (used to locate images when the DXF carries a
    /// relative `file_path`).  `None` means paths are used as-is.
    /// Mirrors ODA's `ImageBase` property.
    pub image_base: Option<std::path::PathBuf>,
    /// Emit `<circle>` / `<ellipse>` / `<path d="A ...">` for native
    /// Circle / Arc / Ellipse entities instead of relying on WireModel
    /// tessellation.  Produces smaller, resolution-independent SVG output
    /// where ODA's 2dExportDevice would otherwise tessellate curves.
    pub native_curves: bool,
    /// Multiplier applied to `WireModel.line_weight_px` when building the
    /// SVG `stroke-width` attribute.  Default `0.2646` converts 96-dpi
    /// pixels to mm (matches the paper-space units the rest of the
    /// exporter uses).  Mirrors ODA's `LineWeightScale` device property.
    pub line_weight_scale: f32,
    /// Emit native Splines whenever possible (degree=1 control-point
    /// polyline, or degree=3 fit-point polyline fallback).  Complex
    /// NURBS curves still defer to WireModel tessellation.  Mirrors the
    /// same native-vs-tessellate trade-off `native_curves` makes.
    pub native_splines: bool,
}

impl Default for SvgExportOptions {
    fn default() -> Self {
        Self {
            monochrome: true,
            text_as_geometry: false,
            font_family: "SimSun, 宋体".into(),
            font_size_scale: 0.8,
            min_stroke_width: 0.1,
            include_hatches: true,
            use_block_defs: true,
            include_images: true,
            embed_images: true,
            image_url_prefix: "./".into(),
            image_base: None,
            native_curves: true,
            line_weight_scale: 0.2646,
            native_splines: true,
        }
    }
}

// ── Public entry points ────────────────────────────────────────────────────

/// Enhanced SVG export with native text, hatches, and configurable options.
pub fn export_svg_full(
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
    options: &SvgExportOptions,
) -> Result<(), String> {
    let content = build_svg_full(
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
    file.write_all(content.as_bytes()).map_err(|e| e.to_string())
}

/// Show an SVG save-file dialog and return the chosen path (or `None` if cancelled).
pub async fn pick_svg_path_owned(stem: String) -> Option<std::path::PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Export as SVG")
        .set_file_name(&format!("{stem}.svg"))
        .add_filter("SVG Files", &["svg"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

// ── SVG builder ────────────────────────────────────────────────────────────

fn build_svg_full(
    wires: &[WireModel],
    hatches: &HashMap<Handle, HatchModel>,
    native_doc: Option<&nm::CadDocument>,
    paper_w: f32,
    paper_h: f32,
    ox: f32,
    oy: f32,
    rotation_deg: i32,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) -> String {
    let est = wires.len() * 200 + hatches.len() * 300 + 2048;
    let mut svg = String::with_capacity(est);

    // XML declaration + root <svg>.
    svg.push_str("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\"?>\n");
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" \
         viewBox=\"0 0 {pw} {ph}\" \
         width=\"100%\" height=\"100%\" \
         preserveAspectRatio=\"xMidYMid meet\" \
         stroke-linecap=\"round\" stroke-linejoin=\"round\" \
         fill-rule=\"evenodd\" xml:space=\"preserve\">\n",
        pw = fmt_f32(paper_w),
        ph = fmt_f32(paper_h),
    );

    // White background.
    let _ = write!(
        svg,
        "<rect width=\"{pw}\" height=\"{ph}\" fill=\"white\" />\n",
        pw = fmt_f32(paper_w),
        ph = fmt_f32(paper_h),
    );

    // Global transform: Y-flip (CAD Y-up → SVG Y-down).
    let transform = build_transform(paper_w, paper_h, rotation_deg);
    let _ = write!(svg, "<g transform=\"{transform}\">\n");

    // ── Pre-pass: collect skip handles + eligible Insert/block set ─────────
    let mut skip_handles: std::collections::HashSet<String> = std::collections::HashSet::new();
    if !options.text_as_geometry {
        if let Some(doc) = native_doc {
            skip_handles.extend(collect_text_handles(doc));
        }
    }
    if options.include_images {
        if let Some(doc) = native_doc {
            skip_handles.extend(collect_image_handles(doc));
        }
    }
    if options.native_curves {
        if let Some(doc) = native_doc {
            skip_handles.extend(collect_native_curve_handles(doc));
        }
    }
    if options.native_splines {
        if let Some(doc) = native_doc {
            skip_handles.extend(collect_emittable_spline_handles(doc));
        }
    }

    let eligible: Vec<EligibleInsert> = if options.use_block_defs {
        if let Some(doc) = native_doc {
            let list = collect_eligible_inserts(doc);
            for e in &list {
                skip_handles.insert(e.insert_handle.clone());
            }
            list
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // ── <defs>: block geometry shared by eligible Inserts ─────────────────
    if !eligible.is_empty() {
        emit_block_defs(&mut svg, &eligible, options);
    }

    // ── Layer 0: Raster images (below hatches so strokes sit on top) ───────
    if options.include_images {
        if let Some(doc) = native_doc {
            emit_raster_images(&mut svg, doc, ox, oy, options);
        }
    }

    // ── Layer 1: Hatch fills (bottom) ──────────────────────────────────────
    if options.include_hatches && !hatches.is_empty() {
        emit_hatches(&mut svg, hatches, ox, oy, options);
    }

    // ── Layer 2: Wire geometry (middle) ────────────────────────────────────
    emit_wires(&mut svg, wires, ox, oy, plot_style, options, &skip_handles);

    // ── Layer 2b: <use> references for eligible Inserts ───────────────────
    if !eligible.is_empty() {
        emit_insert_uses(&mut svg, &eligible, ox, oy, options);
    }

    // ── Layer 2c: Native curves (circles, arcs, ellipses) ─────────────────
    if options.native_curves {
        if let Some(doc) = native_doc {
            emit_native_curves(&mut svg, doc, ox, oy, plot_style, options);
        }
    }

    // ── Layer 2d: Native splines (degree 1 / fit-point fallback) ──────────
    if options.native_splines {
        if let Some(doc) = native_doc {
            emit_native_splines(&mut svg, doc, ox, oy, plot_style, options);
        }
    }

    // ── Layer 3: Text elements (top) ──────────────────────────────────────
    if !options.text_as_geometry {
        if let Some(doc) = native_doc {
            emit_text_entities(&mut svg, doc, ox, oy, options);
        }
    }

    svg.push_str("</g>\n</svg>\n");
    svg
}

// ── Wire emission (polylines from WireModel) ───────────────────────────────

fn emit_wires(
    svg: &mut String,
    wires: &[WireModel],
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
    skip_handles: &std::collections::HashSet<String>,
) {
    for wire in wires {
        let [mut r, mut g, mut b, a] = wire.color;
        if a < 0.01 || wire.name == "__paper_boundary__" {
            continue;
        }
        if skip_handles.contains(&wire.name) {
            continue;
        }

        // CTB plot style overrides.
        let mut lw_override: Option<f32> = None;
        if let Some(ctb) = plot_style {
            if wire.aci > 0 {
                if let Some([cr, cg, cb]) = ctb.resolve_color(wire.aci) {
                    r = cr;
                    g = cg;
                    b = cb;
                }
                lw_override = ctb.resolve_lineweight(wire.aci);
            }
        }

        // Monochrome: map light/yellow to black.
        if options.monochrome {
            r = 0.0;
            g = 0.0;
            b = 0.0;
        } else if lw_override.is_none() {
            let is_light = r > 0.80 && g > 0.80 && b > 0.80;
            let is_yellow = r > 0.80 && g > 0.70 && b < 0.30;
            if is_light || is_yellow {
                r = 0.0;
                g = 0.0;
                b = 0.0;
            }
        }

        let stroke = format!(
            "rgb({},{},{})",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );

        let lw = lw_override
            .unwrap_or_else(|| wire.line_weight_px * options.line_weight_scale)
            .max(options.min_stroke_width);

        let dasharray = if wire.pattern_length > 0.0 {
            build_dasharray(&wire.pattern)
        } else {
            String::new()
        };

        let mut segment: Vec<[f32; 2]> = Vec::new();
        for &[x, y, _z] in &wire.points {
            if x.is_nan() || y.is_nan() {
                flush_polyline(svg, &segment, &stroke, lw, &dasharray);
                segment.clear();
            } else {
                segment.push([x + ox, y + oy]);
            }
        }
        flush_polyline(svg, &segment, &stroke, lw, &dasharray);
    }
}

// ── Hatch fill emission ────────────────────────────────────────────────────

fn emit_hatches(
    svg: &mut String,
    hatches: &HashMap<Handle, HatchModel>,
    ox: f32,
    oy: f32,
    options: &SvgExportOptions,
) {
    for (_handle, hatch) in hatches {
        if hatch.boundary.is_empty() {
            continue;
        }

        let fill_color = if options.monochrome {
            "rgb(0,0,0)".to_string()
        } else {
            let [r, g, b, _a] = hatch.color;
            format!(
                "rgb({},{},{})",
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
            )
        };

        match &hatch.pattern {
            HatchPattern::Solid => {
                svg.push_str("<polygon fill=\"");
                svg.push_str(&fill_color);
                svg.push_str("\" stroke=\"none\" points=\"");
                for (i, &[x, y]) in hatch.boundary.iter().enumerate() {
                    if i > 0 {
                        svg.push(' ');
                    }
                    svg.push_str(&fmt_f32(x + ox));
                    svg.push(',');
                    svg.push_str(&fmt_f32(y + oy));
                }
                svg.push_str("\" />\n");
            }
            HatchPattern::Gradient { .. } => {
                // Simplified: solid fill with base color for now.
                svg.push_str("<polygon fill=\"");
                svg.push_str(&fill_color);
                svg.push_str("\" stroke=\"none\" opacity=\"0.5\" points=\"");
                for (i, &[x, y]) in hatch.boundary.iter().enumerate() {
                    if i > 0 {
                        svg.push(' ');
                    }
                    svg.push_str(&fmt_f32(x + ox));
                    svg.push(',');
                    svg.push_str(&fmt_f32(y + oy));
                }
                svg.push_str("\" />\n");
            }
            HatchPattern::Pattern(_families) => {
                // Pattern hatches: render boundary outline only (GPU pattern not
                // trivially reproducible in static SVG).
                svg.push_str("<polygon fill=\"none\" stroke=\"");
                svg.push_str(&fill_color);
                svg.push_str("\" stroke-width=\"0.1\" points=\"");
                for (i, &[x, y]) in hatch.boundary.iter().enumerate() {
                    if i > 0 {
                        svg.push(' ');
                    }
                    svg.push_str(&fmt_f32(x + ox));
                    svg.push(',');
                    svg.push_str(&fmt_f32(y + oy));
                }
                svg.push_str("\" />\n");
            }
        }
    }
}

// ── Text handle collection (for wire dedup) ────────────────────────────────

fn collect_text_handles(doc: &nm::CadDocument) -> std::collections::HashSet<String> {
    let mut handles = std::collections::HashSet::new();
    for entity in &doc.entities {
        match &entity.data {
            nm::EntityData::Text { .. } | nm::EntityData::MText { .. } => {
                handles.insert(entity.handle.value().to_string());
            }
            _ => {}
        }
    }
    handles
}

// ── Image handle collection (for wire dedup) ───────────────────────────────

fn collect_image_handles(doc: &nm::CadDocument) -> std::collections::HashSet<String> {
    let mut handles = std::collections::HashSet::new();
    for entity in &doc.entities {
        if matches!(&entity.data, nm::EntityData::Image { .. }) {
            handles.insert(entity.handle.value().to_string());
        }
    }
    handles
}

// ── Native curve handle collection (for wire dedup) ────────────────────────

fn collect_native_curve_handles(doc: &nm::CadDocument) -> std::collections::HashSet<String> {
    let mut handles = std::collections::HashSet::new();
    for entity in &doc.entities {
        match &entity.data {
            nm::EntityData::Circle { .. }
            | nm::EntityData::Arc { .. }
            | nm::EntityData::Ellipse { .. }
            | nm::EntityData::LwPolyline { .. } => {
                handles.insert(entity.handle.value().to_string());
            }
            _ => {}
        }
    }
    handles
}

/// Returns handles for only those splines we can emit natively, so unhandled
/// NURBS configurations keep flowing through the WireModel tessellation pass.
fn collect_emittable_spline_handles(
    doc: &nm::CadDocument,
) -> std::collections::HashSet<String> {
    let mut handles = std::collections::HashSet::new();
    for entity in &doc.entities {
        if let nm::EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            ..
        } = &entity.data
        {
            if spline_emit_strategy(*degree, *closed, knots, control_points, weights, fit_points)
                .is_some()
            {
                handles.insert(entity.handle.value().to_string());
            }
        }
    }
    handles
}

/// Pick an emission strategy for a DXF spline.  Returns `None` when the
/// spline needs the wire fallback (high-order NURBS without fit points).
enum SplineEmit {
    /// Degree 1 spline — control polygon IS the curve.
    ControlPoly,
    /// Clamped non-rational degree 2/3 NURBS decomposed into piecewise Bezier.
    /// The payload is a flat `Vec` of control points of length
    /// `segments * degree + 1`, where segment `s` spans
    /// `[s*degree ..= s*degree + degree]`.
    Bezier {
        degree: usize,
        control_points: Vec<[f64; 3]>,
    },
    /// Curve has fit points → polyline through them is visually close.
    FitPoly,
}

fn spline_emit_strategy(
    degree: i32,
    closed: bool,
    knots: &[f64],
    control_points: &[[f64; 3]],
    weights: &[f64],
    fit_points: &[[f64; 3]],
) -> Option<SplineEmit> {
    if degree == 1 && control_points.len() >= 2 {
        return Some(SplineEmit::ControlPoly);
    }
    // Phase 7: NURBS → piecewise Bezier for degree 2/3, clamped, non-rational.
    // Closed/periodic and true rational curves still defer to fit-poly / wire.
    if !closed && (degree == 2 || degree == 3) {
        let non_rational = weights.is_empty()
            || weights.iter().all(|w| (w - 1.0).abs() < 1e-9)
            || {
                let first = weights[0];
                weights.iter().all(|w| (w - first).abs() < 1e-9)
            };
        if non_rational {
            if let Some(pts) = bspline_to_bezier(degree as usize, knots, control_points) {
                return Some(SplineEmit::Bezier {
                    degree: degree as usize,
                    control_points: pts,
                });
            }
        }
    }
    if fit_points.len() >= 2 {
        return Some(SplineEmit::FitPoly);
    }
    None
}

// ── B-spline → Bezier decomposition (Boehm knot insertion) ─────────────────
//
// Converts a clamped non-rational B-spline into piecewise Bezier control
// points.  For each distinct internal knot we insert the knot value until
// its multiplicity equals `degree`; once that's done, every consecutive
// `degree+1` control points form a Bezier segment with C0 continuity.
//
// Complexity: O(k · S · n) where k = degree, S = distinct-internal-knot
// count, n = control-point count — cheap even for splines with hundreds of
// control points that we typically see in real DXF drawings.

/// Convert a clamped, non-rational B-spline into piecewise Bezier control
/// points.  Returns `None` when inputs are inconsistent or the spline is
/// not clamped (first/last knot repeated `degree+1` times).
///
/// The returned vector has length `segments * degree + 1`; segment `s`
/// spans indices `[s*degree ..= s*degree + degree]`.  Adjacent segments
/// share the boundary control point, giving a natural `M … C … C …`
/// (cubic) or `M … Q … Q …` (quadratic) SVG path layout.
fn bspline_to_bezier(
    degree: usize,
    knots: &[f64],
    control_points: &[[f64; 3]],
) -> Option<Vec<[f64; 3]>> {
    let k = degree;
    if !(2..=3).contains(&k) {
        return None;
    }
    let n_plus_1 = control_points.len();
    // Knot vector length invariant: m+1 = n + k + 2 ⇒ n + k + 2 == knots.len()
    if n_plus_1 < k + 1 || knots.len() != n_plus_1 + k + 1 {
        return None;
    }
    // Clamped: first k+1 and last k+1 knots each equal.
    let u0 = knots[0];
    let u_last = knots[knots.len() - 1];
    if !knots.iter().take(k + 1).all(|u| (u - u0).abs() < 1e-9)
        || !knots
            .iter()
            .rev()
            .take(k + 1)
            .all(|u| (u - u_last).abs() < 1e-9)
    {
        return None;
    }
    if (u_last - u0).abs() < 1e-12 {
        return None;
    }

    let mut cps = control_points.to_vec();
    let mut ks = knots.to_vec();

    // Distinct internal knot values (strict interior: indices (k, m-k)).
    let m = ks.len() - 1;
    let mut distinct: Vec<f64> = Vec::new();
    let mut i = k + 1;
    while i < m - k {
        let u = ks[i];
        if distinct.last().map_or(true, |&prev| (prev - u).abs() > 1e-12) {
            distinct.push(u);
        }
        i += 1;
    }

    for u in distinct {
        let mult = ks.iter().filter(|&&x| (x - u).abs() < 1e-12).count();
        let need = k.saturating_sub(mult);
        for _ in 0..need {
            insert_knot_once(k, &mut ks, &mut cps, u)?;
        }
    }

    // Output invariant: segments * degree + 1 control points.
    Some(cps)
}

/// One Boehm knot insertion step.  Finds `j` such that
/// `knots[j] <= bar_u < knots[j+1]` and produces new control points
/// `Q_0 .. Q_{n+1}` plus knot vector with `bar_u` inserted after index `j`.
fn insert_knot_once(
    degree: usize,
    knots: &mut Vec<f64>,
    cps: &mut Vec<[f64; 3]>,
    bar_u: f64,
) -> Option<()> {
    let k = degree;
    let mut j: Option<usize> = None;
    for i in 0..knots.len() - 1 {
        if knots[i] <= bar_u && bar_u < knots[i + 1] {
            j = Some(i);
            break;
        }
    }
    let j = j?;
    // For a clamped spline and internal knot, j ≥ k always holds.
    if j < k {
        return None;
    }
    let n = cps.len() - 1;
    let mut q: Vec<[f64; 3]> = Vec::with_capacity(n + 2);
    for i in 0..=n + 1 {
        if i + k <= j {
            q.push(cps[i]);
        } else if i <= j {
            let denom = knots[i + k] - knots[i];
            let alpha = if denom.abs() < 1e-12 {
                0.0
            } else {
                (bar_u - knots[i]) / denom
            };
            let a = cps[i - 1];
            let b = cps[i];
            q.push([
                (1.0 - alpha) * a[0] + alpha * b[0],
                (1.0 - alpha) * a[1] + alpha * b[1],
                (1.0 - alpha) * a[2] + alpha * b[2],
            ]);
        } else {
            q.push(cps[i - 1]);
        }
    }
    knots.insert(j + 1, bar_u);
    *cps = q;
    Some(())
}

// ── Native curve emission (Circle / Arc / Ellipse → native SVG elements) ───
//
// Top-level curves are normally rasterised by the WireModel pipeline.  ODA's
// 2dExportDevice does the same: curves are tessellated into many short line
// segments.  Emitting them as native `<circle>` / `<ellipse>` / `<path>`
// elements instead produces much smaller SVG files and stays crisp at any
// zoom level.  Curves that live *inside* a block are already handled by the
// existing defs/use pipeline, so this pass only looks at `doc.entities`.

fn emit_native_curves(
    svg: &mut String,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) {
    let frozen_layers: std::collections::HashSet<&str> = doc
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
        let stroke = resolve_entity_stroke(entity, plot_style, options);
        let lw = resolve_entity_lineweight(entity, plot_style, options);

        match &entity.data {
            nm::EntityData::Circle { center, radius } => {
                let _ = write!(
                    svg,
                    "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
                    cx = fmt_f32(center[0] as f32 + ox),
                    cy = fmt_f32(center[1] as f32 + oy),
                    r = fmt_f32(*radius as f32),
                    s = stroke,
                    w = fmt_f32(lw),
                );
            }
            nm::EntityData::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                emit_arc_path(
                    svg,
                    [center[0] as f32 + ox, center[1] as f32 + oy],
                    *radius as f32,
                    *start_angle as f32,
                    *end_angle as f32,
                    &stroke,
                    lw,
                );
            }
            nm::EntityData::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                emit_ellipse(
                    svg,
                    [center[0] + ox as f64, center[1] + oy as f64],
                    *major_axis,
                    *ratio,
                    *start_param,
                    *end_param,
                    &stroke,
                    lw,
                );
            }
            nm::EntityData::LwPolyline {
                vertices, closed, ..
            } => {
                // Phase 4 T2: top-level polylines (straight and bulged alike)
                // now render as a single native `<path>` instead of going
                // through WireModel tessellation.
                let verts: Vec<PolylineVertex> = vertices
                    .iter()
                    .map(|v| PolylineVertex {
                        x: v.x + ox as f64,
                        y: v.y + oy as f64,
                        bulge: v.bulge,
                    })
                    .collect();
                emit_polyline_path(svg, &verts, *closed, &stroke, lw);
            }
            _ => {}
        }
    }
}

/// Phase 5 P2: native Spline emission.  Degree 1 splines become a polyline
/// through their control points (exact).  Splines that carry DXF fit points
/// render as a polyline through those — visually close to the true curve
/// without implementing full NURBS-to-Bezier conversion.  Anything else is
/// left on the WireModel path so tessellated output remains correct.
fn emit_native_splines(
    svg: &mut String,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) {
    let frozen_layers: std::collections::HashSet<&str> = doc
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
        let Some(strategy) =
            spline_emit_strategy(*degree, *closed, knots, control_points, weights, fit_points)
        else {
            continue;
        };

        let stroke = resolve_entity_stroke(entity, plot_style, options);
        let lw = resolve_entity_lineweight(entity, plot_style, options);

        match strategy {
            SplineEmit::ControlPoly => {
                let vertices = offset_polyline_vertices(control_points, ox, oy);
                emit_polyline_path(svg, &vertices, *closed, &stroke, lw);
            }
            SplineEmit::FitPoly => {
                let vertices = offset_polyline_vertices(fit_points, ox, oy);
                emit_polyline_path(svg, &vertices, *closed, &stroke, lw);
            }
            SplineEmit::Bezier {
                degree: k,
                control_points: refined,
            } => {
                emit_bezier_path(svg, k, &refined, ox, oy, &stroke, lw);
            }
        }
    }
}

/// Helper used by the polyline-based spline strategies.  Produces the
/// zero-bulge `PolylineVertex` list with the scene offset already applied.
fn offset_polyline_vertices(src: &[[f64; 3]], ox: f32, oy: f32) -> Vec<PolylineVertex> {
    src.iter()
        .map(|p| PolylineVertex {
            x: p[0] + ox as f64,
            y: p[1] + oy as f64,
            bulge: 0.0,
        })
        .collect()
}

/// Emit an SVG `<path>` tracing piecewise Bezier segments.  `refined` is the
/// flat control-point list returned by `bspline_to_bezier`; adjacent segments
/// share the boundary point so the SVG path only needs `degree` points per
/// segment after the initial `M`.
fn emit_bezier_path(
    svg: &mut String,
    degree: usize,
    refined: &[[f64; 3]],
    ox: f32,
    oy: f32,
    stroke: &str,
    lw: f32,
) {
    if refined.len() < degree + 1 {
        return;
    }
    let op = match degree {
        2 => 'Q',
        3 => 'C',
        _ => return,
    };
    let seg_count = (refined.len() - 1) / degree;
    if seg_count == 0 {
        return;
    }

    let mut d = String::with_capacity(32 + seg_count * (degree * 24 + 4));
    let p0 = refined[0];
    let _ = write!(
        d,
        "M {x} {y}",
        x = fmt_f32(p0[0] as f32 + ox),
        y = fmt_f32(p0[1] as f32 + oy),
    );
    for s in 0..seg_count {
        d.push(' ');
        d.push(op);
        for offset in 1..=degree {
            let p = refined[s * degree + offset];
            let _ = write!(
                d,
                " {x} {y}",
                x = fmt_f32(p[0] as f32 + ox),
                y = fmt_f32(p[1] as f32 + oy),
            );
        }
    }

    svg.push_str("<path d=\"");
    svg.push_str(&d);
    svg.push_str("\" fill=\"none\" stroke=\"");
    svg.push_str(stroke);
    svg.push_str("\" stroke-width=\"");
    svg.push_str(&fmt_f32(lw));
    svg.push_str("\"/>\n");
}

/// Compute an SVG `stroke="..."` value for a native entity, honouring
/// monochrome mode and any CTB plot-style override.  True-color entities
/// win over ACI; otherwise the ACI → RGB mapping in `resolve_entity_fill`
/// is reused.
fn resolve_entity_stroke(
    entity: &nm::Entity,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) -> String {
    if options.monochrome {
        return "rgb(0,0,0)".to_string();
    }
    if let Some(ctb) = plot_style {
        if let Some(aci) = aci_for_ctb(entity.color_index) {
            if let Some([r, g, b]) = ctb.resolve_color(aci) {
                return format!(
                    "rgb({},{},{})",
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
            }
        }
    }
    resolve_entity_fill(entity, options)
}

/// Stroke width for a native entity.  CTB lineweight wins when available;
/// otherwise we fall back to the `min_stroke_width` option so curves never
/// disappear at low zoom (matches ODA's `MinimalWidth=0.1`).
fn resolve_entity_lineweight(
    entity: &nm::Entity,
    plot_style: Option<&PlotStyleTable>,
    options: &SvgExportOptions,
) -> f32 {
    if let Some(ctb) = plot_style {
        if let Some(aci) = aci_for_ctb(entity.color_index) {
            if let Some(w) = ctb.resolve_lineweight(aci) {
                return w.max(options.min_stroke_width);
            }
        }
    }
    options.min_stroke_width
}

/// Narrow the native-model's i16 color index to the 1..=255 ACI range the
/// CTB lookup table accepts.  256 = ByLayer, 0 = ByBlock, <0 = special.
fn aci_for_ctb(color_index: i16) -> Option<u8> {
    if (1..=255).contains(&color_index) {
        Some(color_index as u8)
    } else {
        None
    }
}

/// Emit an SVG `<path>` tracing a CAD arc.  The outer Y-flip group maps
/// CAD CCW to SVG "sweep flag = 1"; `large-arc-flag` is chosen to match the
/// angular span exactly the same way as `BlockPrimitive::Arc`.
fn emit_arc_path(
    svg: &mut String,
    center: [f32; 2],
    radius: f32,
    start_angle_deg: f32,
    end_angle_deg: f32,
    stroke: &str,
    lw: f32,
) {
    let a0 = (start_angle_deg as f64).to_radians();
    let mut a1 = (end_angle_deg as f64).to_radians();
    if a1 <= a0 {
        a1 += std::f64::consts::TAU;
    }
    let sweep = a1 - a0;
    let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
    let sx = center[0] as f64 + radius as f64 * a0.cos();
    let sy = center[1] as f64 + radius as f64 * a0.sin();
    let ex = center[0] as f64 + radius as f64 * a1.cos();
    let ey = center[1] as f64 + radius as f64 * a1.sin();
    let _ = write!(
        svg,
        "<path d=\"M {sx} {sy} A {r} {r} 0 {l} 1 {ex} {ey}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
        sx = fmt_f32(sx as f32),
        sy = fmt_f32(sy as f32),
        r = fmt_f32(radius),
        l = large,
        ex = fmt_f32(ex as f32),
        ey = fmt_f32(ey as f32),
        s = stroke,
        w = fmt_f32(lw),
    );
}

/// Emit either `<ellipse>` (full) or `<path>` (partial arc) for a CAD
/// Ellipse entity.  The `major_axis` vector is applied as a `rotate(...)`
/// transform around the center for the full-ellipse case.
fn emit_ellipse(
    svg: &mut String,
    center: [f64; 2],
    major_axis: [f64; 3],
    ratio: f64,
    start_param: f64,
    end_param: f64,
    stroke: &str,
    lw: f32,
) {
    let rmaj = (major_axis[0].powi(2) + major_axis[1].powi(2)).sqrt();
    if rmaj < 1e-9 {
        return;
    }
    let rmin = rmaj * ratio.abs();
    let rot_rad = major_axis[1].atan2(major_axis[0]);
    let rot_deg = rot_rad.to_degrees();

    // Normalise sweep to [0, 2π).  DXF stores 0..TAU for a full ellipse.
    let mut sweep = end_param - start_param;
    while sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let is_full = (sweep - std::f64::consts::TAU).abs() < 1e-6
        || (sweep - 0.0).abs() < 1e-6;

    if is_full {
        let _ = write!(
            svg,
            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{rx}\" ry=\"{ry}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" transform=\"rotate({rd},{cx},{cy})\" />\n",
            cx = fmt_f32(center[0] as f32),
            cy = fmt_f32(center[1] as f32),
            rx = fmt_f32(rmaj as f32),
            ry = fmt_f32(rmin as f32),
            s = stroke,
            w = fmt_f32(lw),
            rd = fmt_f32(rot_deg as f32),
        );
        return;
    }

    // Partial arc: compute start / end points in world coords, then emit a
    // single `A` command with the ellipse's axis rotation baked in.
    let cos_rot = rot_rad.cos();
    let sin_rot = rot_rad.sin();
    let pt_at = |t: f64| -> [f64; 2] {
        let ct = t.cos();
        let st = t.sin();
        [
            center[0] + ct * rmaj * cos_rot - st * rmin * sin_rot,
            center[1] + ct * rmaj * sin_rot + st * rmin * cos_rot,
        ]
    };
    let [sx, sy] = pt_at(start_param);
    let [ex, ey] = pt_at(end_param);
    let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
    let _ = write!(
        svg,
        "<path d=\"M {sx} {sy} A {rx} {ry} {rd} {l} 1 {ex} {ey}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
        sx = fmt_f32(sx as f32),
        sy = fmt_f32(sy as f32),
        rx = fmt_f32(rmaj as f32),
        ry = fmt_f32(rmin as f32),
        rd = fmt_f32(rot_deg as f32),
        l = large,
        ex = fmt_f32(ex as f32),
        ey = fmt_f32(ey as f32),
        s = stroke,
        w = fmt_f32(lw),
    );
}

// ── Raster image emission (ODA `ImageBase` / `ImageUrl` / `DefaultImageExt`)
//
// Each IMAGE entity becomes a single `<image>` element with a `matrix(...)`
// transform that reproduces the entity's U/V vectors.  Because the enclosing
// group applies `scale(1,-1)`, we build the matrix so SVG-local (0,0) lands
// on the CAD *top-left* corner of the image (insertion + v·h), with local
// (w, h) landing on the CAD bottom-right.  Pixel orientation therefore stays
// upright after the outer Y-flip.
//
// The algorithm mirrors ODA's OdSvgExportEx pipeline: the `ImageBase` /
// `ImageUrl` / `DefaultImageExt` device properties choose between inlined
// base64 blobs (self-contained SVG, default in H7CAD) and external URLs.

fn emit_raster_images(
    svg: &mut String,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    options: &SvgExportOptions,
) {
    let frozen_layers: std::collections::HashSet<&str> = doc
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
        // DXF code 70 bit 1 = SHOW_IMAGE. When the bit is clear the viewer
        // should hide the image (e.g. regen-off in AutoCAD). Skip to match.
        if (*display_flags & 0x1) == 0 {
            continue;
        }

        let href = match image_href(file_path, options) {
            Some(h) => h,
            None => continue,
        };

        let w_px = image_size[0] as f32;
        let h_px = image_size[1] as f32;
        if !(w_px > 0.0 && h_px > 0.0) {
            continue;
        }

        // Build the affine mapping from image-local pixel coords (SVG Y-down)
        // to CAD world coords.  Image corners in world space:
        //   TL (CAD) = insertion + v·h_px   ← pixel (0, 0)
        //   TR (CAD) = insertion + u·w + v·h ← pixel (w, 0)
        //   BL (CAD) = insertion            ← pixel (0, h)
        //   BR (CAD) = insertion + u·w       ← pixel (w, h)
        //
        // Derivation: matrix(a, b, c, d, e, f) with
        //   (a, b) = u, (c, d) = -v, (e, f) = insertion + v·h
        let ux = u_vector[0] as f32;
        let uy = u_vector[1] as f32;
        let vx = v_vector[0] as f32;
        let vy = v_vector[1] as f32;
        let ix = insertion[0] as f32 + ox;
        let iy = insertion[1] as f32 + oy;
        let e = ix + vx * h_px;
        let f = iy + vy * h_px;

        let _ = write!(
            svg,
            "<image transform=\"matrix({a},{b},{c},{d},{e},{f})\" width=\"{w}\" height=\"{h}\" href=\"{href}\" preserveAspectRatio=\"none\" />\n",
            a = fmt_f32(ux),
            b = fmt_f32(uy),
            c = fmt_f32(-vx),
            d = fmt_f32(-vy),
            e = fmt_f32(e),
            f = fmt_f32(f),
            w = fmt_f32(w_px),
            h = fmt_f32(h_px),
            href = xml_attr_escape(&href),
        );
    }
}

/// Build the `href` value for an IMAGE entity.  Returns `None` if embedding
/// is requested but the file cannot be read.
fn image_href(file_path: &str, options: &SvgExportOptions) -> Option<String> {
    if file_path.is_empty() {
        return None;
    }
    let resolved = resolve_image_path(file_path, options.image_base.as_deref());
    if options.embed_images {
        let bytes = std::fs::read(&resolved).ok()?;
        let mime = guess_image_mime(&resolved).unwrap_or("application/octet-stream");
        let encoded = base64_encode(&bytes);
        Some(format!("data:{mime};base64,{encoded}"))
    } else {
        let name = resolved
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_path.to_string());
        Some(format!("{}{name}", options.image_url_prefix))
    }
}

/// Resolve a possibly-relative raster file path against the configured
/// `image_base` directory (matches ODA's `ImageBase` device property).
fn resolve_image_path(file_path: &str, base: Option<&Path>) -> std::path::PathBuf {
    let candidate = std::path::Path::new(file_path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    if let Some(base) = base {
        return base.join(candidate);
    }
    candidate.to_path_buf()
}

/// Guess a MIME type suitable for a `data:` URI based on the file extension.
/// Returns `None` for unknown extensions so callers can fall back to a safe
/// default.
fn guess_image_mime(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())?;
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// RFC 4648 base64 (standard alphabet, `=`-padded).  Hand-rolled so the
/// SVG exporter stays free of extra dependencies even though `base64` is
/// already transitively resolved in `Cargo.lock`.
fn base64_encode(bytes: &[u8]) -> String {
    const TAB: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    let full_chunks = bytes.len() / 3;
    for chunk in 0..full_chunks {
        let i = chunk * 3;
        let n = ((bytes[i] as u32) << 16)
            | ((bytes[i + 1] as u32) << 8)
            | (bytes[i + 2] as u32);
        out.push(TAB[((n >> 18) & 0x3F) as usize] as char);
        out.push(TAB[((n >> 12) & 0x3F) as usize] as char);
        out.push(TAB[((n >> 6) & 0x3F) as usize] as char);
        out.push(TAB[(n & 0x3F) as usize] as char);
    }
    let rem = bytes.len() - full_chunks * 3;
    let tail_i = full_chunks * 3;
    match rem {
        1 => {
            let n = (bytes[tail_i] as u32) << 16;
            out.push(TAB[((n >> 18) & 0x3F) as usize] as char);
            out.push(TAB[((n >> 12) & 0x3F) as usize] as char);
            out.push_str("==");
        }
        2 => {
            let n = ((bytes[tail_i] as u32) << 16) | ((bytes[tail_i + 1] as u32) << 8);
            out.push(TAB[((n >> 18) & 0x3F) as usize] as char);
            out.push(TAB[((n >> 12) & 0x3F) as usize] as char);
            out.push(TAB[((n >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

/// Escape a string for use in an XML attribute value (adds `"` handling on
/// top of the element-content rules used by `xml_escape`).  `href` values
/// may legitimately contain `&` characters from the base64 payload, so we
/// always run the escape even for data URIs to keep the SVG well-formed.
fn xml_attr_escape(s: &str) -> String {
    xml_escape(s)
}

// ── Block / Insert defs+use collection (Batch E) ───────────────────────────

/// An Insert that is eligible to be emitted as a `<use>` reference pointing
/// to a shared `<g>` in `<defs>`.  See `collect_eligible_inserts`.
#[derive(Clone, Debug)]
struct EligibleInsert {
    /// Handle value of the Insert entity (decimal string).
    insert_handle: String,
    /// Handle value of the referenced BlockRecord (decimal string).
    /// Used as the SVG element id suffix (`blk_<value>`).
    block_id: String,
    /// Block base point (codes 10/20/30 on the BLOCK entity).
    base_point: [f64; 3],
    /// Insert insertion point (world coords).
    insertion: [f64; 3],
    /// Per-axis scale factors (x, y, z).
    scale: [f64; 3],
    /// Rotation in degrees around the Z axis (CAD convention).
    rotation_deg: f64,
    /// Owned snapshot of the block's child entities — only primitives that
    /// passed the eligibility filter are retained.
    primitives: Vec<BlockPrimitive>,
}

/// Per-vertex record used by `BlockPrimitive::Polyline`.  Bulges are retained
/// so polylines with curved segments (Phase 4 T2) can round-trip through the
/// defs/use path as native `<path d="... A ...">` commands.
#[derive(Clone, Copy, Debug)]
struct PolylineVertex {
    x: f64,
    y: f64,
    /// DXF bulge = tan(θ/4), where θ is the included angle of the arc
    /// segment that starts at this vertex.  `0.0` means a straight line
    /// segment to the next vertex.
    bulge: f64,
}

/// A simple SVG-renderable primitive extracted from a BlockRecord.  Only
/// geometry that maps 1:1 to SVG shapes is retained; anything else disqualifies
/// the block from defs/use emission.
#[derive(Clone, Debug)]
enum BlockPrimitive {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Circle {
        center: [f64; 3],
        radius: f64,
    },
    Arc {
        center: [f64; 3],
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    /// LwPolyline/Polyline flattened to per-vertex records and a closed flag.
    /// `bulge != 0` segments render as SVG `A` commands so curved polylines
    /// stay faithful to the CAD source.
    Polyline {
        vertices: Vec<PolylineVertex>,
        closed: bool,
    },
    /// Phase 4 T3: Ellipses inside blocks reuse the same emission path as
    /// top-level ellipses.  Keeping the definition close to `nm::EntityData`
    /// avoids a separate conversion step when building `<defs>` entries.
    Ellipse {
        center: [f64; 3],
        major_axis: [f64; 3],
        ratio: f64,
        start_param: f64,
        end_param: f64,
    },
}

/// Return every Insert whose referenced block contains ONLY primitives we can
/// render as SVG directly.  Blocks with Text/MText/Hatch/Dimension/nested
/// Insert/Spline/etc. are skipped so the wire pipeline keeps handling them.
fn collect_eligible_inserts(doc: &nm::CadDocument) -> Vec<EligibleInsert> {
    let mut out = Vec::new();
    for entity in &doc.entities {
        if entity.invisible {
            continue;
        }
        let nm::EntityData::Insert {
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
            ..
        } = &entity.data
        else {
            continue;
        };
        // ATTRIB-bearing inserts need per-instance text; skip defs/use path.
        if *has_attribs || !attribs.is_empty() {
            continue;
        }
        let Some(block) = doc.resolve_insert_block(entity) else {
            continue;
        };
        let Some(prims) = extract_block_primitives(block) else {
            continue;
        };
        out.push(EligibleInsert {
            insert_handle: entity.handle.value().to_string(),
            block_id: block.handle.value().to_string(),
            base_point: block.base_point,
            insertion: *insertion,
            scale: *scale,
            rotation_deg: *rotation,
            primitives: prims,
        });
    }
    out
}

/// Try to convert a BlockRecord's children into `BlockPrimitive`s.  Returns
/// `None` if ANY child entity is not a supported primitive — the block then
/// falls back to wire rendering.
fn extract_block_primitives(block: &nm::BlockRecord) -> Option<Vec<BlockPrimitive>> {
    let mut prims = Vec::with_capacity(block.entities.len());
    for child in &block.entities {
        if child.invisible {
            continue;
        }
        match &child.data {
            nm::EntityData::Line { start, end } => {
                prims.push(BlockPrimitive::Line {
                    start: *start,
                    end: *end,
                });
            }
            nm::EntityData::Circle { center, radius } => {
                prims.push(BlockPrimitive::Circle {
                    center: *center,
                    radius: *radius,
                });
            }
            nm::EntityData::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                prims.push(BlockPrimitive::Arc {
                    center: *center,
                    radius: *radius,
                    start_angle_deg: *start_angle,
                    end_angle_deg: *end_angle,
                });
            }
            nm::EntityData::LwPolyline {
                vertices, closed, ..
            } => {
                // Phase 4 T2: retain bulge values so curved polylines stay in
                // the defs/use path as native `<path>` data.
                let verts = vertices
                    .iter()
                    .map(|v| PolylineVertex {
                        x: v.x,
                        y: v.y,
                        bulge: v.bulge,
                    })
                    .collect();
                prims.push(BlockPrimitive::Polyline {
                    vertices: verts,
                    closed: *closed,
                });
            }
            nm::EntityData::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                prims.push(BlockPrimitive::Ellipse {
                    center: *center,
                    major_axis: *major_axis,
                    ratio: *ratio,
                    start_param: *start_param,
                    end_param: *end_param,
                });
            }
            // Anything else (Text, MText, Hatch, Insert, Dimension, Spline,
            // Polyline3D, Face3D, Solid, …) disqualifies the block.
            _ => return None,
        }
    }
    Some(prims)
}

/// Emit `<defs>` containing one `<g id="blk_<handle>">` per referenced block.
/// Content is authored in the block's LOCAL coordinate frame; the `<use>`
/// transform applies `translate(insertion) rotate(r) scale(sx,sy) translate(-base)`.
fn emit_block_defs(svg: &mut String, eligible: &[EligibleInsert], options: &SvgExportOptions) {
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    svg.push_str("<defs>\n");
    for ins in eligible {
        if !emitted.insert(ins.block_id.as_str()) {
            continue;
        }
        let _ = write!(svg, "<g id=\"blk_{id}\">\n", id = ins.block_id);
        for prim in &ins.primitives {
            emit_block_primitive(svg, prim, options);
        }
        svg.push_str("</g>\n");
    }
    svg.push_str("</defs>\n");
}

/// Emit one `<use>` per eligible Insert, positioned at the insert's world-space
/// insertion point (plus the scene offset `ox, oy`).
fn emit_insert_uses(
    svg: &mut String,
    eligible: &[EligibleInsert],
    ox: f32,
    oy: f32,
    _options: &SvgExportOptions,
) {
    for ins in eligible {
        let ix = ins.insertion[0] as f32 + ox;
        let iy = ins.insertion[1] as f32 + oy;
        let sx = ins.scale[0] as f32;
        let sy = ins.scale[1] as f32;
        let bx = ins.base_point[0] as f32;
        let by = ins.base_point[1] as f32;
        let r = ins.rotation_deg as f32;
        // Transform composition (applied right-to-left in SVG):
        //   1. translate(-base)  — move primitives to block origin
        //   2. scale(sx, sy)     — apply per-axis scaling
        //   3. rotate(r)         — rotate about (0,0) after scaling
        //   4. translate(ix, iy) — drop into world position
        let _ = write!(
            svg,
            "<use href=\"#blk_{id}\" transform=\"translate({ix},{iy}) rotate({r}) scale({sx},{sy}) translate({nbx},{nby})\" />\n",
            id = ins.block_id,
            ix = fmt_f32(ix),
            iy = fmt_f32(iy),
            r = fmt_f32(r),
            sx = fmt_f32(sx),
            sy = fmt_f32(sy),
            nbx = fmt_f32(-bx),
            nby = fmt_f32(-by),
        );
    }
}

/// Render one block-local primitive as SVG.  All geometry is in the block's
/// own coordinate system — the containing `<use>` applies the instance
/// transform when the symbol is referenced.
fn emit_block_primitive(svg: &mut String, prim: &BlockPrimitive, options: &SvgExportOptions) {
    // Blocks in defs inherit stroke via the enclosing `<g>` style.  For a
    // simple baseline we paint them black (or the user-selected hint).
    let stroke = if options.monochrome { "black" } else { "black" };
    let lw = options.min_stroke_width.max(0.01);
    match prim {
        BlockPrimitive::Line { start, end } => {
            let _ = write!(
                svg,
                "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
                x1 = fmt_f32(start[0] as f32),
                y1 = fmt_f32(start[1] as f32),
                x2 = fmt_f32(end[0] as f32),
                y2 = fmt_f32(end[1] as f32),
                s = stroke,
                w = fmt_f32(lw),
            );
        }
        BlockPrimitive::Circle { center, radius } => {
            let _ = write!(
                svg,
                "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
                cx = fmt_f32(center[0] as f32),
                cy = fmt_f32(center[1] as f32),
                r = fmt_f32(*radius as f32),
                s = stroke,
                w = fmt_f32(lw),
            );
        }
        BlockPrimitive::Arc {
            center,
            radius,
            start_angle_deg,
            end_angle_deg,
        } => {
            // Build an SVG arc path.  Angles are CAD CCW from +X.
            let a0 = (*start_angle_deg).to_radians();
            let mut a1 = (*end_angle_deg).to_radians();
            if a1 <= a0 {
                a1 += std::f64::consts::TAU;
            }
            let sweep = a1 - a0;
            let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
            let sx = center[0] + radius * a0.cos();
            let sy = center[1] + radius * a0.sin();
            let ex = center[0] + radius * a1.cos();
            let ey = center[1] + radius * a1.sin();
            let _ = write!(
                svg,
                "<path d=\"M {sx} {sy} A {r} {r} 0 {l} 1 {ex} {ey}\" fill=\"none\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
                sx = fmt_f32(sx as f32),
                sy = fmt_f32(sy as f32),
                r = fmt_f32(*radius as f32),
                l = large,
                ex = fmt_f32(ex as f32),
                ey = fmt_f32(ey as f32),
                s = stroke,
                w = fmt_f32(lw),
            );
        }
        BlockPrimitive::Polyline { vertices, closed } => {
            emit_polyline_path(svg, vertices, *closed, stroke, lw);
        }
        BlockPrimitive::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            emit_ellipse(
                svg,
                [center[0], center[1]],
                *major_axis,
                *ratio,
                *start_param,
                *end_param,
                stroke,
                lw,
            );
        }
    }
}

/// Emit a (possibly bulged, possibly closed) polyline as a single SVG
/// `<path>` element.  Straight-line segments become `L` commands; segments
/// whose starting vertex has `bulge != 0` become circular-arc `A` commands
/// derived from the DXF bulge encoding `b = tan(θ/4)`.  Degenerate runs
/// (fewer than two vertices) are silently dropped.
fn emit_polyline_path(
    svg: &mut String,
    vertices: &[PolylineVertex],
    closed: bool,
    stroke: &str,
    lw: f32,
) {
    if vertices.len() < 2 {
        return;
    }
    let has_bulge = vertices.iter().any(|v| v.bulge.abs() > 1e-9);
    let fill = "none";

    if !has_bulge {
        // Fast path — no curves, use <polyline>/<polygon> for maximum
        // compactness (matches the previous behaviour for straight runs).
        let tag = if closed { "polygon" } else { "polyline" };
        let _ = write!(
            svg,
            "<{tag} fill=\"{fill}\" stroke=\"{s}\" stroke-width=\"{w}\" points=\"",
            tag = tag,
            fill = fill,
            s = stroke,
            w = fmt_f32(lw),
        );
        for (i, v) in vertices.iter().enumerate() {
            if i > 0 {
                svg.push(' ');
            }
            svg.push_str(&fmt_f32(v.x as f32));
            svg.push(',');
            svg.push_str(&fmt_f32(v.y as f32));
        }
        let _ = write!(svg, "\" />\n");
        return;
    }

    // Bulged: build a `<path d=...>` with M / L / A commands.  Segment i
    // connects vertices[i] → vertices[(i+1) % n] and inherits the bulge
    // stored on vertices[i].
    let _ = write!(
        svg,
        "<path d=\"M {sx} {sy}",
        sx = fmt_f32(vertices[0].x as f32),
        sy = fmt_f32(vertices[0].y as f32),
    );
    let n = vertices.len();
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments {
        let v0 = &vertices[i];
        let v1 = &vertices[(i + 1) % n];
        let b = v0.bulge;
        if b.abs() < 1e-9 {
            let _ = write!(
                svg,
                " L {x} {y}",
                x = fmt_f32(v1.x as f32),
                y = fmt_f32(v1.y as f32),
            );
        } else {
            // θ = 4·atan(b).  Chord length c; radius r = c / (2·|sin(θ/2)|).
            // `large-arc-flag` = 1 when |θ| > π (ie |b| > 1).
            // `sweep-flag`     = 1 when b > 0 (CCW in CAD → same visual sense
            // inside the outer scale(1,-1) group — confirmed by the existing
            // `BlockPrimitive::Arc` tests).
            let theta = 4.0 * b.atan();
            let dx = v1.x - v0.x;
            let dy = v1.y - v0.y;
            let chord = (dx * dx + dy * dy).sqrt();
            let sin_half = (theta * 0.5).sin().abs();
            let r = if sin_half > 1e-12 {
                chord / (2.0 * sin_half)
            } else {
                // Degenerate (θ≈0 after the early-out guard) — fall back to L.
                let _ = write!(
                    svg,
                    " L {x} {y}",
                    x = fmt_f32(v1.x as f32),
                    y = fmt_f32(v1.y as f32),
                );
                continue;
            };
            let large = if b.abs() > 1.0 { 1 } else { 0 };
            let sweep = if b > 0.0 { 1 } else { 0 };
            let _ = write!(
                svg,
                " A {r} {r} 0 {l} {sw} {x} {y}",
                r = fmt_f32(r as f32),
                l = large,
                sw = sweep,
                x = fmt_f32(v1.x as f32),
                y = fmt_f32(v1.y as f32),
            );
        }
    }
    if closed {
        svg.push_str(" Z");
    }
    let _ = write!(
        svg,
        "\" fill=\"{fill}\" stroke=\"{s}\" stroke-width=\"{w}\" />\n",
        fill = fill,
        s = stroke,
        w = fmt_f32(lw),
    );
}

// ── Entity color resolution ─────────────────────────────────────────────────

fn resolve_entity_fill(entity: &nm::Entity, options: &SvgExportOptions) -> String {
    if options.monochrome {
        return "rgb(0,0,0)".to_string();
    }
    // True color takes precedence.
    if entity.true_color != 0 {
        let tc = entity.true_color;
        let r = ((tc >> 16) & 0xFF) as u8;
        let g = ((tc >> 8) & 0xFF) as u8;
        let b = (tc & 0xFF) as u8;
        return format!("rgb({r},{g},{b})");
    }
    // ACI color index: map common indices to RGB.
    let aci = entity.color_index;
    let (r, g, b) = match aci {
        1 => (255, 0, 0),       // red
        2 => (255, 255, 0),     // yellow
        3 => (0, 255, 0),       // green
        4 => (0, 255, 255),     // cyan
        5 => (0, 0, 255),       // blue
        6 => (255, 0, 255),     // magenta
        7 | 0 => (0, 0, 0),     // white/ByBlock → black on white paper
        _ => (0, 0, 0),         // default to black
    };
    format!("rgb({r},{g},{b})")
}

// ── Text emission (from native model) ──────────────────────────────────────

fn emit_text_entities(
    svg: &mut String,
    doc: &nm::CadDocument,
    ox: f32,
    oy: f32,
    options: &SvgExportOptions,
) {
    let frozen_layers: std::collections::HashSet<&str> = doc
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

        match &entity.data {
            nm::EntityData::Text {
                insertion,
                height,
                value,
                rotation,
                ..
            } => {
                let x = insertion[0] as f32 + ox;
                let y = insertion[1] as f32 + oy;
                let fs = (*height as f32) * options.font_size_scale;
                let fill = resolve_entity_fill(entity, options);
                emit_text_element(svg, x, y, fs, *rotation as f32, value, &fill, options);
            }
            nm::EntityData::MText {
                insertion,
                height,
                value,
                rotation,
                ..
            } => {
                let x = insertion[0] as f32 + ox;
                let y = insertion[1] as f32 + oy;
                let fs = (*height as f32) * options.font_size_scale;
                let fill = resolve_entity_fill(entity, options);
                let clean = strip_mtext_codes(value);
                let lines: Vec<&str> = clean.split('\n').collect();
                if lines.len() <= 1 {
                    emit_text_element(svg, x, y, fs, *rotation as f32, &clean, &fill, options);
                } else {
                    emit_multiline_text(svg, x, y, fs, *rotation as f32, &lines, &fill, options);
                }
            }
            _ => {}
        }
    }
}

fn emit_text_element(
    svg: &mut String,
    x: f32,
    y: f32,
    font_size: f32,
    rotation_deg: f32,
    text: &str,
    fill: &str,
    options: &SvgExportOptions,
) {
    if text.is_empty() || font_size < 0.001 {
        return;
    }
    // The global <g> applies scale(1,-1) which mirrors text.
    // Counter-flip: translate to text origin, scale(1,-1), then rotate.
    svg.push_str("<text x=\"");
    svg.push_str(&fmt_f32(x));
    svg.push_str("\" y=\"");
    svg.push_str(&fmt_f32(y));
    svg.push_str("\" font-size=\"");
    svg.push_str(&fmt_f32(font_size));
    svg.push_str("\" font-family=\"");
    svg.push_str(&options.font_family);
    svg.push_str("\" fill=\"");
    svg.push_str(fill);
    svg.push('"');

    // Build transform: translate to origin, flip Y back, rotate, translate back.
    if rotation_deg.abs() > 0.01 {
        let _ = write!(
            svg,
            " transform=\"translate({x},{y}) scale(1,-1) rotate({r}) translate({nx},{ny})\"",
            x = fmt_f32(x),
            y = fmt_f32(y),
            r = fmt_f32(rotation_deg),
            nx = fmt_f32(-x),
            ny = fmt_f32(-y),
        );
    } else {
        let _ = write!(
            svg,
            " transform=\"translate({x},{y}) scale(1,-1) translate({nx},{ny})\"",
            x = fmt_f32(x),
            y = fmt_f32(y),
            nx = fmt_f32(-x),
            ny = fmt_f32(-y),
        );
    }

    svg.push('>');
    svg.push_str(&xml_escape(text));
    svg.push_str("</text>\n");
}

fn emit_multiline_text(
    svg: &mut String,
    x: f32,
    y: f32,
    font_size: f32,
    rotation_deg: f32,
    lines: &[&str],
    fill: &str,
    options: &SvgExportOptions,
) {
    svg.push_str("<text x=\"");
    svg.push_str(&fmt_f32(x));
    svg.push_str("\" y=\"");
    svg.push_str(&fmt_f32(y));
    svg.push_str("\" font-size=\"");
    svg.push_str(&fmt_f32(font_size));
    svg.push_str("\" font-family=\"");
    svg.push_str(&options.font_family);
    svg.push_str("\" fill=\"");
    svg.push_str(fill);
    svg.push('"');

    // Counter-flip text from the global Y-flip transform.
    if rotation_deg.abs() > 0.01 {
        let _ = write!(
            svg,
            " transform=\"translate({x},{y}) scale(1,-1) rotate({r}) translate({nx},{ny})\"",
            x = fmt_f32(x),
            y = fmt_f32(y),
            r = fmt_f32(rotation_deg),
            nx = fmt_f32(-x),
            ny = fmt_f32(-y),
        );
    } else {
        let _ = write!(
            svg,
            " transform=\"translate({x},{y}) scale(1,-1) translate({nx},{ny})\"",
            x = fmt_f32(x),
            y = fmt_f32(y),
            nx = fmt_f32(-x),
            ny = fmt_f32(-y),
        );
    }
    svg.push('>');

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            svg.push_str(&xml_escape(line));
        } else {
            let _ = write!(
                svg,
                "<tspan x=\"{x}\" dy=\"{dy}\">",
                x = fmt_f32(x),
                dy = fmt_f32(font_size * 1.2),
            );
            svg.push_str(&xml_escape(line));
            svg.push_str("</tspan>");
        }
    }
    svg.push_str("</text>\n");
}

// ── MText control code parser ──────────────────────────────────────────────

/// Strip MText formatting codes and return plain text with `\n` for line breaks.
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
                    // Skip until ';' (parameter terminator).
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch == ';' {
                            break;
                        }
                    }
                }
                Some('S') | Some('s') => {
                    // Stacking: \Snum1^num2; → "num1/num2"
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
                _ => {
                    out.push('\\');
                }
            }
        } else if c == '{' || c == '}' {
            // Grouping braces: skip.
        } else {
            out.push(c);
        }
    }
    out
}

// ── Shared helpers ─────────────────────────────────────────────────────────

fn build_transform(paper_w: f32, paper_h: f32, rotation_deg: i32) -> String {
    match rotation_deg {
        90 => format!(
            "translate(0,{ph}) scale(1,-1) translate(0,{ph}) rotate(-90,0,0)",
            ph = fmt_f32(paper_h),
        ),
        180 => format!(
            "translate(0,{ph}) scale(1,-1) translate({pw},{ph}) rotate(180,0,0)",
            ph = fmt_f32(paper_h),
            pw = fmt_f32(paper_w),
        ),
        270 => format!(
            "translate(0,{ph}) scale(1,-1) translate({pw},0) rotate(-270,0,0)",
            ph = fmt_f32(paper_h),
            pw = fmt_f32(paper_w),
        ),
        _ => format!("translate(0,{ph}) scale(1,-1)", ph = fmt_f32(paper_h)),
    }
}

fn flush_polyline(svg: &mut String, pts: &[[f32; 2]], stroke: &str, lw: f32, dasharray: &str) {
    if pts.len() < 2 {
        return;
    }
    svg.push_str("<polyline fill=\"none\" stroke=\"");
    svg.push_str(stroke);
    svg.push_str("\" stroke-width=\"");
    svg.push_str(&fmt_f32(lw));
    svg.push('"');
    if !dasharray.is_empty() {
        svg.push_str(" stroke-dasharray=\"");
        svg.push_str(dasharray);
        svg.push('"');
    }
    svg.push_str(" points=\"");
    for (i, &[x, y]) in pts.iter().enumerate() {
        if i > 0 {
            svg.push(' ');
        }
        svg.push_str(&fmt_f32(x));
        svg.push(',');
        svg.push_str(&fmt_f32(y));
    }
    svg.push_str("\" />\n");
}

fn build_dasharray(pattern: &[f32; 8]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for &val in pattern.iter() {
        if val == 0.0 {
            break;
        }
        parts.push(fmt_f32(val.abs()));
    }
    if parts.is_empty() {
        return String::new();
    }
    parts.join(",")
}

fn fmt_f32(v: f32) -> String {
    // Normalise IEEE 754 -0.0 → 0.0 so matrix(...) transforms don't emit
    // "-0" which confuses downstream renderers and trips exact-match tests.
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::hatch_model::HatchModel;

    // ── Pure helpers ────────────────────────────────────────────────────

    #[test]
    fn fmt_f32_trims_trailing_zeros() {
        assert_eq!(fmt_f32(1.0), "1");
        assert_eq!(fmt_f32(1.500), "1.5");
        assert_eq!(fmt_f32(0.0), "0");
        assert_eq!(fmt_f32(3.14159), "3.142");
        assert_eq!(fmt_f32(-2.50), "-2.5");
    }

    #[test]
    fn xml_escape_covers_all_entities() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<x>"), "&lt;x&gt;");
        assert_eq!(xml_escape("\"q\""), "&quot;q&quot;");
        assert_eq!(xml_escape("it's"), "it&apos;s");
        assert_eq!(xml_escape("plain"), "plain");
    }

    #[test]
    fn build_dasharray_stops_at_zero_sentinel() {
        // Pattern [5, -2, 1, -2, 0, 0, 0, 0] → "5,2,1,2"
        let pat = [5.0, -2.0, 1.0, -2.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(build_dasharray(&pat), "5,2,1,2");

        let empty = [0.0_f32; 8];
        assert_eq!(build_dasharray(&empty), "");
    }

    #[test]
    fn build_transform_handles_all_rotations() {
        // Baseline Y-flip only.
        assert_eq!(
            build_transform(100.0, 50.0, 0),
            "translate(0,50) scale(1,-1)",
        );
        // 90° → additional translate + rotate(-90).
        let t90 = build_transform(100.0, 50.0, 90);
        assert!(t90.contains("rotate(-90"));
        assert!(t90.contains("scale(1,-1)"));
        // 180° → rotate(180).
        assert!(build_transform(100.0, 50.0, 180).contains("rotate(180"));
        // 270° → rotate(-270).
        assert!(build_transform(100.0, 50.0, 270).contains("rotate(-270"));
    }

    // ── MText control-code parser ─────────────────────────────────────────

    #[test]
    fn strip_mtext_linebreak_and_font() {
        assert_eq!(strip_mtext_codes("Line1\\PLine2"), "Line1\nLine2");
        assert_eq!(
            strip_mtext_codes("\\fArial|b0|i0;Hello"),
            "Hello",
            "\\f font code should be stripped through ';'",
        );
    }

    #[test]
    fn strip_mtext_height_color_and_stacking() {
        assert_eq!(strip_mtext_codes("\\H2.5;Tall"), "Tall");
        assert_eq!(strip_mtext_codes("\\C1;Red"), "Red");
        // Stacking \S a^b; → a/b
        assert_eq!(strip_mtext_codes("\\S1^2;"), "1/2");
    }

    #[test]
    fn strip_mtext_escapes_and_braces() {
        // Literal backslash: \\ → \
        assert_eq!(strip_mtext_codes("a\\\\b"), "a\\b");
        // Escaped braces \{ \}
        assert_eq!(strip_mtext_codes("\\{x\\}"), "{x}");
        // Grouping braces {} are stripped
        assert_eq!(strip_mtext_codes("{grp}"), "grp");
    }

    // ── Full SVG builder smoke tests ──────────────────────────────────────

    fn make_wire(name: &str, pts: Vec<[f32; 3]>, aci: u8) -> WireModel {
        WireModel {
            name: name.into(),
            points: pts,
            color: [0.0, 0.0, 0.0, 1.0],
            selected: false,
            pattern_length: 0.0,
            pattern: [0.0; 8],
            line_weight_px: 1.0,
            aci,
            snap_pts: Vec::new(),
            tangent_geoms: Vec::new(),
            key_vertices: Vec::new(),
        }
    }

    fn default_opts() -> SvgExportOptions {
        SvgExportOptions::default()
    }

    #[test]
    fn empty_inputs_emit_valid_svg_skeleton() {
        let wires: Vec<WireModel> = Vec::new();
        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let svg = build_svg_full(
            &wires, &hatches, None, 210.0, 297.0, 0.0, 0.0, 0, None, &default_opts(),
        );
        assert!(svg.starts_with("<?xml version=\"1.0\""));
        assert!(svg.contains("viewBox=\"0 0 210 297\""));
        assert!(svg.contains("fill=\"white\""));
        // Y-flip transform present.
        assert!(svg.contains("scale(1,-1)"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn single_line_wire_becomes_polyline() {
        let wires = vec![make_wire("100", vec![[0.0, 0.0, 0.0], [10.0, 20.0, 0.0]], 7)];
        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let svg = build_svg_full(
            &wires, &hatches, None, 50.0, 50.0, 0.0, 0.0, 0, None, &default_opts(),
        );
        assert!(svg.contains("<polyline"));
        assert!(svg.contains("0,0"));
        assert!(svg.contains("10,20"));
    }

    #[test]
    fn paper_boundary_wire_is_skipped() {
        let wires = vec![
            make_wire("100", vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]], 7),
            make_wire(
                "__paper_boundary__",
                vec![[0.0, 0.0, 0.0], [100.0, 100.0, 0.0]],
                7,
            ),
        ];
        let hatches: HashMap<Handle, HatchModel> = HashMap::new();
        let svg = build_svg_full(
            &wires, &hatches, None, 50.0, 50.0, 0.0, 0.0, 0, None, &default_opts(),
        );
        // Only one polyline should be emitted.
        assert_eq!(svg.matches("<polyline").count(), 1);
    }

    #[test]
    fn solid_hatch_emits_polygon() {
        use crate::scene::hatch_model::HatchPattern;
        let hatch = HatchModel {
            boundary: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            pattern: HatchPattern::Solid,
            name: "SOLID".into(),
            color: [1.0, 0.0, 0.0, 1.0],
            angle_offset: 0.0,
            scale: 1.0,
        };
        let mut hatches = HashMap::new();
        hatches.insert(Handle::new(42), hatch);

        let wires: Vec<WireModel> = Vec::new();
        let svg = build_svg_full(
            &wires, &hatches, None, 50.0, 50.0, 0.0, 0.0, 0, None, &default_opts(),
        );
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("0,0 10,0 10,10 0,10"));
    }

    #[test]
    fn monochrome_hatch_forces_black_fill() {
        use crate::scene::hatch_model::HatchPattern;
        let hatch = HatchModel {
            boundary: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            pattern: HatchPattern::Solid,
            name: "SOLID".into(),
            color: [1.0, 0.5, 0.25, 1.0], // orange
            angle_offset: 0.0,
            scale: 1.0,
        };
        let mut hatches = HashMap::new();
        hatches.insert(Handle::new(1), hatch);

        let mono = SvgExportOptions {
            monochrome: true,
            ..default_opts()
        };
        let svg_mono = build_svg_full(
            &Vec::<WireModel>::new(),
            &hatches,
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &mono,
        );
        assert!(svg_mono.contains("rgb(0,0,0)"));
        assert!(!svg_mono.contains("rgb(255,127,63)"));

        let color = SvgExportOptions {
            monochrome: false,
            ..default_opts()
        };
        let svg_color = build_svg_full(
            &Vec::<WireModel>::new(),
            &hatches,
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &color,
        );
        // 1.0*255=255, 0.5*255=127, 0.25*255=63
        assert!(svg_color.contains("rgb(255,127,63)"));
    }

    #[test]
    fn native_text_entity_emits_counter_flipped_text() {
        let mut doc = nm::CadDocument::new();
        let mut entity = nm::Entity::new(nm::EntityData::Text {
            insertion: [5.0, 10.0, 0.0],
            height: 2.5,
            value: "Hi".into(),
            rotation: 0.0,
            style_name: "Standard".into(),
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            alignment_point: None,
        });
        entity.handle = nm::Handle::new(7);
        entity.layer_name = "0".into();
        doc.entities.push(entity);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<text"));
        assert!(svg.contains(">Hi</text>"));
        // Text must carry counter-flip scale(1,-1).
        let text_elem = svg
            .split("<text")
            .nth(1)
            .and_then(|s| s.split("</text>").next())
            .expect("text element should exist");
        assert!(
            text_elem.contains("scale(1,-1)"),
            "text element missing counter-flip: {text_elem}",
        );
    }

    #[test]
    fn text_handle_wire_is_deduplicated() {
        let mut doc = nm::CadDocument::new();
        let mut entity = nm::Entity::new(nm::EntityData::Text {
            insertion: [0.0, 0.0, 0.0],
            height: 1.0,
            value: "X".into(),
            rotation: 0.0,
            style_name: "Standard".into(),
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            alignment_point: None,
        });
        entity.handle = nm::Handle::new(77);
        doc.entities.push(entity);

        // A wire whose name matches the text entity's handle must be dropped
        // (text is already emitted as a native <text> element).
        let wires = vec![make_wire("77", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("<polyline"), "text wire should be skipped: {svg}");
        assert!(svg.contains("<text"));
    }

    #[test]
    fn text_as_geometry_keeps_wire_and_skips_native_text() {
        let mut doc = nm::CadDocument::new();
        let mut entity = nm::Entity::new(nm::EntityData::Text {
            insertion: [0.0, 0.0, 0.0],
            height: 1.0,
            value: "keep".into(),
            rotation: 0.0,
            style_name: "Standard".into(),
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            alignment_point: None,
        });
        entity.handle = nm::Handle::new(99);
        doc.entities.push(entity);

        let wires = vec![make_wire("99", vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]], 7)];
        let opts = SvgExportOptions {
            text_as_geometry: true,
            ..default_opts()
        };
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(svg.contains("<polyline"), "wire kept as geometry: {svg}");
        assert!(!svg.contains("<text"), "native <text> suppressed in geometry mode: {svg}");
    }

    #[test]
    fn monochrome_wire_is_black_even_when_aci_red() {
        let mut w = make_wire("1", vec![[0.0, 0.0, 0.0], [5.0, 5.0, 0.0]], 1);
        w.color = [1.0, 0.0, 0.0, 1.0]; // red
        let svg = build_svg_full(
            &vec![w],
            &HashMap::new(),
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(), // monochrome=true by default
        );
        assert!(svg.contains("rgb(0,0,0)"));
        assert!(!svg.contains("rgb(255,0,0)"));
    }

    #[test]
    fn color_mode_preserves_wire_color() {
        let mut w = make_wire("1", vec![[0.0, 0.0, 0.0], [5.0, 5.0, 0.0]], 1);
        w.color = [1.0, 0.0, 0.0, 1.0];
        let opts = SvgExportOptions {
            monochrome: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &vec![w],
            &HashMap::new(),
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(svg.contains("rgb(255,0,0)"));
    }

    #[test]
    fn nan_in_wire_points_splits_polyline() {
        let wires = vec![make_wire(
            "1",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [f32::NAN, f32::NAN, 0.0],
                [5.0, 5.0, 0.0],
                [6.0, 6.0, 0.0],
            ],
            7,
        )];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // NaN breaks the path: expect TWO polyline elements.
        assert_eq!(svg.matches("<polyline").count(), 2);
    }

    // ── Batch E: Block defs/use ────────────────────────────────────────────

    fn make_doc_with_line_block(
        block_name: &str,
        block_handle: u64,
        line_endpoints: ([f64; 3], [f64; 3]),
        insert_handle: u64,
        insertion: [f64; 3],
        scale: [f64; 3],
        rotation: f64,
    ) -> nm::CadDocument {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(block_handle);
        let mut br = nm::BlockRecord::new(bh, block_name);
        br.base_point = [0.0, 0.0, 0.0];
        let mut line = nm::Entity::new(nm::EntityData::Line {
            start: line_endpoints.0,
            end: line_endpoints.1,
        });
        line.handle = nm::Handle::new(block_handle + 1);
        line.owner_handle = bh;
        br.entities.push(line);
        doc.insert_block_record(br);

        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: block_name.into(),
            insertion,
            scale,
            rotation,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(insert_handle);
        doc.entities.push(insert);
        doc
    }

    #[test]
    fn eligible_block_emits_defs_and_use() {
        let doc = make_doc_with_line_block(
            "BLK1",
            1000,
            ([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
            100,
            [5.0, 10.0, 0.0],
            [1.0, 1.0, 1.0],
            0.0,
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // <defs> container with the block id.
        assert!(svg.contains("<defs>"), "defs missing: {svg}");
        assert!(svg.contains("id=\"blk_1000\""));
        // Line primitive inside.
        assert!(svg.contains("<line"));
        assert!(svg.contains("x2=\"10\""));
        // <use> referencing the block with the insertion transform.
        assert!(svg.contains("<use href=\"#blk_1000\""));
        assert!(svg.contains("translate(5,10)"));
    }

    #[test]
    fn eligible_insert_handle_is_added_to_skip_set() {
        let doc = make_doc_with_line_block(
            "BLK2",
            2000,
            ([0.0, 0.0, 0.0], [1.0, 1.0, 0.0]),
            200,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            0.0,
        );
        // Wire whose name matches the Insert handle — should NOT appear as
        // polyline because the defs/use path replaces it.
        let wires = vec![make_wire("200", vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("<polyline"));
        assert!(svg.contains("<use href=\"#blk_2000\""));
    }

    #[test]
    fn block_with_text_falls_back_to_wires() {
        // Build a block whose content is a Text entity — disqualifies defs/use.
        let mut doc = nm::CadDocument::new();
        let block_handle = nm::Handle::new(3000);
        let mut br = nm::BlockRecord::new(block_handle, "BLK_WITH_TEXT");
        let mut text = nm::Entity::new(nm::EntityData::Text {
            insertion: [0.0, 0.0, 0.0],
            height: 1.0,
            value: "A".into(),
            rotation: 0.0,
            style_name: "Standard".into(),
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            alignment_point: None,
        });
        text.handle = nm::Handle::new(3001);
        text.owner_handle = block_handle;
        br.entities.push(text);
        doc.insert_block_record(br);

        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_WITH_TEXT".into(),
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(300);
        doc.entities.push(insert);

        // Give the Insert a wire so we can verify the wire pass still emits it.
        let wires = vec![make_wire("300", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];

        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("id=\"blk_3000\""));
        assert!(!svg.contains("<use href=\"#blk_3000\""));
        assert!(svg.contains("<polyline"), "fallback wire must be emitted: {svg}");
    }

    #[test]
    fn two_inserts_share_one_defs_entry() {
        let doc_first = make_doc_with_line_block(
            "SHARED",
            4000,
            ([0.0, 0.0, 0.0], [5.0, 0.0, 0.0]),
            400,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            0.0,
        );
        // Add a second Insert pointing at the same block.
        let mut doc = doc_first;
        let mut insert2 = nm::Entity::new(nm::EntityData::Insert {
            block_name: "SHARED".into(),
            insertion: [20.0, 20.0, 0.0],
            scale: [2.0, 2.0, 2.0],
            rotation: 45.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert2.handle = nm::Handle::new(401);
        doc.entities.push(insert2);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            100.0,
            100.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // ONE defs entry, TWO use references.
        assert_eq!(
            svg.matches("id=\"blk_4000\"").count(),
            1,
            "defs block should be emitted once",
        );
        assert_eq!(
            svg.matches("<use href=\"#blk_4000\"").count(),
            2,
            "two inserts → two use elements",
        );
        // The second instance's rotation and scale must appear.
        assert!(svg.contains("translate(20,20)"));
        assert!(svg.contains("rotate(45)"));
        assert!(svg.contains("scale(2,2)"));
    }

    #[test]
    fn use_block_defs_false_disables_feature() {
        let doc = make_doc_with_line_block(
            "OFF",
            5000,
            ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            500,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            0.0,
        );
        let wires = vec![make_wire("500", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];
        let opts = SvgExportOptions {
            use_block_defs: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(!svg.contains("<defs>"));
        assert!(!svg.contains("<use "));
        // Original wire must survive.
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn arc_in_block_produces_path_d_arc() {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(6000);
        let mut br = nm::BlockRecord::new(bh, "BLK_ARC");
        let mut arc = nm::Entity::new(nm::EntityData::Arc {
            center: [0.0, 0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: 90.0,
        });
        arc.handle = nm::Handle::new(6001);
        arc.owner_handle = bh;
        br.entities.push(arc);
        doc.insert_block_record(br);
        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_ARC".into(),
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(600);
        doc.entities.push(insert);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<path d=\"M 5"), "arc start at (r,0): {svg}");
        assert!(svg.contains("A 5 5 0 0 1"), "arc command present: {svg}");
    }

    #[test]
    fn insert_with_attribs_is_not_deduplicated() {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(7000);
        let mut br = nm::BlockRecord::new(bh, "BLK_ATTR");
        let mut line = nm::Entity::new(nm::EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        });
        line.handle = nm::Handle::new(7001);
        line.owner_handle = bh;
        br.entities.push(line);
        doc.insert_block_record(br);
        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_ATTR".into(),
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: true,  // ← per-instance attribs disable defs/use
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(700);
        doc.entities.push(insert);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("id=\"blk_7000\""));
        assert!(!svg.contains("<use "));
    }

    #[test]
    fn lwpolyline_bulge_enters_defs_use_as_path() {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(8000);
        let mut br = nm::BlockRecord::new(bh, "BLK_BULGE");
        let mut poly = nm::Entity::new(nm::EntityData::LwPolyline {
            // DXF stores bulge on the *starting* vertex of each segment, so
            // v[0].bulge describes the v0 → v1 arc.
            vertices: vec![
                nm::LwVertex {
                    x: 0.0,
                    y: 0.0,
                    bulge: 0.5,
                    start_width: 0.0,
                    end_width: 0.0,
                },
                nm::LwVertex {
                    x: 5.0,
                    y: 5.0,
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                },
            ],
            closed: false,
            constant_width: 0.0,
        });
        poly.handle = nm::Handle::new(8001);
        poly.owner_handle = bh;
        br.entities.push(poly);
        doc.insert_block_record(br);
        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_BULGE".into(),
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(800);
        doc.entities.push(insert);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // Phase 4 T2: bulged polylines now compile into `<path>` with an
        // `A` command instead of disqualifying the block.
        assert!(svg.contains("id=\"blk_8000\""), "block should be emitted: {svg}");
        assert!(svg.contains("<path d=\"M 0 0"));
        assert!(svg.contains(" A "), "bulge must map to an arc command: {svg}");
    }

    // ── S1: Raster image emission ────────────────────────────────────────

    #[test]
    fn base64_encode_rfc4648_vectors() {
        // Standard RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn guess_image_mime_common_extensions() {
        let p = |s: &str| std::path::PathBuf::from(s);
        assert_eq!(guess_image_mime(&p("a.PNG")), Some("image/png"));
        assert_eq!(guess_image_mime(&p("a.jpg")), Some("image/jpeg"));
        assert_eq!(guess_image_mime(&p("a.jpeg")), Some("image/jpeg"));
        assert_eq!(guess_image_mime(&p("a.bmp")), Some("image/bmp"));
        assert_eq!(guess_image_mime(&p("a.tif")), Some("image/tiff"));
        assert_eq!(guess_image_mime(&p("a.webp")), Some("image/webp"));
        assert_eq!(guess_image_mime(&p("a.unknown")), None);
        assert_eq!(guess_image_mime(&p("noext")), None);
    }

    #[test]
    fn resolve_image_path_handles_relative_and_absolute() {
        let base = std::path::Path::new("C:/a/b");
        let rel = resolve_image_path("img.png", Some(base));
        assert_eq!(rel, base.join("img.png"));

        // Absolute path should bypass the base join.
        #[cfg(windows)]
        {
            let abs = resolve_image_path("C:/other/x.png", Some(base));
            assert_eq!(abs, std::path::PathBuf::from("C:/other/x.png"));
        }

        // Without a base, relative paths pass through as-is.
        let plain = resolve_image_path("img.png", None);
        assert_eq!(plain, std::path::PathBuf::from("img.png"));
    }

    /// Minimal 1×1 transparent PNG (89 bytes).  The IHDR is intentionally
    /// hand-crafted so the tests do not depend on a PNG encoder.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // magic
        0x00, 0x00, 0x00, 0x0D, // IHDR length
        0x49, 0x48, 0x44, 0x52, // "IHDR"
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
        0x1F, 0x15, 0xC4, 0x89, // IHDR CRC
        0x00, 0x00, 0x00, 0x0D, // IDAT length
        0x49, 0x44, 0x41, 0x54, // "IDAT"
        0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4,
        0x00, 0x00, 0x00, 0x00, // IDAT CRC slot 1
        0x49, 0x45, 0x4E, 0x44, // "IEND"
        0xAE, 0x42, 0x60, 0x82, // IEND CRC
    ];

    fn write_tiny_png(filename: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("h7cad_svg_export_tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(filename);
        std::fs::write(&path, TINY_PNG).unwrap();
        path
    }

    fn make_image_entity(
        path: &std::path::Path,
        insertion: [f64; 3],
        u_vector: [f64; 3],
        v_vector: [f64; 3],
        size: [f64; 2],
        display_flags: i32,
        handle_value: u64,
    ) -> nm::Entity {
        let mut e = nm::Entity::new(nm::EntityData::Image {
            insertion,
            u_vector,
            v_vector,
            image_size: size,
            image_def_handle: nm::Handle::NULL,
            file_path: path.to_string_lossy().into_owned(),
            display_flags,
        });
        e.handle = nm::Handle::new(handle_value);
        e.layer_name = "0".into();
        e
    }

    #[test]
    fn embedded_image_emits_data_uri_with_png_mime() {
        let path = write_tiny_png("embedded.png");
        let mut doc = nm::CadDocument::new();
        doc.entities.push(make_image_entity(
            &path,
            [1.0, 2.0, 0.0],
            [0.1, 0.0, 0.0], // 0.1 world units per pixel in X
            [0.0, 0.1, 0.0], // 0.1 world units per pixel in Y
            [1.0, 1.0],
            0x1, // SHOW_IMAGE
            42,
        ));

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<image "), "image element should be emitted: {svg}");
        assert!(svg.contains("href=\"data:image/png;base64,"));
        // Transform matrix captures u/v vectors (a,b,c,d,e,f):
        //   a = u.x = 0.1, b = u.y = 0, c = -v.x = 0, d = -v.y = -0.1
        //   e = insertion.x + v.x * h = 1 + 0 * 1 = 1
        //   f = insertion.y + v.y * h = 2 + 0.1 * 1 = 2.1
        assert!(svg.contains("matrix(0.1,0,0,-0.1,1,2.1)"), "transform: {svg}");
        assert!(svg.contains("width=\"1\""));
        assert!(svg.contains("height=\"1\""));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn external_url_prefix_builds_href() {
        let path = write_tiny_png("external.png");
        let mut doc = nm::CadDocument::new();
        doc.entities.push(make_image_entity(
            &path,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 2.0],
            0x1,
            7,
        ));

        let opts = SvgExportOptions {
            embed_images: false,
            image_url_prefix: "images/".into(),
            ..default_opts()
        };
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(!svg.contains("data:image/"));
        assert!(svg.contains("href=\"images/external.png\""));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_file_when_embedding_skips_image() {
        let mut doc = nm::CadDocument::new();
        let missing = std::path::PathBuf::from("Z:/definitely/not/here.png");
        doc.entities.push(make_image_entity(
            &missing,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0],
            0x1,
            11,
        ));
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("<image "), "missing image must be skipped: {svg}");
    }

    #[test]
    fn display_flag_hidden_skips_image() {
        let path = write_tiny_png("hidden.png");
        let mut doc = nm::CadDocument::new();
        doc.entities.push(make_image_entity(
            &path,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0],
            0x0, // SHOW_IMAGE bit cleared
            55,
        ));
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(!svg.contains("<image "));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn image_handle_deduplicates_matching_wire() {
        let path = write_tiny_png("dedup.png");
        let mut doc = nm::CadDocument::new();
        doc.entities.push(make_image_entity(
            &path,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0],
            0x1,
            123,
        ));
        // Wire name matches the image entity handle → must be skipped in
        // favour of the native <image> element.
        let wires = vec![make_wire("123", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];

        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<image "));
        assert!(!svg.contains("<polyline"), "wire dedup failed: {svg}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn include_images_false_disables_emission() {
        let path = write_tiny_png("disabled.png");
        let mut doc = nm::CadDocument::new();
        doc.entities.push(make_image_entity(
            &path,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0],
            0x1,
            88,
        ));
        let opts = SvgExportOptions {
            include_images: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(!svg.contains("<image "));
        let _ = std::fs::remove_file(path);
    }

    // ── S2: Native curve emission ────────────────────────────────────────

    fn push_entity(
        doc: &mut nm::CadDocument,
        handle: u64,
        data: nm::EntityData,
    ) -> &mut nm::Entity {
        let mut e = nm::Entity::new(data);
        e.handle = nm::Handle::new(handle);
        e.layer_name = "0".into();
        doc.entities.push(e);
        doc.entities.last_mut().unwrap()
    }

    #[test]
    fn native_circle_emits_circle_element() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            10,
            nm::EntityData::Circle {
                center: [5.0, 7.0, 0.0],
                radius: 3.0,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<circle cx=\"5\" cy=\"7\" r=\"3\""));
        assert!(svg.contains("fill=\"none\""));
    }

    #[test]
    fn native_arc_emits_path_with_a_command() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            11,
            nm::EntityData::Arc {
                center: [0.0, 0.0, 0.0],
                radius: 4.0,
                start_angle: 0.0,
                end_angle: 90.0,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<path d=\"M 4 0 A 4 4 0 0 1"));
        assert!(svg.contains("fill=\"none\""));
    }

    #[test]
    fn native_arc_large_arc_flag_for_angles_over_180() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            12,
            nm::EntityData::Arc {
                center: [0.0, 0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: 270.0, // 3π/2 > π
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // large-arc-flag should be 1 because sweep > π.
        assert!(svg.contains("A 5 5 0 1 1"));
    }

    #[test]
    fn native_ellipse_full_axis_aligned_emits_ellipse() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            13,
            nm::EntityData::Ellipse {
                center: [10.0, 20.0, 0.0],
                major_axis: [5.0, 0.0, 0.0], // horizontal, length 5
                ratio: 0.5,                   // minor = 2.5
                start_param: 0.0,
                end_param: std::f64::consts::TAU,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            100.0,
            100.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<ellipse cx=\"10\" cy=\"20\" rx=\"5\" ry=\"2.5\""));
        // Axis-aligned → rotation should be 0.
        assert!(svg.contains("transform=\"rotate(0,10,20)\""));
    }

    #[test]
    fn native_ellipse_rotated_has_nonzero_transform() {
        let mut doc = nm::CadDocument::new();
        // major_axis (0, 1, 0) = rotated 90°.
        push_entity(
            &mut doc,
            14,
            nm::EntityData::Ellipse {
                center: [0.0, 0.0, 0.0],
                major_axis: [0.0, 1.0, 0.0],
                ratio: 0.5,
                start_param: 0.0,
                end_param: std::f64::consts::TAU,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // atan2(1,0) = 90°
        assert!(svg.contains("<ellipse "));
        assert!(svg.contains("transform=\"rotate(90,0,0)\""));
    }

    #[test]
    fn native_ellipse_partial_arc_emits_path() {
        let mut doc = nm::CadDocument::new();
        // Half-ellipse, axis-aligned, major=5, minor=2.5.
        push_entity(
            &mut doc,
            15,
            nm::EntityData::Ellipse {
                center: [0.0, 0.0, 0.0],
                major_axis: [5.0, 0.0, 0.0],
                ratio: 0.5,
                start_param: 0.0,
                end_param: std::f64::consts::PI, // half
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<path d=\"M 5 0 A 5 2.5 0 0 1"));
        // Partial arc → no standalone <ellipse> element for this entity.
        assert!(!svg.contains("<ellipse "));
    }

    #[test]
    fn native_curve_handle_deduplicates_matching_wire() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            321,
            nm::EntityData::Circle {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
            },
        );
        let wires = vec![make_wire("321", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<circle"));
        assert!(!svg.contains("<polyline"), "wire dedup failed: {svg}");
    }

    #[test]
    fn line_weight_scale_multiplier_applies_to_wire_stroke() {
        let mut w = make_wire("1", vec![[0.0, 0.0, 0.0], [5.0, 5.0, 0.0]], 7);
        w.line_weight_px = 10.0;

        // Default scale = 0.2646 → lw = 10 * 0.2646 = 2.646
        let opts_default = SvgExportOptions {
            min_stroke_width: 0.0,
            ..default_opts()
        };
        let svg_default = build_svg_full(
            &vec![w.clone()],
            &HashMap::new(),
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts_default,
        );
        assert!(svg_default.contains("stroke-width=\"2.646\""), "default: {svg_default}");

        // Override scale = 0.5 → lw = 10 * 0.5 = 5
        let opts_big = SvgExportOptions {
            min_stroke_width: 0.0,
            line_weight_scale: 0.5,
            ..default_opts()
        };
        let svg_big = build_svg_full(
            &vec![w],
            &HashMap::new(),
            None,
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts_big,
        );
        assert!(svg_big.contains("stroke-width=\"5\""), "scaled: {svg_big}");
    }

    // ── Phase 5 P1: Optional real-file smoke test ────────────────────────
    //
    // Run with:
    //   $env:H7CAD_REAL_DXF = "D:\\CAD\\ODATrial\\...\\2025.09.18.dxf"
    //   cargo test --package H7CAD --bin H7CAD svg_export_real_file
    //
    // The test is env-gated so CI/ordinary runs skip it when the path is
    // not available.  It asserts basic sanity (file reads, SVG not empty,
    // reasonable size, contains native elements) and prints a metrics
    // table that doubles as a regression log.

    #[test]
    fn svg_export_real_file_roundtrip_if_env_set() {
        let Ok(path_str) = std::env::var("H7CAD_REAL_DXF") else {
            eprintln!("[svg_export_real_file] skipped — set H7CAD_REAL_DXF to enable");
            return;
        };
        let path = std::path::Path::new(&path_str);
        if !path.exists() {
            eprintln!("[svg_export_real_file] skipped — file not found: {path_str}");
            return;
        }
        let dxf_bytes = std::fs::read(path).expect("read DXF");
        let doc = h7cad_native_dxf::read_dxf_bytes(&dxf_bytes).expect("parse DXF");

        // Count entity variants (cheap, useful for the regression log).
        let mut line = 0u32;
        let mut circle = 0u32;
        let mut arc = 0u32;
        let mut ellipse = 0u32;
        let mut polyline = 0u32;
        let mut text = 0u32;
        let mut mtext = 0u32;
        let mut insert = 0u32;
        let mut spline = 0u32;
        let mut image = 0u32;
        let mut other = 0u32;
        for e in &doc.entities {
            match &e.data {
                nm::EntityData::Line { .. } => line += 1,
                nm::EntityData::Circle { .. } => circle += 1,
                nm::EntityData::Arc { .. } => arc += 1,
                nm::EntityData::Ellipse { .. } => ellipse += 1,
                nm::EntityData::LwPolyline { .. } | nm::EntityData::Polyline { .. } => {
                    polyline += 1
                }
                nm::EntityData::Text { .. } => text += 1,
                nm::EntityData::MText { .. } => mtext += 1,
                nm::EntityData::Insert { .. } => insert += 1,
                nm::EntityData::Spline { .. } => spline += 1,
                nm::EntityData::Image { .. } => image += 1,
                _ => other += 1,
            }
        }

        // Wires: build straight-line WireModels for everything that is NOT
        // covered by a native emitter so the wire pass still produces
        // something for Spline / Dimension / Polyline3D etc.
        let mut wires: Vec<WireModel> = Vec::new();
        for entity in &doc.entities {
            let handle = entity.handle.value().to_string();
            if let nm::EntityData::Line { start, end } = &entity.data {
                wires.push(make_wire(
                    &handle,
                    vec![
                        [start[0] as f32, start[1] as f32, start[2] as f32],
                        [end[0] as f32, end[1] as f32, end[2] as f32],
                    ],
                    7,
                ));
            }
        }

        let opts = SvgExportOptions {
            // Keep embedded images off for the smoke test — the base64
            // inlining can multiply the output size dramatically.
            embed_images: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            1000.0,
            700.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );

        // Report.
        eprintln!("[svg_export_real_file] file: {path_str}");
        eprintln!("  DXF bytes        : {}", dxf_bytes.len());
        eprintln!("  SVG bytes        : {}", svg.len());
        eprintln!(
            "  Size ratio       : {:.2}x",
            svg.len() as f64 / dxf_bytes.len() as f64
        );
        eprintln!(
            "  entities  line={line} circle={circle} arc={arc} ellipse={ellipse} \
             polyline={polyline} text={text} mtext={mtext} insert={insert} \
             spline={spline} image={image} other={other}",
        );
        eprintln!(
            "  <text>   count   : {}",
            svg.matches("<text ").count()
        );
        eprintln!(
            "  <circle> count   : {}",
            svg.matches("<circle ").count()
        );
        eprintln!(
            "  <path>   count   : {}",
            svg.matches("<path ").count()
        );
        eprintln!(
            "  <ellipse> count  : {}",
            svg.matches("<ellipse ").count()
        );
        eprintln!(
            "  <polyline> count : {}",
            svg.matches("<polyline ").count()
        );
        eprintln!(
            "  <polygon> count  : {}",
            svg.matches("<polygon ").count()
        );
        eprintln!(
            "  <image>  count   : {}",
            svg.matches("<image ").count()
        );
        eprintln!(
            "  <use>    count   : {}",
            svg.matches("<use ").count()
        );

        // Sanity assertions — keep the threshold loose to avoid flakiness.
        assert!(!svg.is_empty(), "SVG output should not be empty");
        assert!(
            svg.contains("<svg"),
            "SVG root element missing: {} bytes",
            svg.len()
        );
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(
            svg.len() < dxf_bytes.len() * 20,
            "SVG output suspiciously large ({} vs {} DXF)",
            svg.len(),
            dxf_bytes.len()
        );

        // Optional: if the caller also set H7CAD_REAL_DXF_OUT, write the
        // SVG so they can open it in a browser for visual comparison.
        if let Ok(out) = std::env::var("H7CAD_REAL_DXF_OUT") {
            std::fs::write(&out, &svg).expect("write output SVG");
            eprintln!("[svg_export_real_file] wrote SVG → {out}");
        }
    }

    // ── Phase 4 T1: End-to-end integration with the fixture DXF ─────────

    #[test]
    fn sample_dxf_end_to_end_produces_all_native_shapes() {
        // The fixture carries 4 LINE + 1 CIRCLE + 1 ARC + 1 TEXT.  Driving it
        // through the full native pipeline should yield one <circle>, one
        // arc-shaped <path>, one <text>, and four <polyline> elements (one
        // per LINE — no native <line> emission is wired yet).
        const SAMPLE: &[u8] = include_bytes!("../../tests/fixtures/sample.dxf");
        let doc = h7cad_native_dxf::read_dxf_bytes(SAMPLE).expect("sample.dxf parses");

        // Build WireModels directly from the LINE entities so the wire pass
        // has something to emit (the scene::entity_wires helper isn't
        // available here — keep the test independent of app state).
        let mut wires = Vec::new();
        for entity in &doc.entities {
            if let nm::EntityData::Line { start, end } = &entity.data {
                let w = make_wire(
                    &entity.handle.value().to_string(),
                    vec![
                        [start[0] as f32, start[1] as f32, start[2] as f32],
                        [end[0] as f32, end[1] as f32, end[2] as f32],
                    ],
                    7,
                );
                wires.push(w);
            }
        }
        assert_eq!(wires.len(), 4, "fixture has four LINE entities");

        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            210.0,
            297.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );

        // Native emissions: circle, arc (path), text.
        assert_eq!(
            svg.matches("<circle ").count(),
            1,
            "expected one native <circle> element: {svg}"
        );
        assert!(
            svg.contains("<path d=\"M") && svg.contains(" A "),
            "expected one arc emitted as <path>: {svg}"
        );
        assert_eq!(
            svg.matches("<text ").count(),
            1,
            "expected one <text> element: {svg}"
        );
        // Wires: four LINE → four polyline elements.
        assert_eq!(
            svg.matches("<polyline ").count(),
            4,
            "expected four wire polylines: {svg}"
        );
        // XML skeleton sanity.
        assert!(svg.starts_with("<?xml version=\"1.0\""));
        assert!(svg.contains("viewBox=\"0 0 210 297\""));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    // ── Phase 4 T2: LwPolyline native emission (top level) ───────────────

    fn lwvertex(x: f64, y: f64, bulge: f64) -> nm::LwVertex {
        nm::LwVertex {
            x,
            y,
            bulge,
            start_width: 0.0,
            end_width: 0.0,
        }
    }

    #[test]
    fn native_lwpolyline_straight_emits_polyline_fast_path() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            201,
            nm::EntityData::LwPolyline {
                vertices: vec![
                    lwvertex(0.0, 0.0, 0.0),
                    lwvertex(10.0, 0.0, 0.0),
                    lwvertex(10.0, 5.0, 0.0),
                ],
                closed: false,
                constant_width: 0.0,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // No bulge ⇒ compact <polyline> fast path.
        assert!(svg.contains("<polyline "));
        assert!(svg.contains("points=\"0,0 10,0 10,5\""));
        // Should NOT fall back to wire emission for the same handle.
    }

    #[test]
    fn native_lwpolyline_bulge_emits_path_with_arc_command() {
        let mut doc = nm::CadDocument::new();
        // Semicircle via bulge = 1 on a unit-length chord.
        push_entity(
            &mut doc,
            202,
            nm::EntityData::LwPolyline {
                vertices: vec![
                    lwvertex(0.0, 0.0, 1.0),
                    lwvertex(2.0, 0.0, 0.0),
                ],
                closed: false,
                constant_width: 0.0,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // bulge=1 → θ=π → radius=chord/2=1; not a large arc (|b|==1 → 0).
        assert!(svg.contains("<path d=\"M 0 0 A 1 1 0 0 1 2 0"));
    }

    #[test]
    fn native_lwpolyline_closed_emits_z_command() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            203,
            nm::EntityData::LwPolyline {
                vertices: vec![
                    lwvertex(0.0, 0.0, 0.0),
                    lwvertex(4.0, 0.0, 0.5),
                    lwvertex(4.0, 3.0, 0.0),
                ],
                closed: true,
                constant_width: 0.0,
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // bulge on segment 1 → one A command; closed path ends with Z.
        assert!(svg.contains("<path d=\"M 0 0 L 4 0 A "));
        assert!(svg.contains(" Z\""), "closed path should end with Z: {svg}");
    }

    #[test]
    fn native_lwpolyline_handle_deduplicates_wire() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            204,
            nm::EntityData::LwPolyline {
                vertices: vec![lwvertex(0.0, 0.0, 0.0), lwvertex(5.0, 0.0, 0.0)],
                closed: false,
                constant_width: 0.0,
            },
        );
        // Wire whose name matches the polyline entity handle → must be
        // dropped once the native path replaces it.
        let wires = vec![make_wire("204", vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // The native polyline uses <polyline> (straight fast path); the
        // wire pass would ALSO produce a <polyline> — dedup guarantees
        // exactly one is emitted.
        assert_eq!(svg.matches("<polyline ").count(), 1, "dedup: {svg}");
    }

    // ── Phase 5 P2: Native Spline emission ────────────────────────────────

    #[test]
    fn native_spline_degree_1_emits_polyline_of_control_points() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            701,
            nm::EntityData::Spline {
                degree: 1,
                closed: false,
                knots: vec![0.0, 0.0, 1.0, 1.0],
                control_points: vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0, 5.0, 0.0]],
                weights: vec![1.0, 1.0, 1.0],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<polyline "));
        assert!(svg.contains("points=\"0,0 5,0 5,5\""));
    }

    #[test]
    fn native_spline_clamped_cubic_emits_bezier_path() {
        // Phase 7: a clamped non-rational cubic (degree 3, 4 cps, knots all 0
        // or 1) is a single Bezier segment.  We now prefer the exact `<path
        // C>` output over the earlier fit-point polyline approximation.
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            702,
            nm::EntityData::Spline {
                degree: 3,
                closed: false,
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                control_points: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                weights: vec![1.0; 4],
                fit_points: vec![[0.0, 0.0, 0.0], [1.5, 1.0, 0.0], [3.0, 0.0, 0.0]],
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // Exact Bezier path, one segment (`M` + one `C`).
        assert!(svg.contains("<path "), "svg was: {svg}");
        assert!(
            svg.contains("M 0 0 C 1 2 2 2 3 0"),
            "expected Bezier path in: {svg}"
        );
        // fit-poly polyline should NOT appear for this spline any more.
        assert!(!svg.contains("points=\"0,0 1.5,1 3,0\""), "svg was: {svg}");
    }

    #[test]
    fn native_spline_high_order_no_fit_points_falls_back_to_wire() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            703,
            nm::EntityData::Spline {
                degree: 5,
                closed: false,
                knots: vec![0.0; 12],
                control_points: vec![[0.0; 3]; 6],
                weights: vec![1.0; 6],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        // Provide a wire so we can assert the fallback kicks in.
        let wires = vec![make_wire("703", vec![[0.0, 0.0, 0.0], [9.0, 0.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert!(svg.contains("<polyline "));
        // No native path generated for the spline itself — the only polyline
        // is the wire fallback.
        assert_eq!(svg.matches("<polyline ").count(), 1);
    }

    #[test]
    fn native_spline_closed_degree_1_emits_polygon() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            704,
            nm::EntityData::Spline {
                degree: 1,
                closed: true,
                knots: vec![0.0; 4],
                control_points: vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0, 5.0, 0.0]],
                weights: vec![1.0; 3],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // Closed straight polyline → <polygon>.
        assert!(svg.contains("<polygon "));
    }

    // ── Phase 7: NURBS → Bezier decomposition ─────────────────────────────

    #[test]
    fn bspline_to_bezier_single_cubic_segment_returns_control_points_unchanged() {
        // Clamped cubic with no internal knots: the 4 control points already
        // ARE the single Bezier segment.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let out = bspline_to_bezier(3, &knots, &cps).expect("clamped cubic");
        assert_eq!(out.len(), 4);
        for (a, b) in out.iter().zip(cps.iter()) {
            for i in 0..3 {
                assert!((a[i] - b[i]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn bspline_to_bezier_cubic_with_one_internal_knot_yields_two_segments() {
        // 5 control points, 10 knots, one internal knot at 0.5 → 2 segments,
        // 7 refined control points, first point matches input.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let out = bspline_to_bezier(3, &knots, &cps).expect("clamped cubic");
        // (segments * degree + 1) = 2 * 3 + 1 = 7
        assert_eq!(out.len(), 7);
        // First and last points are preserved (clamped spline property).
        for i in 0..3 {
            assert!((out[0][i] - cps[0][i]).abs() < 1e-12);
            assert!((out[6][i] - cps[4][i]).abs() < 1e-12);
        }
        // The shared boundary control point (index 3) lies on the original
        // curve at u=0.5 and must sit at a sensible y-coordinate between
        // the inner hump and the descent — strictly positive for this shape.
        assert!(out[3][1] > 0.0, "boundary y should be positive: {:?}", out[3]);
    }

    #[test]
    fn bspline_to_bezier_quadratic_emits_one_segment() {
        // 3 cps, knots [0,0,0,1,1,1] → single quadratic Bezier.
        let cps = vec![[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0]];
        let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let out = bspline_to_bezier(2, &knots, &cps).expect("clamped quadratic");
        assert_eq!(out.len(), 3);
        for (a, b) in out.iter().zip(cps.iter()) {
            for i in 0..3 {
                assert!((a[i] - b[i]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn bspline_to_bezier_rejects_non_clamped_knots() {
        // Uniform (open, non-clamped) knots → we don't try to decompose.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        assert!(bspline_to_bezier(3, &knots, &cps).is_none());
    }

    #[test]
    fn bspline_to_bezier_rejects_unsupported_degree() {
        let cps = vec![[0.0; 3]; 6];
        let knots = vec![0.0; 11];
        assert!(bspline_to_bezier(4, &knots, &cps).is_none());
        assert!(bspline_to_bezier(1, &knots, &cps).is_none());
    }

    #[test]
    fn bspline_to_bezier_rejects_mismatched_inputs() {
        // knots.len() must equal cps.len() + degree + 1.
        let cps = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let wrong_knots = vec![0.0; 5]; // would need 7 for degree 3
        assert!(bspline_to_bezier(3, &wrong_knots, &cps).is_none());
    }

    #[test]
    fn spline_emit_strategy_prefers_bezier_over_fit_poly() {
        // A clamped cubic with fit_points should still go through Bezier
        // because that's the more accurate representation.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let fit = vec![[0.0, 0.0, 0.0], [1.5, 1.0, 0.0], [3.0, 0.0, 0.0]];
        let weights = vec![1.0; 4];
        let strat = spline_emit_strategy(3, false, &knots, &cps, &weights, &fit);
        assert!(matches!(strat, Some(SplineEmit::Bezier { .. })));
    }

    #[test]
    fn spline_emit_strategy_falls_back_when_closed() {
        // Closed/periodic splines still go through fit-poly or wire.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let fit = vec![[0.0, 0.0, 0.0], [1.5, 1.0, 0.0]];
        let weights = vec![1.0; 4];
        let strat = spline_emit_strategy(3, true, &knots, &cps, &weights, &fit);
        assert!(matches!(strat, Some(SplineEmit::FitPoly)));
    }

    #[test]
    fn spline_emit_strategy_rational_defers_to_fit_poly() {
        // Non-unit weights = true NURBS; we don't handle rational curves yet.
        let cps = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 0.0, 0.0],
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let weights = vec![1.0, 2.0, 1.0, 1.0];
        let fit = vec![[0.0, 0.0, 0.0], [1.5, 1.0, 0.0]];
        let strat = spline_emit_strategy(3, false, &knots, &cps, &weights, &fit);
        assert!(matches!(strat, Some(SplineEmit::FitPoly)));
    }

    #[test]
    fn native_spline_bezier_path_includes_scene_offset() {
        // Sanity: scene offset (ox=10, oy=20) is applied to every point in
        // the emitted Bezier path.
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            720,
            nm::EntityData::Spline {
                degree: 3,
                closed: false,
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                control_points: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                weights: vec![1.0; 4],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            10.0,
            20.0,
            0,
            None,
            &default_opts(),
        );
        // M at (0+10, 0+20) = (10, 20); first cubic C control at (1+10, 2+20).
        assert!(svg.contains("M 10 20"), "svg was: {svg}");
        assert!(svg.contains("C 11 22 12 22 13 20"), "svg was: {svg}");
    }

    #[test]
    fn native_spline_bezier_path_suppresses_wire_fallback() {
        // Emitting Bezier natively should stop the wire passthrough from
        // also drawing this handle.
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            721,
            nm::EntityData::Spline {
                degree: 3,
                closed: false,
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                control_points: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 2.0, 0.0],
                    [2.0, 2.0, 0.0],
                    [3.0, 0.0, 0.0],
                ],
                weights: vec![1.0; 4],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let wires = vec![make_wire("721", vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0]], 7)];
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        assert_eq!(
            svg.matches("<polyline ").count(),
            0,
            "wire must be suppressed when Bezier path is emitted: {svg}"
        );
        assert_eq!(svg.matches("<path ").count(), 1, "svg was: {svg}");
    }

    #[test]
    fn native_splines_false_disables_emission() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            705,
            nm::EntityData::Spline {
                degree: 1,
                closed: false,
                knots: vec![0.0; 4],
                control_points: vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]],
                weights: vec![1.0; 2],
                fit_points: Vec::new(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        );
        let wires = vec![make_wire("705", vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]], 7)];
        let opts = SvgExportOptions {
            native_splines: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(svg.contains("<polyline "));
        // Wire survives because the native pass is gated off.
        assert_eq!(svg.matches("<polyline ").count(), 1);
    }

    // ── Phase 4 T3: Ellipse inside a block ────────────────────────────────

    #[test]
    fn block_containing_ellipse_enters_defs_use() {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(9000);
        let mut br = nm::BlockRecord::new(bh, "BLK_ELLIPSE");
        let mut el = nm::Entity::new(nm::EntityData::Ellipse {
            center: [0.0, 0.0, 0.0],
            major_axis: [2.0, 0.0, 0.0],
            ratio: 0.5,
            start_param: 0.0,
            end_param: std::f64::consts::TAU,
        });
        el.handle = nm::Handle::new(9001);
        el.owner_handle = bh;
        br.entities.push(el);
        doc.insert_block_record(br);
        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_ELLIPSE".into(),
            insertion: [5.0, 5.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(900);
        doc.entities.push(insert);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // The ellipse lives inside <defs>; <use> drops it into world.
        assert!(svg.contains("id=\"blk_9000\""));
        assert!(svg.contains("<ellipse cx=\"0\" cy=\"0\" rx=\"2\" ry=\"1\""));
        assert!(svg.contains("<use href=\"#blk_9000\""));
        assert!(svg.contains("translate(5,5)"));
    }

    #[test]
    fn block_with_partial_ellipse_arc_emits_path_in_defs() {
        let mut doc = nm::CadDocument::new();
        let bh = nm::Handle::new(9100);
        let mut br = nm::BlockRecord::new(bh, "BLK_ELARC");
        let mut el = nm::Entity::new(nm::EntityData::Ellipse {
            center: [0.0, 0.0, 0.0],
            major_axis: [5.0, 0.0, 0.0],
            ratio: 0.4,
            start_param: 0.0,
            end_param: std::f64::consts::PI, // half-ellipse
        });
        el.handle = nm::Handle::new(9101);
        el.owner_handle = bh;
        br.entities.push(el);
        doc.insert_block_record(br);
        let mut insert = nm::Entity::new(nm::EntityData::Insert {
            block_name: "BLK_ELARC".into(),
            insertion: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            rotation: 0.0,
            has_attribs: false,
            attribs: Vec::new(),
        });
        insert.handle = nm::Handle::new(910);
        doc.entities.push(insert);

        let svg = build_svg_full(
            &Vec::<WireModel>::new(),
            &HashMap::new(),
            Some(&doc),
            50.0,
            50.0,
            0.0,
            0.0,
            0,
            None,
            &default_opts(),
        );
        // Partial ellipse arc in defs emits a <path> with A.
        assert!(svg.contains("id=\"blk_9100\""));
        assert!(svg.contains("<path d=\"M 5 0 A 5 2 0 0 1"));
    }

    #[test]
    fn native_curves_false_falls_back_to_wire() {
        let mut doc = nm::CadDocument::new();
        push_entity(
            &mut doc,
            99,
            nm::EntityData::Circle {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
            },
        );
        let wires = vec![make_wire("99", vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], 7)];
        let opts = SvgExportOptions {
            native_curves: false,
            ..default_opts()
        };
        let svg = build_svg_full(
            &wires,
            &HashMap::new(),
            Some(&doc),
            10.0,
            10.0,
            0.0,
            0.0,
            0,
            None,
            &opts,
        );
        assert!(!svg.contains("<circle"));
        assert!(svg.contains("<polyline"), "fallback wire should remain: {svg}");
    }
}
