//! Integration tests for DXF HEADER UCS family expansion
//! (2026-04-21 header-ucs-family plan).

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_ucs_family() -> String {
    // Non-default UCS: origin (10, 20, 30), X-axis = world -Y,
    // Y-axis = world +X (i.e. a 90° CW rotation around Z).
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$UCSBASE\n  2\nTOP\n",
        "  9\n$UCSNAME\n  2\nRotatedUCS\n",
        "  9\n$UCSORG\n 10\n10.0\n 20\n20.0\n 30\n30.0\n",
        "  9\n$UCSXDIR\n 10\n0.0\n 20\n-1.0\n 30\n0.0\n",
        "  9\n$UCSYDIR\n 10\n1.0\n 20\n0.0\n 30\n0.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_5_ucs_vars() {
    let doc = read_dxf(&dxf_with_ucs_family()).expect("parse");
    assert_eq!(doc.header.ucsbase, "TOP");
    assert_eq!(doc.header.ucsname, "RotatedUCS");
    assert_eq!(doc.header.ucsorg, [10.0, 20.0, 30.0]);
    assert_eq!(doc.header.ucsxdir, [0.0, -1.0, 0.0]);
    assert_eq!(doc.header.ucsydir, [1.0, 0.0, 0.0]);
}

#[test]
fn header_writes_all_5_ucs_vars() {
    let mut doc = CadDocument::new();
    doc.header.ucsbase = "LEFT".into();
    doc.header.ucsname = "WorkPlaneA".into();
    doc.header.ucsorg = [5.5, -2.25, 11.0];
    doc.header.ucsxdir = [0.0, 0.0, 1.0];
    doc.header.ucsydir = [1.0, 0.0, 0.0];

    let text = write_dxf(&doc).expect("write");
    for var in &["$UCSBASE", "$UCSNAME", "$UCSORG", "$UCSXDIR", "$UCSYDIR"] {
        assert!(
            text.contains(var),
            "writer must emit {var}; got:\n{text}"
        );
    }
    assert!(text.contains("LEFT"));
    assert!(text.contains("WorkPlaneA"));
}

#[test]
fn header_roundtrip_preserves_all_5_ucs_vars() {
    let doc1 = read_dxf(&dxf_with_ucs_family()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc1.header.ucsbase, doc2.header.ucsbase);
    assert_eq!(doc1.header.ucsname, doc2.header.ucsname);
    let tol = 1e-9;
    for i in 0..3 {
        assert!(
            (doc1.header.ucsorg[i] - doc2.header.ucsorg[i]).abs() < tol,
            "ucsorg[{i}] drift: {} vs {}",
            doc1.header.ucsorg[i],
            doc2.header.ucsorg[i]
        );
        assert!((doc1.header.ucsxdir[i] - doc2.header.ucsxdir[i]).abs() < tol);
        assert!((doc1.header.ucsydir[i] - doc2.header.ucsydir[i]).abs() < tol);
    }
}

#[test]
fn header_legacy_file_without_ucs_fields_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    let def = h7cad_native_model::DocumentHeader::default();
    assert_eq!(doc.header.ucsbase, def.ucsbase);
    assert_eq!(doc.header.ucsname, def.ucsname);
    assert_eq!(doc.header.ucsorg, def.ucsorg);
    assert_eq!(doc.header.ucsxdir, def.ucsxdir);
    assert_eq!(doc.header.ucsydir, def.ucsydir);
    // And sanity: defaults match WCS-equivalent.
    assert_eq!(doc.header.ucsorg, [0.0, 0.0, 0.0]);
    assert_eq!(doc.header.ucsxdir, [1.0, 0.0, 0.0]);
    assert_eq!(doc.header.ucsydir, [0.0, 1.0, 0.0]);
}
