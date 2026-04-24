//! Headless CLI batch path (三十六轮).
//!
//! Allows the `h7cad` binary to perform DXF → PDF conversion without
//! launching the iced GUI, so CI / automation pipelines can integrate it.
//!
//! Invocation:
//!
//! ```text
//! h7cad drawing.dxf --export-pdf out.pdf
//! h7cad drawing.dxf --export-pdf            # infers out = drawing.pdf
//! h7cad --help
//! ```
//!
//! The GUI entry (`h7cad drawing.dxf` without a batch flag) is unaffected —
//! `main.rs` only diverts to this module when a batch flag is present.

use std::path::{Path, PathBuf};

/// Parsed form of a recognised batch-mode invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchArgs {
    /// Show usage string on stdout and exit 0.
    Help,
    /// Export `input` → `output` as PDF.
    ExportPdf { input: PathBuf, output: PathBuf },
}

/// Inspect `args` (already stripped of argv[0]) and return a recognised
/// batch form, or `None` if the user meant to launch the GUI.
pub fn parse_batch_args(args: &[String]) -> Option<BatchArgs> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Some(BatchArgs::Help);
    }

    // Any supported batch flag currently requires at least `<input> --export-pdf`.
    let export_idx = args.iter().position(|a| a == "--export-pdf")?;

    // The first arg after `--export-pdf` is treated as the output path when
    // it doesn't itself look like a flag — so we must skip it during input
    // search, otherwise invocation `--export-pdf out.pdf drawing.dxf` would
    // pick `out.pdf` for both input and output.
    let output_idx = args
        .get(export_idx + 1)
        .filter(|s| !s.starts_with('-'))
        .map(|_| export_idx + 1);

    let input = args
        .iter()
        .enumerate()
        .find(|(i, a)| *i != export_idx && Some(*i) != output_idx && !a.starts_with('-'))
        .map(|(_, a)| PathBuf::from(a))?;

    let output = match output_idx {
        Some(idx) => PathBuf::from(&args[idx]),
        None => {
            // Infer output by swapping extension to .pdf.
            let mut inferred = input.clone();
            inferred.set_extension("pdf");
            inferred
        }
    };

    Some(BatchArgs::ExportPdf { input, output })
}

/// Short help text; kept inline (not pulled from a markdown file) so the
/// build produces a self-contained binary.
pub const HELP_TEXT: &str = "\
H7CAD — CAD viewer and DXF/DWG editor

USAGE:
    h7cad                                         Launch the GUI.
    h7cad <PATH>                                  Launch the GUI and open PATH.
    h7cad <INPUT.dxf> --export-pdf [OUTPUT.pdf]   Batch convert DXF → PDF.
    h7cad --help                                  Show this message.

BATCH EXPORT NOTES:
    When `OUTPUT.pdf` is omitted, it defaults to `<INPUT>.pdf`.
    The batch path uses default `PdfExportOptions` (monochrome, native
    curves/splines/text, solid + pattern HATCH, embedded images).
    Exit code 0 on success, 1 on failure (diagnostic printed to stderr).
";

/// Execute the batch path.  Matches the entry-point signature expected by
/// `main.rs`: `Ok(())` on success, `Err(String)` with a human-readable
/// diagnostic otherwise.
pub fn run_batch_export(args: BatchArgs) -> Result<(), String> {
    match args {
        BatchArgs::Help => {
            print!("{HELP_TEXT}");
            Ok(())
        }
        BatchArgs::ExportPdf { input, output } => export_pdf(&input, &output),
    }
}

