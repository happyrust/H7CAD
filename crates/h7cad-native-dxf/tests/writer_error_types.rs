use h7cad_native_dxf::{write_dxf_strict, write_dxf_string, DxfWriteError};
use h7cad_native_model::CadDocument;

#[test]
fn write_dxf_strict_returns_ok_for_minimal_doc() {
    let doc = CadDocument::new();
    let result = write_dxf_strict(&doc);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("$ACADVER"));
    assert!(output.contains("EOF"));
}

#[test]
fn write_dxf_strict_matches_write_dxf_string() {
    let doc = CadDocument::new();
    let strict_output = write_dxf_strict(&doc).unwrap();
    let legacy_output = write_dxf_string(&doc).unwrap();
    assert_eq!(strict_output, legacy_output);
}

#[test]
fn dxf_write_error_display_roundtrip() {
    let err = DxfWriteError::InvalidDocument("test error".to_string());
    assert_eq!(err.to_string(), "invalid document: test error");

    let err = DxfWriteError::Unsupported("binary DXF".to_string());
    assert_eq!(err.to_string(), "unsupported: binary DXF");

    let err = DxfWriteError::Io("write failed".to_string());
    assert_eq!(err.to_string(), "io: write failed");

    let from_str: DxfWriteError = "auto wrap".into();
    assert_eq!(from_str, DxfWriteError::InvalidDocument("auto wrap".to_string()));

    let from_string: DxfWriteError = String::from("string wrap").into();
    assert_eq!(from_string, DxfWriteError::InvalidDocument("string wrap".to_string()));
}
