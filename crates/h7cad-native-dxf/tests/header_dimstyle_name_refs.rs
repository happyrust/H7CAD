//! Integration tests for DXF HEADER current dimension / dim-text style
//! name references. 2026-04-21 header-dimstyle-name-refs plan.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_name_refs() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMSTYLE\n  2\nArchitectural\n",
        "  9\n$DIMTXSTY\n  7\nArialBold\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_both_name_refs() {
    let doc = read_dxf(&dxf_with_name_refs()).expect("parse");
    assert_eq!(doc.header.dimstyle, "Architectural");
    assert_eq!(doc.header.dimtxsty, "ArialBold");
}

#[test]
fn header_writes_both_name_refs() {
    let mut doc = CadDocument::new();
    doc.header.dimstyle = "ISO-25".into();
    doc.header.dimtxsty = "RomanSimplex".into();

    let text = write_dxf(&doc).expect("write");
    for var in &["$DIMSTYLE", "$DIMTXSTY"] {
        assert!(text.contains(var), "writer must emit {var}");
    }
    assert!(text.contains("ISO-25"));
    assert!(text.contains("RomanSimplex"));
}

#[test]
fn header_roundtrip_preserves_name_refs() {
    let doc1 = read_dxf(&dxf_with_name_refs()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");
    assert_eq!(doc1.header.dimstyle, doc2.header.dimstyle);
    assert_eq!(doc1.header.dimtxsty, doc2.header.dimtxsty);
}

#[test]
fn header_legacy_file_uses_standard_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(doc.header.dimstyle, "Standard");
    assert_eq!(doc.header.dimtxsty, "Standard");
}
