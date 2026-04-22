//! Integration tests for the DXF HEADER SNAP / GRID geometry family
//! (`$SNAPBASE / $SNAPUNIT / $SNAPSTYLE / $SNAPANG / $SNAPISOPAIR /
//! $GRIDUNIT`). 2026-04-22 snap-grid-family plan — 6 variables in one
//! round, companion to the existing `snapmode / gridmode / orthomode`
//! bool triplet.
//!
//! Ground-truth values are chosen so **each field can fail
//! independently** without being masked by another:
//!
//!   snap_base    = [3.25, -7.125]   // non-origin, negative, fractional
//!   snap_unit    = [0.25, 0.5]      // x ≠ y (catches 10/20 column swap)
//!   snap_style   = 1                // isometric (distinct from default 0)
//!   snap_ang     = π/4 ≈ 0.7853981633974483  // distinct from default 0.0
//!                                   // and exercises format_f64 precision
//!   snap_iso_pair = 2               // right iso plane (distinct from 0)
//!   grid_unit    = [1.0, 2.0]       // x ≠ y AND ≠ snap_unit
//!                                   // (catches snap/grid mix-up)

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const SNAP_BASE: [f64; 2] = [3.25, -7.125];
const SNAP_UNIT: [f64; 2] = [0.25, 0.5];
const SNAP_STYLE: i16 = 1;
const SNAP_ANG: f64 = std::f64::consts::FRAC_PI_4;
const SNAP_ISO_PAIR: i16 = 2;
const GRID_UNIT: [f64; 2] = [1.0, 2.0];

fn dxf_with_snap_grid_family() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$SNAPBASE\n 10\n{sbx}\n 20\n{sby}\n",
            "  9\n$SNAPUNIT\n 10\n{sux}\n 20\n{suy}\n",
            "  9\n$SNAPSTYLE\n 70\n{sst}\n",
            "  9\n$SNAPANG\n 50\n{san}\n",
            "  9\n$SNAPISOPAIR\n 70\n{sip}\n",
            "  9\n$GRIDUNIT\n 10\n{gux}\n 20\n{guy}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        sbx = SNAP_BASE[0],
        sby = SNAP_BASE[1],
        sux = SNAP_UNIT[0],
        suy = SNAP_UNIT[1],
        sst = SNAP_STYLE,
        san = SNAP_ANG,
        sip = SNAP_ISO_PAIR,
        gux = GRID_UNIT[0],
        guy = GRID_UNIT[1],
    )
}

#[test]
fn header_reads_snap_grid_family() {
    let doc = read_dxf(&dxf_with_snap_grid_family()).expect("parse");

    assert_eq!(doc.header.snap_base, SNAP_BASE, "$SNAPBASE");
    assert_eq!(doc.header.snap_unit, SNAP_UNIT, "$SNAPUNIT");
    assert_ne!(
        doc.header.snap_unit[0], doc.header.snap_unit[1],
        "$SNAPUNIT x ≠ y — regression guard for code 10/20 column swap"
    );
    assert_eq!(doc.header.snap_style, SNAP_STYLE, "$SNAPSTYLE");
    assert_eq!(doc.header.snap_ang, SNAP_ANG, "$SNAPANG (π/4 radians)");
    assert_eq!(doc.header.snap_iso_pair, SNAP_ISO_PAIR, "$SNAPISOPAIR");
    assert_eq!(doc.header.grid_unit, GRID_UNIT, "$GRIDUNIT");
    assert_ne!(
        doc.header.grid_unit, doc.header.snap_unit,
        "grid_unit ≠ snap_unit — regression guard for snap/grid mix-up"
    );
}

#[test]
fn header_writes_snap_grid_family() {
    let mut doc = CadDocument::new();
    doc.header.snap_base = SNAP_BASE;
    doc.header.snap_unit = SNAP_UNIT;
    doc.header.snap_style = SNAP_STYLE;
    doc.header.snap_ang = SNAP_ANG;
    doc.header.snap_iso_pair = SNAP_ISO_PAIR;
    doc.header.grid_unit = GRID_UNIT;

    let text = write_dxf(&doc).expect("write");

    for name in [
        "$SNAPBASE",
        "$SNAPUNIT",
        "$SNAPSTYLE",
        "$SNAPANG",
        "$SNAPISOPAIR",
        "$GRIDUNIT",
    ] {
        assert!(text.contains(name), "writer must emit {name}");
    }

    // Emission order must match reader arm order so HEADER layout stays
    // deterministic and roundtrip-stable. Finding all 6 in order is a
    // loose but sufficient check.
    let order = [
        "$SNAPBASE",
        "$SNAPUNIT",
        "$SNAPSTYLE",
        "$SNAPANG",
        "$SNAPISOPAIR",
        "$GRIDUNIT",
    ];
    let mut cursor = 0usize;
    for name in order {
        let hit = text[cursor..]
            .find(name)
            .unwrap_or_else(|| panic!("{name} not found in expected order"));
        cursor += hit + name.len();
    }
}

#[test]
fn header_roundtrip_preserves_snap_grid_family() {
    let doc1 = read_dxf(&dxf_with_snap_grid_family()).expect("first read");

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc2.header.snap_base, SNAP_BASE, "$SNAPBASE roundtrip");
    assert_eq!(doc2.header.snap_unit, SNAP_UNIT, "$SNAPUNIT roundtrip");
    assert_eq!(doc2.header.snap_style, SNAP_STYLE, "$SNAPSTYLE roundtrip");
    assert_eq!(
        doc2.header.snap_ang, SNAP_ANG,
        "$SNAPANG π/4 must survive format_f64 {{:.10}} → parse without \
         drift (if this fails, `format_f64` precision is the bug, not \
         this test)"
    );
    assert_eq!(
        doc2.header.snap_iso_pair, SNAP_ISO_PAIR,
        "$SNAPISOPAIR roundtrip"
    );
    assert_eq!(doc2.header.grid_unit, GRID_UNIT, "$GRIDUNIT roundtrip");
}

#[test]
fn header_legacy_file_without_snap_grid_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // Defaults from DocumentHeader::default() — AutoCAD imperial template
    // baseline. Any change to these defaults must be synced with the
    // plan doc's "默认值选型" section.
    assert_eq!(doc.header.snap_base, [0.0, 0.0]);
    assert_eq!(doc.header.snap_unit, [0.5, 0.5]);
    assert_eq!(doc.header.snap_style, 0);
    assert_eq!(doc.header.snap_ang, 0.0);
    assert_eq!(doc.header.snap_iso_pair, 0);
    assert_eq!(doc.header.grid_unit, [0.5, 0.5]);
}
