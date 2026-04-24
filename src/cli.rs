//! Headless CLI batch path (三十六轮 初版; 三十七轮扩展至多输入 + SVG).
//!
//! Allows the `h7cad` binary to perform DXF → PDF / DXF → SVG conversion
//! without launching the iced GUI, so CI / automation pipelines can
//! integrate it.  Supports multiple input files in a single invocation.
//!
//! Invocation:
//!
//! ```text
//! h7cad INPUT.dxf --export-pdf OUTPUT.pdf           # single, explicit output
//! h7cad INPUT.dxf --export-pdf                      # single, inferred output (INPUT.pdf)
//! h7cad A.dxf B.dxf C.dxf --export-pdf OUT_DIR/     # multi-input, output directory
//! h7cad A.dxf B.dxf --export-pdf                    # multi-input, inferred side-by-side
//! h7cad INPUT.dxf --export-svg OUTPUT.svg           # SVG, mirrors --export-pdf
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
    /// Export a list of input DXFs to PDF or SVG.
    Export {
        format: ExportFormat,
        inputs: Vec<PathBuf>,
        output: ExportTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Svg,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Svg => "svg",
        }
    }
    fn label(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "PDF",
            ExportFormat::Svg => "SVG",
        }
    }
}

/// Where to put each exported file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportTarget {
    /// Infer: each input's parent dir + stem + correct extension.
    SameStem,
    /// Exactly one input → this path is the output file.
    File(PathBuf),
    /// Any number of inputs → put `<stem>.<ext>` inside this directory.
    Dir(PathBuf),
}

/// Inspect `args` (already stripped of argv[0]) and return a recognised
/// batch form, or `None` if the user meant to launch the GUI.
pub fn parse_batch_args(args: &[String]) -> Option<BatchArgs> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Some(BatchArgs::Help);
    }

    let (format, flag_idx) = match args.iter().position(|a| a == "--export-pdf") {
        Some(idx) => (ExportFormat::Pdf, idx),
        None => match args.iter().position(|a| a == "--export-svg") {
            Some(idx) => (ExportFormat::Svg, idx),
            None => return None,
        },
    };

    // The arg immediately after the flag is the (optional) output path —
    // skip it when gathering inputs so we don't double-count.
    let output_idx = args
        .get(flag_idx + 1)
        .filter(|s| !s.starts_with('-'))
        .map(|_| flag_idx + 1);

    let inputs: Vec<PathBuf> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != flag_idx && Some(*i) != output_idx)
        .map(|(_, s)| s.as_str())
        .filter(|s| !s.starts_with('-'))
        .map(PathBuf::from)
        .collect();

    if inputs.is_empty() {
        return None;
    }

    let output = match output_idx {
        Some(idx) => {
            let raw = &args[idx];
            if looks_like_dir(raw) {
                ExportTarget::Dir(PathBuf::from(raw))
            } else {
                ExportTarget::File(PathBuf::from(raw))
            }
        }
        None => ExportTarget::SameStem,
    };

    Some(BatchArgs::Export {
        format,
        inputs,
        output,
    })
}

/// `true` when `raw` already exists as a directory, or ends with `/` / `\`.
fn looks_like_dir(raw: &str) -> bool {
    if raw.ends_with('/') || raw.ends_with('\\') {
        return true;
    }
    let p = Path::new(raw);
    p.is_dir()
}

/// Short help text; kept inline so the build produces a self-contained binary.
pub const HELP_TEXT: &str = "\
H7CAD — CAD viewer and DXF/DWG editor

USAGE:
    h7cad                                         Launch the GUI.
    h7cad <PATH>                                  Launch the GUI and open PATH.
    h7cad <INPUT.dxf>... --export-pdf [OUTPUT]    Batch convert DXF → PDF.
    h7cad <INPUT.dxf>... --export-svg [OUTPUT]    Batch convert DXF → SVG.
    h7cad --help                                  Show this message.

OUTPUT RESOLUTION:
    - Omitted            each INPUT's stem + .pdf / .svg beside the input
    - Ends with / or \\   treated as a directory (required for multi-input)
    - Existing directory same as above
    - Otherwise          treated as a single output file (only valid when
                         exactly one input is given)

