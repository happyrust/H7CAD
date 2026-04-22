//! Integration tests for DXF HEADER default-dimension Tier-1 variables.
//! 2026-04-21 header-dim-tier1 plan.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_dim_tier1() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMTXT\n 40\n0.5\n",
        "  9\n$DIMASZ\n 40\n0.3\n",
        "  9\n$DIMEXO\n 40\n0.125\n",
        "  9\n$DIMEXE\n 40\n0.25\n",
        "  9\n$DIMGAP\n 40\n0.15\n",
        "  9\n$DIMDEC\n 70\n     6\n",
        "  9\n$DIMADEC\n 70\n     2\n",
        "  9\n$DIMTOFL\n 70\n     1\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_8_dim_tier1_vars() {
    let doc = read_dxf(&dxf_with_dim_tier1()).expect("parse");
    assert!((doc.header.dimtxt - 0.5).abs() < 1e-12);
    assert!((doc.header.dimasz - 0.3).abs() < 1e-12);
    assert!((doc.header.dimexo - 0.125).abs() < 1e-12);
    assert!((doc.header.dimexe - 0.25).abs() < 1e-12);
    assert!((doc.header.dimgap - 0.15).abs() < 1e-12);
    assert_eq!(doc.header.dimdec, 6);
    assert_eq!(doc.header.dimadec, 2);
    assert!(doc.header.dimtofl);
}

#[test]
fn header_writes_all_8_dim_tier1_vars() {
    let mut doc = CadDocument::new();
    doc.header.dimtxt = 2.5;
    doc.header.dimasz = 1.0;
    doc.header.dimexo = 0.5;
    doc.header.dimexe = 1.0;
    doc.header.dimgap = 0.6;
    doc.header.dimdec = 3;
    doc.header.dimadec = 1;
    doc.header.dimtofl = true;

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$DIMTXT", "$DIMASZ", "$DIMEXO", "$DIMEXE", "$DIMGAP",
        "$DIMDEC", "$DIMADEC", "$DIMTOFL",
    ] {
        assert!(
            text.contains(var),
            "writer must emit {var}; got first 2 KB:\n{}",
            &text[..text.len().min(2048)]
        );
    }
}

#[test]
fn header_roundtrip_preserves_all_8_dim_tier1_vars() {
    let doc1 = read_dxf(&dxf_with_dim_tier1()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    let tol = 1e-9;
    assert!((doc1.header.dimtxt - doc2.header.dimtxt).abs() < tol);
    assert!((doc1.header.dimasz - doc2.header.dimasz).abs() < tol);
    assert!((doc1.header.dimexo - doc2.header.dimexo).abs() < tol);
    assert!((doc1.header.dimexe - doc2.header.dimexe).abs() < tol);
    assert!((doc1.header.dimgap - doc2.header.dimgap).abs() < tol);
    assert_eq!(doc1.header.dimdec, doc2.header.dimdec);
    assert_eq!(doc1.header.dimadec, doc2.header.dimadec);
    assert_eq!(doc1.header.dimtofl, doc2.header.dimtofl);
}

#[test]
fn header_legacy_file_without_dim_tier1_loads_with_imperial_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    // AutoCAD new-imperial defaults.
    assert!((doc.header.dimtxt - 0.18).abs() < 1e-12);
    assert!((doc.header.dimasz - 0.18).abs() < 1e-12);
    assert!((doc.header.dimexo - 0.0625).abs() < 1e-12);
    assert!((doc.header.dimexe - 0.18).abs() < 1e-12);
    assert!((doc.header.dimgap - 0.09).abs() < 1e-12);
    assert_eq!(doc.header.dimdec, 4);
    assert_eq!(doc.header.dimadec, 0);
    assert!(!doc.header.dimtofl);
}
