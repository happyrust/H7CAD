//! Integration tests for DXF HEADER drawing identity and render metadata.
//! 2026-04-22 drawing-metadata plan (4 variables):
//!
//!   $FINGERPRINTGUID (code 2  string) permanent drawing GUID
//!   $VERSIONGUID     (code 2  string) per-save GUID
//!   $DWGCODEPAGE     (code 3  string) character code page
//!   $CSHADOW         (code 280 i16)   current-entity shadow mode

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const FP_GUID: &str = "{7A4B0E1C-6F0D-4A9B-9E3D-0123456789AB}";
const VER_GUID: &str = "{F1E2D3C4-B5A6-4978-8899-FEDCBA987654}";
const CODEPAGE: &str = "ANSI_1252";

fn dxf_with_drawing_metadata() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$FINGERPRINTGUID\n  2\n{fp}\n",
            "  9\n$VERSIONGUID\n  2\n{ver}\n",
            "  9\n$DWGCODEPAGE\n  3\n{cp}\n",
            // $CSHADOW = 2 (receives only) — chosen as a non-default
            // non-zero value to prove the field is not accidentally
            // clamped or re-interpreted as a bool.
            "  9\n$CSHADOW\n280\n2\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        fp = FP_GUID,
        ver = VER_GUID,
        cp = CODEPAGE,
    )
}

#[test]
fn header_reads_all_4_drawing_metadata_vars() {
    let doc = read_dxf(&dxf_with_drawing_metadata()).expect("parse");
    assert_eq!(doc.header.fingerprint_guid, FP_GUID);
    assert_eq!(doc.header.version_guid, VER_GUID);
    assert_eq!(doc.header.dwg_codepage, CODEPAGE);
    assert_eq!(doc.header.cshadow, 2);
}

#[test]
fn header_writes_all_4_drawing_metadata_vars() {
    let mut doc = CadDocument::new();
    doc.header.fingerprint_guid = FP_GUID.to_string();
    doc.header.version_guid = VER_GUID.to_string();
    doc.header.dwg_codepage = CODEPAGE.to_string();
    doc.header.cshadow = 3;

    let text = write_dxf(&doc).expect("write");
    for var in &[
        "$FINGERPRINTGUID",
        "$VERSIONGUID",
        "$DWGCODEPAGE",
        "$CSHADOW",
    ] {
        assert!(text.contains(var), "writer must emit {var}");
    }
    // The GUID strings themselves must land in the output verbatim —
    // catches any accidental lowercasing / brace stripping by the writer.
    assert!(text.contains(FP_GUID));
    assert!(text.contains(VER_GUID));
    assert!(text.contains(CODEPAGE));
}

#[test]
fn header_roundtrip_preserves_all_4_drawing_metadata_vars() {
    let doc1 = read_dxf(&dxf_with_drawing_metadata()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc1.header.fingerprint_guid, doc2.header.fingerprint_guid);
    assert_eq!(doc1.header.version_guid, doc2.header.version_guid);
    assert_eq!(doc1.header.dwg_codepage, doc2.header.dwg_codepage);
    assert_eq!(doc1.header.cshadow, doc2.header.cshadow);

    // Cross-check absolute values survived intact (not just equal to self).
    assert_eq!(doc2.header.fingerprint_guid, FP_GUID);
    assert_eq!(doc2.header.version_guid, VER_GUID);
    assert_eq!(doc2.header.dwg_codepage, CODEPAGE);
    assert_eq!(doc2.header.cshadow, 2);
}

#[test]
fn header_legacy_file_without_drawing_metadata_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // All four fields default to their "not authored" values — the io
    // layer does NOT synthesize GUIDs or guess a codepage.
    assert_eq!(doc.header.fingerprint_guid, "");
    assert_eq!(doc.header.version_guid, "");
    assert_eq!(doc.header.dwg_codepage, "");
    assert_eq!(doc.header.cshadow, 0);
}
