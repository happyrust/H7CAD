//! Integration tests for DXF HEADER active-view variables
//! (2026-04-21 header-view-vars plan).

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_view_vars() -> String {
    // Non-default view: centred at (100, 200), 42.5 world-height,
    // view direction along the [1, 1, 1] diagonal (not yet normalised;
    // the reader/writer pair is expected to passthrough raw values).
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$VIEWCTR\n 10\n100.0\n 20\n200.0\n",
        "  9\n$VIEWSIZE\n 40\n42.5\n",
        "  9\n$VIEWDIR\n 10\n1.0\n 20\n1.0\n 30\n1.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_3_view_vars() {
    let doc = read_dxf(&dxf_with_view_vars()).expect("parse");
    assert_eq!(doc.header.viewctr, [100.0, 200.0]);
    assert!((doc.header.viewsize - 42.5).abs() < 1e-12);
    assert_eq!(doc.header.viewdir, [1.0, 1.0, 1.0]);
}

#[test]
fn header_writes_all_3_view_vars() {
    let mut doc = CadDocument::new();
    doc.header.viewctr = [-50.0, 75.5];
    doc.header.viewsize = 1000.0;
    doc.header.viewdir = [0.0, -1.0, 0.0];

    let text = write_dxf(&doc).expect("write");
    for var in &["$VIEWCTR", "$VIEWSIZE", "$VIEWDIR"] {
        assert!(
            text.contains(var),
            "writer must emit {var}; got:\n{text}"
        );
    }
}

#[test]
fn header_roundtrip_preserves_all_3_view_vars() {
    let doc1 = read_dxf(&dxf_with_view_vars()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    let tol = 1e-9;
    assert!((doc1.header.viewctr[0] - doc2.header.viewctr[0]).abs() < tol);
    assert!((doc1.header.viewctr[1] - doc2.header.viewctr[1]).abs() < tol);
    assert!((doc1.header.viewsize - doc2.header.viewsize).abs() < tol);
    for i in 0..3 {
        assert!((doc1.header.viewdir[i] - doc2.header.viewdir[i]).abs() < tol);
    }
}

#[test]
fn header_legacy_file_without_view_fields_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    let def = h7cad_native_model::DocumentHeader::default();
    assert_eq!(doc.header.viewctr, def.viewctr);
    assert_eq!(doc.header.viewsize, def.viewsize);
    assert_eq!(doc.header.viewdir, def.viewdir);
    // Default view is the top-down plan view.
    assert_eq!(doc.header.viewctr, [0.0, 0.0]);
    assert_eq!(doc.header.viewsize, 1.0);
    assert_eq!(doc.header.viewdir, [0.0, 0.0, 1.0]);
}