BATCH EXPORT NOTES:
    Uses default `PdfExportOptions` / `SvgExportOptions` (monochrome, native
    curves/splines/text, solid + pattern HATCH, embedded images).
    Exit code 0 when every input succeeds, 1 when any failed.  Failures are
    non-fatal — the remaining inputs still attempt export and a per-file
    diagnostic is printed to stderr.
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
        BatchArgs::Export {
            format,
            inputs,
            output,
        } => run_export_batch(format, &inputs, &output),
    }
}

fn run_export_batch(
    format: ExportFormat,
    inputs: &[PathBuf],
    output: &ExportTarget,
) -> Result<(), String> {
    // Reject obvious misuses up front so the user gets a single clean error
    // instead of N identical "overwrite-on-same-file" diagnostics.
    if inputs.len() > 1 {
        if let ExportTarget::File(_) = output {
            return Err(format!(
                "{} inputs were given but the output \"{}\" is a single file — \
                 pass a directory (ending in '/' or '\\\\') or omit the output \
                 to infer side-by-side paths.",
                inputs.len(),
                output_display(output)
            ));
        }
    }

    let mut failed = 0usize;
    let total = inputs.len();

    for input in inputs {
        let out_path = resolve_output(input, output, format);
        match export_one(input, &out_path, format) {
            Ok(()) => {
                eprintln!(
                    "h7cad: {} -> {} ({})",
                    input.display(),
                    out_path.display(),
                    format.label()
                );
            }
            Err(e) => {
                eprintln!("h7cad: {} failed: {}", input.display(), e);
                failed += 1;
            }
        }
    }

    if failed > 0 {
        Err(format!("{failed} of {total} inputs failed"))
    } else {
        Ok(())
    }
}

fn output_display(target: &ExportTarget) -> String {
    match target {
        ExportTarget::SameStem => "<inferred>".into(),
        ExportTarget::File(p) => p.display().to_string(),
        ExportTarget::Dir(p) => p.display().to_string(),
    }
}

fn resolve_output(input: &Path, target: &ExportTarget, format: ExportFormat) -> PathBuf {
    match target {
        ExportTarget::SameStem => input.with_extension(format.extension()),
        ExportTarget::File(path) => path.clone(),
        ExportTarget::Dir(dir) => {
            let stem = input.file_stem().unwrap_or_default();
            let mut name = stem.to_string_lossy().into_owned();
            name.push('.');
            name.push_str(format.extension());
            dir.join(name)
        }
    }
}

