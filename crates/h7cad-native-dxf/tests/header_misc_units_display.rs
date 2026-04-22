//! Integration tests for DXF HEADER misc 5 variables (insertion units +
//! display + external-edit). 2026-04-21 header-misc-units-display plan.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_misc_vars() -> String {
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$INSUNITS\n 70\n     4\n",        // mm
        "  9\n$INSUNITSDEFSOURCE\n 70\n     1\n", // in
        "  9\n$INSUNITSDEFTARGET\n 70\n     6\n", // m
        "  9\n$LWDISPLAY\n290\n     1\n",
        "  9\n$XEDIT\n290\n     0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_5_misc_vars() {
    let doc = read_dxf(&dxf_with_misc_vars()).expect("parse");
    assert_eq!(doc.header.insunits, 4);
    assert_eq!(doc.header.insunits_def_source, 1);
    assert_eq!(doc.header.insunits_def_target, 6);
    assert!(doc.header.lwdisplay);
    assert!(!doc.header.xedit);
}

#[test]
fn header_writes_all_5_misc_vars() {
    let mut doc = CadDocument::new();
    doc.header.insunits = 5;
    doc.header.insunits_def_source = 2;
    doc.header.insunits_def_target = 4;
    doc.header.lwdisplay = true;
    doc.header.xedit = false;

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$INSUNITS",
        "$INSUNITSDEFSOURCE",
        "$INSUNITSDEFTARGET",
        "$LWDISPLAY",
        "$XEDIT",
    ] {
        assert!(text.contains(var), "writer must emit {var}");
    }
}

#[test]
fn header_roundtrip_preserves_all_5_misc_vars() {
    let doc1 = read_dxf(&dxf_with_misc_vars()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc1.header.insunits, doc2.header.insunits);
    assert_eq!(doc1.header.insunits_def_source, doc2.header.insunits_def_source);
    assert_eq!(doc1.header.insunits_def_target, doc2.header.insunits_def_target);
    assert_eq!(doc1.header.lwdisplay, doc2.header.lwdisplay);
    assert_eq!(doc1.header.xedit, doc2.header.xedit);
}

#[test]
fn header_legacy_file_without_misc_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(doc.header.insunits, 0);
    assert_eq!(doc.header.insunits_def_source, 0);
    assert_eq!(doc.header.insunits_def_target, 0);
    assert!(!doc.header.lwdisplay);
    // $XEDIT default is true (any drawing is editable as XREF unless
    // explicitly turned off).
    assert!(doc.header.xedit);
}