fn export_pdf(input: &Path, output: &Path) -> Result<(), String> {
    if !input.exists() {
        return Err(format!("cannot open \"{}\": file not found", input.display()));
    }

    let (compat, native, _notices) = crate::io::load_file_with_native_blocking(input)
        .map_err(|e| format!("failed to load \"{}\": {e}", input.display()))?;

    // Assemble a headless Scene mirroring the GUI default-display path:
    // compat doc in the tessellator, native doc preserved for bridge-aware
    // emits (text / images / native curves / native splines).
    let mut scene = crate::scene::Scene::new();
    scene.document = compat;
    scene.set_native_doc(native);
    scene.native_render_enabled = false;

    let wires = scene.entity_wires();

    // Paper size: prefer paper_limits if the active layout supplies them,
    // otherwise fit the model-space extents with a 5% margin, otherwise
    // fall back to A4 (297 × 210).  Mirrors the PlotExportPath branch in
    // `src/app/update.rs` but drops PlotSettings / centering / rotation
    // so the CLI output is deterministic and config-free.
    let (paper_w, paper_h, offset_x, offset_y) = resolve_paper_and_offset(&scene);

    let options = crate::io::pdf_export::PdfExportOptions::default();
    crate::io::pdf_export::export_pdf_full(
        &wires,
        &scene.hatches,
        scene.native_doc(),
        paper_w,
        paper_h,
        offset_x,
        offset_y,
        0, // no rotation
        output,
        None, // no CTB
        &options,
    )
    .map_err(|e| format!("PDF export failed: {e}"))?;

    eprintln!(
        "h7cad: wrote {} ({:.1} × {:.1} mm, {} wires)",
        output.display(),
        paper_w,
        paper_h,
        wires.len()
    );
    Ok(())
}

fn resolve_paper_and_offset(scene: &crate::scene::Scene) -> (f64, f64, f32, f32) {
    if let Some(((x0, y0), (x1, y1))) = scene.paper_limits() {
        return (
            (x1 - x0) as f64,
            (y1 - y0) as f64,
            -(x0 as f32),
            -(y0 as f32),
        );
    }
    if let Some((mn, mx)) = scene.model_space_extents() {
        let margin = 1.05_f64;
        let w = ((mx.x - mn.x) as f64 * margin).max(1.0);
        let h = ((mx.y - mn.y) as f64 * margin).max(1.0);
        let pad_x = (w - (mx.x - mn.x) as f64) * 0.5;
        let pad_y = (h - (mx.y - mn.y) as f64) * 0.5;
        return (
            w,
            h,
            -(mn.x) + pad_x as f32,
            -(mn.y) + pad_y as f32,
        );
    }
    (297.0, 210.0, 0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_returns_none_for_plain_gui_invocation() {
        assert_eq!(parse_batch_args(&[]), None);
        assert_eq!(parse_batch_args(&s(&["drawing.dxf"])), None);
    }

    #[test]
    fn parse_recognises_help_flag() {
        assert_eq!(parse_batch_args(&s(&["--help"])), Some(BatchArgs::Help));
        assert_eq!(parse_batch_args(&s(&["-h"])), Some(BatchArgs::Help));
        // --help wins over any other args.
        assert_eq!(
            parse_batch_args(&s(&["input.dxf", "--export-pdf", "--help"])),
            Some(BatchArgs::Help)
        );
    }

    #[test]
    fn parse_extracts_input_and_output() {
        let got = parse_batch_args(&s(&["drawing.dxf", "--export-pdf", "out.pdf"]));
        assert_eq!(
            got,
            Some(BatchArgs::ExportPdf {
                input: PathBuf::from("drawing.dxf"),
                output: PathBuf::from("out.pdf"),
            })
        );
    }

    #[test]
    fn parse_infers_output_when_missing() {
        let got = parse_batch_args(&s(&["drawing.dxf", "--export-pdf"]));
        assert_eq!(
            got,
            Some(BatchArgs::ExportPdf {
                input: PathBuf::from("drawing.dxf"),
                output: PathBuf::from("drawing.pdf"),
            })
        );
    }

    #[test]
    fn parse_accepts_flag_order_swapped() {
        let got = parse_batch_args(&s(&["--export-pdf", "out.pdf", "drawing.dxf"]));
        assert_eq!(
            got,
            Some(BatchArgs::ExportPdf {
                input: PathBuf::from("drawing.dxf"),
                output: PathBuf::from("out.pdf"),
            })
        );
    }

    #[test]
    fn run_batch_export_help_succeeds() {
        assert!(run_batch_export(BatchArgs::Help).is_ok());
    }

    #[test]
    fn run_batch_export_missing_file_fails() {
        let err = run_batch_export(BatchArgs::ExportPdf {
            input: PathBuf::from("this_definitely_does_not_exist.dxf"),
            output: PathBuf::from("out.pdf"),
        })
        .expect_err("missing input must fail");
        assert!(
            err.to_lowercase().contains("cannot open"),
            "expected 'cannot open' in error, got: {err}"
        );
    }
}
