//! Integration tests for the DXF HEADER display & render flag family
//! (`$DISPSILH / $DRAGMODE / $REGENMODE / $SHADEDGE / $SHADEDIF`).
//! 2026-04-22 display-render-family plan — 5 variables, all code 70 /
//! `i16`, nominally the "default 3D viewport & shading behaviour"
//! passthrough.
//!
//! Ground-truth values differ from each field's AutoCAD default so an
//! arm-wiring mix-up would mis-route at least two fields and trip two
//! assertions:
//!
//!   dispsilh  = 1  (default 0)
//!   dragmode  = 0  (default 2)
//!   regenmode = 0  (default 1)
//!   shadedge  = 1  (default 3)
//!   shadedif  = 50 (default 70)

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const DISPSILH: i16 = 1;
const DRAGMODE: i16 = 0;
const REGENMODE: i16 = 0;
const SHADEDGE: i16 = 1;
const SHADEDIF: i16 = 50;

fn dxf_with_display_render_family() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$DISPSILH\n 70\n{ds}\n",
            "  9\n$DRAGMODE\n 70\n{dm}\n",
            "  9\n$REGENMODE\n 70\n{rm}\n",
            "  9\n$SHADEDGE\n 70\n{se}\n",
            "  9\n$SHADEDIF\n 70\n{sd}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        ds = DISPSILH,
        dm = DRAGMODE,
        rm = REGENMODE,
        se = SHADEDGE,
        sd = SHADEDIF,
    )
}

#[test]
fn header_reads_display_render_family() {
    let doc = read_dxf(&dxf_with_display_render_family()).expect("parse");
    assert_eq!(doc.header.dispsilh, DISPSILH, "$DISPSILH");
    assert_eq!(doc.header.dragmode, DRAGMODE, "$DRAGMODE");
    assert_eq!(doc.header.regenmode, REGENMODE, "$REGENMODE");
    assert_eq!(doc.header.shadedge, SHADEDGE, "$SHADEDGE");
    assert_eq!(doc.header.shadedif, SHADEDIF, "$SHADEDIF");
}

#[test]
fn header_writes_display_render_family() {
    let mut doc = CadDocument::new();
    doc.header.dispsilh = DISPSILH;
    doc.header.dragmode = DRAGMODE;
    doc.header.regenmode = REGENMODE;
    doc.header.shadedge = SHADEDGE;
    doc.header.shadedif = SHADEDIF;

    let text = write_dxf(&doc).expect("write");

    for name in ["$DISPSILH", "$DRAGMODE", "$REGENMODE", "$SHADEDGE", "$SHADEDIF"] {
        assert!(text.contains(name), "writer must emit {name}");
    }

    // Emission order must match reader arm order for deterministic
    // HEADER layout + roundtrip stability.
    let order = ["$DISPSILH", "$DRAGMODE", "$REGENMODE", "$SHADEDGE", "$SHADEDIF"];
    let mut cursor = 0usize;
    for name in order {
        let hit = text[cursor..]
            .find(name)
            .unwrap_or_else(|| panic!("{name} not found in expected order"));
        cursor += hit + name.len();
    }
}

#[test]
fn header_roundtrip_preserves_display_render_family() {
    let doc1 = read_dxf(&dxf_with_display_render_family()).expect("first read");

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc2.header.dispsilh, DISPSILH);
    assert_eq!(doc2.header.dragmode, DRAGMODE);
    assert_eq!(doc2.header.regenmode, REGENMODE);
    assert_eq!(doc2.header.shadedge, SHADEDGE);
    assert_eq!(doc2.header.shadedif, SHADEDIF);
}

#[test]
fn header_legacy_file_without_display_render_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // AutoCAD factory defaults (documented in DocumentHeader::default).
    // Any change to these must be synced with the plan doc's §"默认值
    // 选型" section AND this test.
    assert_eq!(doc.header.dispsilh, 0);
    assert_eq!(doc.header.dragmode, 2);
    assert_eq!(doc.header.regenmode, 1);
    assert_eq!(doc.header.shadedge, 3);
    assert_eq!(doc.header.shadedif, 70);
}
