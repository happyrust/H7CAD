// PDF export — converts the paper-space wire model to a PDF file using printpdf.
//
// Each WireModel becomes a sequence of DrawLine operations.  NaN values in the
// points array act as segment separators (pen-up).
//
// Coordinate system: CAD uses mm units with origin at bottom-left and Y up.
// printpdf's Point::new(Mm, Mm) also has origin at bottom-left, so no Y-flip
// is needed — we shift the coordinates by (offset_x, offset_y) to place the
// drawing origin at the paper origin.

use crate::scene::WireModel;
use printpdf::{Color, Line, LineCapStyle, LineJoinStyle, LinePoint, Mm, Op, PdfDocument,
               PdfPage, PdfSaveOptions, Point, Pt, Rgb};
use std::io::Write;
use std::path::Path;

// ── Public entry point ────────────────────────────────────────────────────

/// Export `wires` to a PDF file.
///
/// - `paper_w` / `paper_h`: page dimensions in mm.
/// - `offset_x` / `offset_y`: added to every wire coordinate so the drawing
///   origin maps to the bottom-left corner of the page.  Pass 0/0 for paper
///   space (coordinates are already relative to page origin); pass a computed
///   shift for model space to normalise the extents.
pub fn export_pdf(
    wires: &[WireModel],
    paper_w: f64,
    paper_h: f64,
    offset_x: f32,
    offset_y: f32,
    path: &Path,
) -> Result<(), String> {
    let bytes = build_pdf(wires, paper_w as f32, paper_h as f32, offset_x, offset_y);
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

// ── PDF builder ───────────────────────────────────────────────────────────

fn build_pdf(wires: &[WireModel], paper_w: f32, paper_h: f32, ox: f32, oy: f32) -> Vec<u8> {
    let mut doc = PdfDocument::new("H7CAD Export");
    let mut ops: Vec<Op> = Vec::new();

    // White page background.
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb { r: 1.0, g: 1.0, b: 1.0, icc_profile: None }),
    });
    ops.push(Op::DrawRectangle {
        rectangle: printpdf::Rect::from_wh(Mm(paper_w).into(), Mm(paper_h).into()),
    });

    // Round line caps/joins for CAD aesthetics.
    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
    ops.push(Op::SetLineJoinStyle { join: LineJoinStyle::Round });

    let mut last_color: Option<[f32; 3]> = None;
    let mut last_lw: Option<f32> = None;

    for wire in wires {
        let [mut r, mut g, mut b, a] = wire.color;
        if a < 0.01 {
            continue;
        }
        // Skip the paper-boundary wire — the white PDF background already provides it.
        if wire.name == "__paper_boundary__" {
            continue;
        }
        // Near-white and near-yellow (viewport active border) → dark grey for print.
        let is_light = r > 0.80 && g > 0.80 && b > 0.80;
        let is_yellow = r > 0.80 && g > 0.70 && b < 0.30;
        let is_cyan   = r < 0.30 && g > 0.70 && b > 0.70;
        if is_light || is_yellow {
            r = 0.0; g = 0.0; b = 0.0;
        } else if is_cyan {
            // Viewport border: print as dark blue.
            r = 0.0; g = 0.15; b = 0.50;
        }

        if last_color.map(|c| {
            (c[0]-r).abs() > 0.01 || (c[1]-g).abs() > 0.01 || (c[2]-b).abs() > 0.01
        }).unwrap_or(true) {
            ops.push(Op::SetOutlineColor {
                col: Color::Rgb(Rgb { r, g, b, icc_profile: None }),
            });
            last_color = Some([r, g, b]);
        }

        // Line weight: screen px → points (approximate 1 px ≈ 0.35 pt).
        let lw_pt = (wire.line_weight_px * 0.35278_f32).max(0.1);
        if last_lw.map(|l| (l - lw_pt).abs() > 0.01).unwrap_or(true) {
            ops.push(Op::SetOutlineThickness { pt: Pt(lw_pt) });
            last_lw = Some(lw_pt);
        }

        // Emit segments (NaN = pen-up).
        let mut segment: Vec<LinePoint> = Vec::new();
        for &[x, y, _z] in &wire.points {
            if x.is_nan() || y.is_nan() {
                flush_line(&mut ops, &segment);
                segment.clear();
            } else {
                segment.push(LinePoint {
                    p: Point::new(Mm(x + ox), Mm(y + oy)),
                    bezier: false,
                });
            }
        }
        flush_line(&mut ops, &segment);
    }

    let page = PdfPage::new(Mm(paper_w), Mm(paper_h), ops);
    doc.pages.push(page);

    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
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
