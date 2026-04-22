//! Integration tests for DXF HEADER `$REQUIREDVERSIONS` (code 160, i64).
//! 2026-04-22 required-versions plan — i64 helpers + 1 variable.
//!
//! `$REQUIREDVERSIONS` is a R2018+ bitfield marking AutoCAD features
//! a reader must support. H7CAD treats it as an opaque i64 passthrough.
//!
//! The test ground-truth value is chosen to exercise 64-bit paths:
//!
//!   0x0000_1F2E_4D5C_789A = 34_275_408_493_830_298
//!
//!   - Exceeds i32::MAX (2_147_483_647) by >7 orders of magnitude,
//!     proving the helpers are genuine i64 (not i32 promoted).
//!   - Both the high and low 32 bits are non-zero, so any 32-bit
//!     truncation bug would corrupt the value visibly.
//!   - Neither 0 nor i64::MAX, so it's clearly distinguishable from
//!     both the Default (0) and any naive sentinel.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const BIG: i64 = 0x0000_1F2E_4D5C_789A;

fn dxf_with_required_versions(value: i64) -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$REQUIREDVERSIONS\n160\n{v}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        v = value,
    )
}

#[test]
fn header_reads_required_versions() {
    let doc = read_dxf(&dxf_with_required_versions(BIG)).expect("parse");
    assert_eq!(
        doc.header.required_versions, BIG,
        "reader must preserve all 64 bits of $REQUIREDVERSIONS"
    );
    assert!(
        doc.header.required_versions > i32::MAX as i64,
        "sanity: big value must exceed i32::MAX, proving i64 path"
    );
}

#[test]
fn header_writes_required_versions() {
    let mut doc = CadDocument::new();
    doc.header.required_versions = BIG;

    let text = write_dxf(&doc).expect("write");
    assert!(
        text.contains("$REQUIREDVERSIONS"),
        "writer must emit $REQUIREDVERSIONS"
    );
    // The decimal representation must appear verbatim — not truncated,
    // not reformatted with thousands separators, not lowercased hex.
    let expected_decimal = BIG.to_string();
    assert!(
        text.contains(&expected_decimal),
        "writer must emit big value as decimal `{expected_decimal}`"
    );
}

#[test]
fn header_roundtrip_preserves_required_versions() {
    let doc1 = read_dxf(&dxf_with_required_versions(BIG)).expect("first read");
    assert_eq!(doc1.header.required_versions, BIG);

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(
        doc2.header.required_versions, BIG,
        "$REQUIREDVERSIONS must survive a full 64-bit read->write->read \
         roundtrip without bit-loss (most important test in this file)"
    );
}

#[test]
fn header_legacy_file_without_required_versions_loads_with_zero() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(
        doc.header.required_versions, 0,
        "legacy DXF without $REQUIREDVERSIONS defaults to 0 (no \
         required features, fully backward compatible)"
    );
}
