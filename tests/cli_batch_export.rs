//! Integration tests for the headless CLI batch export path (三十六轮).
//!
//! These tests invoke the compiled `h7cad` binary via
//! `CARGO_BIN_EXE_H7CAD` and drive it with real command-line arguments,
//! so we verify the full main → cli::parse_batch_args → run_batch_export
//! pipeline end-to-end, not just the unit tests inside `src/cli.rs`.

use std::process::{Command, Stdio};

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_H7CAD")
}

#[test]
fn cli_help_flag_returns_zero_and_prints_usage() {
    let output = Command::new(binary_path())
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad --help");

    assert!(
        output.status.success(),
        "exit code: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("H7CAD") && stdout.contains("--export-pdf"),
        "expected usage text, got: {stdout}"
    );
}

#[test]
fn cli_missing_input_returns_nonzero() {
    let output = Command::new(binary_path())
        .arg("this_definitely_does_not_exist_1234.dxf")
        .arg("--export-pdf")
        .arg("out.pdf")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad for missing-input case");

    assert!(
        !output.status.success(),
        "expected non-zero exit for missing input; got stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("cannot open") || stderr.contains("not found"),
        "expected 'cannot open'/'not found' in stderr, got: {stderr}"
    );
}

#[test]
fn cli_writes_pdf_for_minimal_dxf() {
    use std::fs;

    // Create a minimal DXF containing a single LINE entity directly via
    // the h7cad-native-dxf writer so we don't depend on GUI fixtures.
    let mut doc = h7cad_native_model::CadDocument::new();
    let _line_h = doc
        .add_entity(h7cad_native_model::Entity::new(
            h7cad_native_model::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [100.0, 50.0, 0.0],
            },
        ))
        .expect("add line");

    let dxf_str = h7cad_native_dxf::write_dxf_string(&doc).expect("write DXF");

    let tmp = std::env::temp_dir();
    let input_path = tmp.join(format!("h7cad_cli_test_{}_input.dxf", std::process::id()));
    let output_path = tmp.join(format!("h7cad_cli_test_{}_out.pdf", std::process::id()));
    fs::write(&input_path, &dxf_str).expect("write input DXF");
    let _ = fs::remove_file(&output_path); // clean prior runs

    let output = Command::new(binary_path())
        .arg(&input_path)
        .arg("--export-pdf")
        .arg(&output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad for export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0; stdout={} stderr={stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    let bytes = fs::read(&output_path).expect("output pdf should exist");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "expected PDF magic header, got {:?}",
        &bytes[..8.min(bytes.len())]
    );
    assert!(
        bytes.len() > 500,
        "empty-looking PDF ({} bytes), stderr: {stderr}",
        bytes.len()
    );

    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&output_path);
}

