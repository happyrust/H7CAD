// PDF export — converts the paper-space wire model to a PDF file using printpdf.
//
// Each WireModel becomes a sequence of DrawLine operations.  NaN values in the
// points array act as segment separators (pen-up).
//
// Coordinate system: CAD uses mm units with origin at bottom-left and Y up.
// printpdf's Point::new(Mm, Mm) also has origin at bottom-left, so no Y-flip
// is needed — we just pass the coordinates through directly.

use crate::scene::WireModel;
use printpdf::{Color, Line, LineCapStyle, LineJoinStyle, LinePoint, Mm, Op, PdfDocument,
               PdfPage, PdfSaveOptions, Point, Pt, Rgb};
use std::io::Write;
use std::path::Path;

// ── Public entry point ────────────────────────────────────────────────────

/// Export `wires` to a PDF file.
///
/// `paper_w` / `paper_h` are in millimetres (drawing units assumed = mm).
pub fn export_pdf(
    wires: &[WireModel],
    paper_w: f64,
    paper_h: f64,
    path: &Path,
) -> Result<(), String> {
    let bytes = build_pdf(wires, paper_w as f32, paper_h as f32);
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

fn build_pdf(wires: &[WireModel], paper_w: f32, paper_h: f32) -> Vec<u8> {
    let mut doc = PdfDocument::new("H7CAD Export");
    let mut ops: Vec<Op> = Vec::new();

    // White page background rectangle.
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb { r: 1.0, g: 1.0, b: 1.0, icc_profile: None }),
    });
    ops.push(Op::DrawRectangle {
        rectangle: printpdf::Rect::from_wh(Mm(paper_w).into(), Mm(paper_h).into()),
    });

    // Round line caps for CAD aesthetics.
    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
    ops.push(Op::SetLineJoinStyle { join: LineJoinStyle::Round });

    let mut last_color: Option<[f32; 4]> = None;
    let mut last_lw: Option<f32> = None;

    for wire in wires {
        // Set stroke color (white → black for print; skip fully transparent).
        let [mut r, mut g, mut b, a] = wire.color;
        if a < 0.01 {
            continue;
        }
        // Invert near-white to black so it prints on white paper.
        if r > 0.85 && g > 0.85 && b > 0.85 {
            r = 0.0;
            g = 0.0;
            b = 0.0;
        }
        let color_changed = last_color.map(|c| {
            (c[0] - r).abs() > 0.01 || (c[1] - g).abs() > 0.01 || (c[2] - b).abs() > 0.01
        }).unwrap_or(true);
        if color_changed {
            ops.push(Op::SetOutlineColor {
                col: Color::Rgb(Rgb { r, g, b, icc_profile: None }),
            });
            last_color = Some([r, g, b, a]);
        }

        // Set line width (in points; 1 mm = 2.8346 pt).
        let lw_pt = (wire.line_weight_px as f32 * 0.35278).max(0.1);
        if last_lw.map(|l| (l - lw_pt).abs() > 0.01).unwrap_or(true) {
            ops.push(Op::SetOutlineThickness { pt: Pt(lw_pt) });
            last_lw = Some(lw_pt);
        }

        // Collect segments (split at NaN).
        let mut segment: Vec<LinePoint> = Vec::new();
        for &[x, y, _z] in &wire.points {
            if x.is_nan() || y.is_nan() {
                flush_line(&mut ops, &segment);
                segment.clear();
            } else {
                segment.push(LinePoint {
                    p: Point::new(Mm(x), Mm(y)),
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
