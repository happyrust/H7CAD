//! Integration tests for DXF HEADER timestamp variables.
//!
//! Part of the 2026-04-21 header-timestamps plan. The four variables
//! (`$TDCREATE` / `$TDUPDATE` / `$TDINDWG` / `$TDUSRTIMER`) are all
//! DXF code 40 f64 passthroughs — H7CAD does **not** do a Julian-date
//! → `DateTime` conversion in-core to keep the dependency tree lean,
//! so every test here just checks the raw numeric value.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

fn dxf_with_timestamps() -> String {
    // Example Julian dates from the AutoCAD 2018 DXF Reference:
    //  - 2458849.82939815 ≈ 2020-01-01 07:54:19 UTC
    //  - 2458849.89444444 ≈ 2020-01-01 09:27:59 UTC (update time)
    //  - 0.00125 fractional days ≈ 1 min 48 sec editing time
    //  - 0.00034 fractional days ≈ 29.4 sec user timer
    concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$TDCREATE\n 40\n2458849.82939815\n",
        "  9\n$TDUPDATE\n 40\n2458849.89444444\n",
        "  9\n$TDINDWG\n 40\n0.00125\n",
        "  9\n$TDUSRTIMER\n 40\n0.00034\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    )
    .to_string()
}

#[test]
fn header_reads_all_4_timestamps() {
    let doc = read_dxf(&dxf_with_timestamps()).expect("parse");
    assert!((doc.header.tdcreate - 2458849.82939815).abs() < 1e-9);
    assert!((doc.header.tdupdate - 2458849.89444444).abs() < 1e-9);
    assert!((doc.header.tdindwg - 0.00125).abs() < 1e-9);
    assert!((doc.header.tdusrtimer - 0.00034).abs() < 1e-9);
}

#[test]
fn header_writes_all_4_timestamps() {
    let mut doc = CadDocument::new();
    doc.header.tdcreate = 2460000.123456789;
    doc.header.tdupdate = 2460001.987654321;
    doc.header.tdindwg = 0.5;
    doc.header.tdusrtimer = 0.25;

    let text = write_dxf(&doc).expect("write");
    for var in &["$TDCREATE", "$TDUPDATE", "$TDINDWG", "$TDUSRTIMER"] {
        assert!(
            text.contains(var),
            "writer must emit {var} HEADER variable; got:\n{text}"
        );
    }
}

#[test]
fn header_roundtrip_preserves_all_4_timestamps() {
    let doc1 = read_dxf(&dxf_with_timestamps()).expect("first read");
    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    // format_f64 caps at 10 decimal digits → Julian-date values of
    // magnitude ~2.4e6 have a precision floor near 2.4e-4, i.e. about
    // 20 seconds of wall-clock time. Tolerance = 1e-3 absorbs that
    // while still catching a gross regression (e.g. field swap).
    let tol_julian = 1e-3;
    // Fractional-day values (TDINDWG / TDUSRTIMER) are ≤ 1, so the
    // 10-digit ceiling keeps precision near 1e-10 — tolerate 1e-9.
    let tol_days = 1e-9;

    assert!((doc1.header.tdcreate - doc2.header.tdcreate).abs() < tol_julian);
    assert!((doc1.header.tdupdate - doc2.header.tdupdate).abs() < tol_julian);
    assert!((doc1.header.tdindwg - doc2.header.tdindwg).abs() < tol_days);
    assert!((doc1.header.tdusrtimer - doc2.header.tdusrtimer).abs() < tol_days);
}

#[test]
fn header_legacy_file_without_td_fields_loads_with_zero() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");
    assert_eq!(doc.header.tdcreate, 0.0);
    assert_eq!(doc.header.tdupdate, 0.0);
    assert_eq!(doc.header.tdindwg, 0.0);
    assert_eq!(doc.header.tdusrtimer, 0.0);
}