#[test]
fn cli_exports_svg_for_minimal_dxf() {
    use std::fs;

    let mut doc = h7cad_native_model::CadDocument::new();
    let _ = doc
        .add_entity(h7cad_native_model::Entity::new(
            h7cad_native_model::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [50.0, 25.0, 0.0],
            },
        ))
        .expect("add line");
    let dxf_str = h7cad_native_dxf::write_dxf_string(&doc).expect("write DXF");

    let tmp = std::env::temp_dir();
    let input = tmp.join(format!("h7cad_cli_svg_{}_in.dxf", std::process::id()));
    let output = tmp.join(format!("h7cad_cli_svg_{}_out.svg", std::process::id()));
    fs::write(&input, &dxf_str).expect("write input");
    let _ = fs::remove_file(&output);

    let out = Command::new(binary_path())
        .arg(&input)
        .arg("--export-svg")
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad --export-svg");

    assert!(
        out.status.success(),
        "expected exit 0 for svg export; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = fs::read(&output).expect("output svg should exist");
    // SVG magic: starts with either "<?xml" or "<svg".
    let head = String::from_utf8_lossy(&bytes[..32.min(bytes.len())]);
    assert!(
        head.starts_with("<?xml") || head.starts_with("<svg"),
        "expected SVG header, got: {head}"
    );
    assert!(
        bytes.windows(5).any(|w| w == b"<svg "),
        "expected <svg element in output"
    );

    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_batch_two_dxfs_to_dir() {
    use std::fs;

    let mut doc = h7cad_native_model::CadDocument::new();
    let _ = doc.add_entity(h7cad_native_model::Entity::new(
        h7cad_native_model::EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [10.0, 0.0, 0.0],
        },
    ));
    let dxf_str = h7cad_native_dxf::write_dxf_string(&doc).expect("write dxf");

    let pid = std::process::id();
    let tmp = std::env::temp_dir();
    let input_a = tmp.join(format!("h7cad_batch_{pid}_a.dxf"));
    let input_b = tmp.join(format!("h7cad_batch_{pid}_b.dxf"));
    let out_dir = tmp.join(format!("h7cad_batch_{pid}_out"));
    fs::write(&input_a, &dxf_str).expect("write a");
    fs::write(&input_b, &dxf_str).expect("write b");
    let _ = fs::remove_dir_all(&out_dir); // clean prior runs

    // Pass output as a path ending in platform separator so it's detected
    // as a directory even before it exists.
    let out_arg = format!("{}\\", out_dir.display());

    let out = Command::new(binary_path())
        .arg(&input_a)
        .arg(&input_b)
        .arg("--export-pdf")
        .arg(&out_arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad batch");

    assert!(
        out.status.success(),
        "expected exit 0 for batch; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let expect_a = out_dir.join(format!("h7cad_batch_{pid}_a.pdf"));
    let expect_b = out_dir.join(format!("h7cad_batch_{pid}_b.pdf"));
    assert!(expect_a.exists(), "missing {}", expect_a.display());
    assert!(expect_b.exists(), "missing {}", expect_b.display());

    let _ = fs::remove_file(&input_a);
    let _ = fs::remove_file(&input_b);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_mixed_failure_keeps_processing_and_reports_nonzero() {
    use std::fs;

    let mut doc = h7cad_native_model::CadDocument::new();
    let _ = doc.add_entity(h7cad_native_model::Entity::new(
        h7cad_native_model::EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [5.0, 0.0, 0.0],
        },
    ));
    let dxf_str = h7cad_native_dxf::write_dxf_string(&doc).expect("write dxf");

    let pid = std::process::id();
    let tmp = std::env::temp_dir();
    let good_input = tmp.join(format!("h7cad_mixed_{pid}_good.dxf"));
    let bad_input = tmp.join(format!("h7cad_mixed_{pid}_nonexistent.dxf"));
    let out_dir = tmp.join(format!("h7cad_mixed_{pid}_out"));
    fs::write(&good_input, &dxf_str).expect("write good");
    let _ = fs::remove_dir_all(&out_dir);

    let out_arg = format!("{}\\", out_dir.display());
    let out = Command::new(binary_path())
        .arg(&good_input)
        .arg(&bad_input)
        .arg("--export-pdf")
        .arg(&out_arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad mixed");

    assert!(
        !out.status.success(),
        "expected non-zero exit when one input fails"
    );
    // Good one should still have produced output.
    let good_output = out_dir.join(format!("h7cad_mixed_{pid}_good.pdf"));
    assert!(
        good_output.exists(),
        "good input should still be exported alongside the failing one"
    );

    let _ = fs::remove_file(&good_input);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_infers_output_path_from_input_stem() {
    use std::fs;

    let mut doc = h7cad_native_model::CadDocument::new();
    let _ = doc
        .add_entity(h7cad_native_model::Entity::new(
            h7cad_native_model::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            },
        ))
        .expect("add line");
    let dxf_str = h7cad_native_dxf::write_dxf_string(&doc).expect("write DXF");

    let tmp = std::env::temp_dir();
    let input_path = tmp.join(format!("h7cad_cli_infer_{}_in.dxf", std::process::id()));
    let expected_output = input_path.with_extension("pdf");
    fs::write(&input_path, &dxf_str).expect("write input DXF");
    let _ = fs::remove_file(&expected_output);

    let output = Command::new(binary_path())
        .arg(&input_path)
        .arg("--export-pdf")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn h7cad with inferred output");

    assert!(output.status.success(), "expected exit 0 for inferred output");
    let bytes = fs::read(&expected_output).expect("inferred output pdf should exist");
    assert!(bytes.starts_with(b"%PDF-"));

    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&expected_output);
}