fn export_one(input: &Path, output: &Path, format: ExportFormat) -> Result<(), String> {
    if !input.exists() {
        return Err(format!("cannot open \"{}\": file not found", input.display()));
    }

    let (compat, native, _notices) = crate::io::load_file_with_native_blocking(input)
        .map_err(|e| format!("failed to load \"{}\": {e}", input.display()))?;

    let mut scene = crate::scene::Scene::new();
    scene.document = compat;
    scene.set_native_doc(native);
    scene.native_render_enabled = false;

    let wires = scene.entity_wires();
    let (paper_w, paper_h, offset_x, offset_y) = resolve_paper_and_offset(&scene);

    // Ensure the parent directory exists for Dir-style outputs.
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "cannot create output directory \"{}\": {e}",
                    parent.display()
                )
            })?;
        }
    }

    match format {
        ExportFormat::Pdf => {
            let options = crate::io::pdf_export::PdfExportOptions::default();
            crate::io::pdf_export::export_pdf_full(
                &wires,
                &scene.hatches,
                scene.native_doc(),
                paper_w,
                paper_h,
                offset_x,
                offset_y,
                0,
                output,
                None,
                &options,
            )
            .map_err(|e| format!("PDF export failed: {e}"))?;
        }
        ExportFormat::Svg => {
            let options = crate::io::svg_export::SvgExportOptions::default();
            crate::io::svg_export::export_svg_full(
                &wires,
                &scene.hatches,
                scene.native_doc(),
                paper_w,
                paper_h,
                offset_x,
                offset_y,
                0,
                output,
                None,
                &options,
            )
            .map_err(|e| format!("SVG export failed: {e}"))?;
        }
    }

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

    fn export(format: ExportFormat, inputs: &[&str], output: ExportTarget) -> BatchArgs {
        BatchArgs::Export {
            format,
            inputs: inputs.iter().map(PathBuf::from).collect(),
            output,
        }
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
        assert_eq!(
            parse_batch_args(&s(&["input.dxf", "--export-pdf", "--help"])),
            Some(BatchArgs::Help)
        );
    }

    #[test]
    fn parse_single_input_pdf_explicit_file() {
        let got = parse_batch_args(&s(&["drawing.dxf", "--export-pdf", "out.pdf"]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Pdf,
                &["drawing.dxf"],
                ExportTarget::File(PathBuf::from("out.pdf"))
            ))
        );
    }

    #[test]
    fn parse_single_input_pdf_inferred() {
        let got = parse_batch_args(&s(&["drawing.dxf", "--export-pdf"]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Pdf,
                &["drawing.dxf"],
                ExportTarget::SameStem,
            ))
        );
    }

    #[test]
    fn parse_single_input_svg() {
        let got = parse_batch_args(&s(&["drawing.dxf", "--export-svg", "out.svg"]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Svg,
                &["drawing.dxf"],
                ExportTarget::File(PathBuf::from("out.svg"))
            ))
        );
    }

    #[test]
    fn parse_multi_input_with_dir_output_via_trailing_slash() {
        let got = parse_batch_args(&s(&[
            "a.dxf",
            "b.dxf",
            "--export-pdf",
            "out/",
        ]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Pdf,
                &["a.dxf", "b.dxf"],
                ExportTarget::Dir(PathBuf::from("out/"))
            ))
        );
    }

    #[test]
    fn parse_multi_input_no_output_uses_same_stem() {
        let got = parse_batch_args(&s(&["a.dxf", "b.dxf", "--export-svg"]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Svg,
                &["a.dxf", "b.dxf"],
                ExportTarget::SameStem,
            ))
        );
    }

    #[test]
    fn parse_accepts_flag_order_swapped() {
        let got = parse_batch_args(&s(&["--export-pdf", "out.pdf", "drawing.dxf"]));
        assert_eq!(
            got,
            Some(export(
                ExportFormat::Pdf,
                &["drawing.dxf"],
                ExportTarget::File(PathBuf::from("out.pdf"))
            ))
        );
    }

    #[test]
    fn run_batch_export_help_succeeds() {
        assert!(run_batch_export(BatchArgs::Help).is_ok());
    }

    #[test]
    fn run_batch_export_missing_file_fails() {
        let err = run_batch_export(export(
            ExportFormat::Pdf,
            &["this_definitely_does_not_exist.dxf"],
            ExportTarget::File(PathBuf::from("out.pdf")),
        ))
        .expect_err("missing input must fail");
        assert!(
            err.to_lowercase().contains("failed"),
            "expected 'failed' in error, got: {err}"
        );
    }

    #[test]
    fn run_batch_export_rejects_multi_input_to_single_file() {
        let err = run_batch_export(export(
            ExportFormat::Pdf,
            &["a.dxf", "b.dxf"],
            ExportTarget::File(PathBuf::from("merged.pdf")),
        ))
        .expect_err("multi input → single file must be rejected");
        assert!(
            err.contains("single file"),
            "expected 'single file' guidance in error, got: {err}"
        );
    }

    #[test]
    fn resolve_output_same_stem_uses_format_extension() {
        let p = resolve_output(
            Path::new("/tmp/drawing.dxf"),
            &ExportTarget::SameStem,
            ExportFormat::Pdf,
        );
        assert_eq!(p, Path::new("/tmp/drawing.pdf"));

        let s = resolve_output(
            Path::new("drawing.dxf"),
            &ExportTarget::SameStem,
            ExportFormat::Svg,
        );
        assert_eq!(s, Path::new("drawing.svg"));
    }

    #[test]
    fn resolve_output_dir_joins_stem() {
        let p = resolve_output(
            Path::new("/src/alpha.dxf"),
            &ExportTarget::Dir(PathBuf::from("/out")),
            ExportFormat::Pdf,
        );
        assert_eq!(p, Path::new("/out/alpha.pdf"));
    }
}
