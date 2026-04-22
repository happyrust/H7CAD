//! Integration tests for DXF HEADER `$CHAMMODE` (chamfer input mode).
//! 2026-04-22 chammode plan — 1 variable, code 70 i16.
//!
//! Values: 0 = distance-distance (uses $CHAMFERA / $CHAMFERB),
//!         1 = length-angle     (uses $CHAMFERC / $CHAMFERD).

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_chammode(mode: i16) -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$CHAMMODE\n 70\n{mode}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        mode = mode,
    )
}

#[test]
fn header_reads_chammode() {
    let doc = read_dxf(&dxf_with_chammode(1)).expect("parse length-angle mode");
    assert_eq!(
        doc.header.chammode, 1,
        "$CHAMMODE=1 must read back as length-angle mode"
    );

    let doc0 = read_dxf(&dxf_with_chammode(0)).expect("parse distance-distance mode");
    assert_eq!(
        doc0.header.chammode, 0,
        "$CHAMMODE=0 must read back as distance-distance mode"
    );
}

#[test]
fn header_writes_chammode() {
    let mut doc = CadDocument::new();
    doc.header.chammode = 1;

    let text = write_dxf(&doc).expect("write");
    assert!(
        text.contains("$CHAMMODE"),
        "writer must emit $CHAMMODE variable name"
    );

    // Verify the code-70 value that follows $CHAMMODE is exactly "1".
    // This protects against the pair being emitted under the wrong group code
    // (e.g. accidentally as code 40 float).
    let idx = text
        .find("$CHAMMODE")
        .expect("$CHAMMODE must appear in writer output");
    let after = &text[idx..];
    let code_70 = after
        .lines()
        .skip_while(|line| !line.trim().starts_with("70"))
        .nth(1)
        .map(str::trim)
        .expect("expected a code 70 / value pair right after $CHAMMODE");
    assert_eq!(
        code_70, "1",
        "$CHAMMODE value must follow the '70' group code"
    );
}

#[test]
fn header_roundtrip_preserves_chammode() {
    let doc1 = read_dxf(&dxf_with_chammode(1)).expect("first read");
    assert_eq!(doc1.header.chammode, 1);

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(
        doc2.header.chammode, 1,
        "chammode must survive a full read → write → read roundtrip"
    );
}

#[test]
fn header_legacy_file_without_chammode_loads_with_zero() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(
        doc.header.chammode, 0,
        "legacy DXF without $CHAMMODE must default to 0 (distance-distance)"
    );
}
