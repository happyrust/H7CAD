//! Integration tests for DXF HEADER Spline + MLine 6 variables.
//! 2026-04-21 header-spline-mline plan.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_spline_mline() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$SPLFRAME\n 70\n     1\n",
        "  9\n$SPLINESEGS\n 70\n    24\n",
        "  9\n$SPLINETYPE\n 70\n     5\n",
        "  9\n$CMLSTYLE\n  2\nDoublePipe\n",
        "  9\n$CMLJUST\n 70\n     2\n",
        "  9\n$CMLSCALE\n 40\n2.5\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_6_spline_mline_vars() {
    let doc = read_dxf(&dxf_with_spline_mline()).expect("parse");
    assert!(doc.header.splframe);
    assert_eq!(doc.header.splinesegs, 24);
    assert_eq!(doc.header.splinetype, 5);
    assert_eq!(doc.header.cmlstyle, "DoublePipe");
    assert_eq!(doc.header.cmljust, 2);
    assert!((doc.header.cmlscale - 2.5).abs() < 1e-12);
}

#[test]
fn header_writes_all_6_spline_mline_vars() {
    let mut doc = CadDocument::new();
    doc.header.splframe = true;
    doc.header.splinesegs = 16;
    doc.header.splinetype = 5;
    doc.header.cmlstyle = "TriplePipe".into();
    doc.header.cmljust = 1;
    doc.header.cmlscale = 0.5;

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$SPLFRAME", "$SPLINESEGS", "$SPLINETYPE",
        "$CMLSTYLE", "$CMLJUST", "$CMLSCALE",
    ] {
        assert!(text.contains(var), "writer must emit {var}");
    }
    assert!(text.contains("TriplePipe"));
}

#[test]
fn header_roundtrip_preserves_all_6_spline_mline_vars() {
    let doc1 = read_dxf(&dxf_with_spline_mline()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc1.header.splframe, doc2.header.splframe);
    assert_eq!(doc1.header.splinesegs, doc2.header.splinesegs);
    assert_eq!(doc1.header.splinetype, doc2.header.splinetype);
    assert_eq!(doc1.header.cmlstyle, doc2.header.cmlstyle);
    assert_eq!(doc1.header.cmljust, doc2.header.cmljust);
    assert!((doc1.header.cmlscale - doc2.header.cmlscale).abs() < 1e-9);
}

#[test]
fn header_legacy_file_without_spline_mline_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert!(!doc.header.splframe);
    assert_eq!(doc.header.splinesegs, 8);
    assert_eq!(doc.header.splinetype, 6);
    assert_eq!(doc.header.cmlstyle, "Standard");
    assert_eq!(doc.header.cmljust, 0);
    assert!((doc.header.cmlscale - 1.0).abs() < 1e-12);
}
