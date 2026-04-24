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
