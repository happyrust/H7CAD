//! Integration tests for DXF HEADER Chamfer / Fillet / 2.5-D defaults.
//! 2026-04-22 header-geom-defaults plan (7 variables, all code 40 f64).

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_geom_defaults() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$CHAMFERA\n 40\n1.25\n",
        "  9\n$CHAMFERB\n 40\n0.75\n",
        "  9\n$CHAMFERC\n 40\n2.0\n",
        "  9\n$CHAMFERD\n 40\n45.0\n",
        "  9\n$FILLETRAD\n 40\n0.5\n",
        "  9\n$ELEVATION\n 40\n10.0\n",
        "  9\n$THICKNESS\n 40\n3.14\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_7_geom_default_vars() {
    let doc = read_dxf(&dxf_with_geom_defaults()).expect("parse");
    assert!((doc.header.chamfera - 1.25).abs() < 1e-12);
    assert!((doc.header.chamferb - 0.75).abs() < 1e-12);
    assert!((doc.header.chamferc - 2.0).abs() < 1e-12);
    assert!((doc.header.chamferd - 45.0).abs() < 1e-12);
    assert!((doc.header.filletrad - 0.5).abs() < 1e-12);
    assert!((doc.header.elevation - 10.0).abs() < 1e-12);
    assert!((doc.header.thickness - 3.14).abs() < 1e-12);
}

#[test]
fn header_writes_all_7_geom_default_vars() {
    let mut doc = CadDocument::new();
    doc.header.chamfera = 2.5;
    doc.header.chamferb = 1.5;
    doc.header.chamferc = 4.0;
    doc.header.chamferd = 30.0;
    doc.header.filletrad = 1.0;
    doc.header.elevation = -5.0;
    doc.header.thickness = 2.71;

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$CHAMFERA",
        "$CHAMFERB",
        "$CHAMFERC",
        "$CHAMFERD",
        "$FILLETRAD",
        "$ELEVATION",
        "$THICKNESS",
    ] {
        assert!(text.contains(var), "writer must emit {var}");
    }
}

#[test]
fn header_roundtrip_preserves_all_7_geom_default_vars() {
    let doc1 = read_dxf(&dxf_with_geom_defaults()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert!((doc1.header.chamfera - doc2.header.chamfera).abs() < 1e-9);
    assert!((doc1.header.chamferb - doc2.header.chamferb).abs() < 1e-9);
    assert!((doc1.header.chamferc - doc2.header.chamferc).abs() < 1e-9);
    assert!((doc1.header.chamferd - doc2.header.chamferd).abs() < 1e-9);
    assert!((doc1.header.filletrad - doc2.header.filletrad).abs() < 1e-9);
    assert!((doc1.header.elevation - doc2.header.elevation).abs() < 1e-9);
    assert!((doc1.header.thickness - doc2.header.thickness).abs() < 1e-9);
}

#[test]
fn header_legacy_file_without_geom_defaults_loads_with_zeros() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(doc.header.chamfera, 0.0);
    assert_eq!(doc.header.chamferb, 0.0);
    assert_eq!(doc.header.chamferc, 0.0);
    assert_eq!(doc.header.chamferd, 0.0);
    assert_eq!(doc.header.filletrad, 0.0);
    assert_eq!(doc.header.elevation, 0.0);
    assert_eq!(doc.header.thickness, 0.0);
}
