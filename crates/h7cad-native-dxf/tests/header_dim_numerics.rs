//! Integration tests for DXF HEADER Tier-2 dim numerics formatting vars.
//! 2026-04-22 dim-numerics plan (6 variables):
//!
//!   $DIMRND  (code 40) rounding value
//!   $DIMLFAC (code 40) linear scale factor
//!   $DIMTDEC (code 70) tolerance decimals
//!   $DIMFRAC (code 70) fraction format
//!   $DIMDSEP (code 70) decimal separator ASCII
//!   $DIMZIN  (code 70) zero-suppression bitfield

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_dim_numerics() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMRND\n 40\n0.25\n",
        "  9\n$DIMLFAC\n 40\n2.54\n",
        "  9\n$DIMTDEC\n 70\n3\n",
        "  9\n$DIMFRAC\n 70\n1\n",
        // 44 = ',' (European decimal separator).
        "  9\n$DIMDSEP\n 70\n44\n",
        // 3 = suppress leading (bit 1) + trailing (bit 2) zeros.
        "  9\n$DIMZIN\n 70\n3\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_6_dim_numerics() {
    let doc = read_dxf(&dxf_with_dim_numerics()).expect("parse");
    assert!((doc.header.dimrnd - 0.25).abs() < 1e-12);
    assert!((doc.header.dimlfac - 2.54).abs() < 1e-12);
    assert_eq!(doc.header.dimtdec, 3);
    assert_eq!(doc.header.dimfrac, 1);
    assert_eq!(doc.header.dimdsep, 44);
    assert_eq!(doc.header.dimzin, 3);
}

#[test]
fn header_writes_all_6_dim_numerics() {
    let mut doc = CadDocument::new();
    doc.header.dimrnd = 0.5;
    doc.header.dimlfac = 25.4;
    doc.header.dimtdec = 2;
    doc.header.dimfrac = 2;
    doc.header.dimdsep = 46;
    doc.header.dimzin = 12; // bit 4 + bit 8 = suppress 0-feet and 0-inches

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$DIMRND",
        "$DIMLFAC",
        "$DIMTDEC",
        "$DIMFRAC",
        "$DIMDSEP",
        "$DIMZIN",
    ] {
        assert!(text.contains(var), "writer must emit {var}");
    }
}

#[test]
fn header_roundtrip_preserves_all_6_dim_numerics() {
    let doc1 = read_dxf(&dxf_with_dim_numerics()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert!((doc1.header.dimrnd - doc2.header.dimrnd).abs() < 1e-9);
    assert!((doc1.header.dimlfac - doc2.header.dimlfac).abs() < 1e-9);
    assert_eq!(doc1.header.dimtdec, doc2.header.dimtdec);
    assert_eq!(doc1.header.dimfrac, doc2.header.dimfrac);
    assert_eq!(doc1.header.dimdsep, doc2.header.dimdsep);
    assert_eq!(
        doc1.header.dimzin, doc2.header.dimzin,
        "$DIMZIN bitfield (3 = bit1|bit2) must survive roundtrip exactly"
    );
}

#[test]
fn header_legacy_file_without_dim_numerics_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // Defaults are NOT all zero for this group — dimlfac/dimtdec/dimdsep
    // ship with non-zero defaults that must be preserved by the Default
    // trait when the HEADER is silent.
    assert_eq!(doc.header.dimrnd, 0.0);
    assert_eq!(doc.header.dimlfac, 1.0);
    assert_eq!(doc.header.dimtdec, 4);
    assert_eq!(doc.header.dimfrac, 0);
    assert_eq!(doc.header.dimdsep, 46);
    assert_eq!(doc.header.dimzin, 0);
}
