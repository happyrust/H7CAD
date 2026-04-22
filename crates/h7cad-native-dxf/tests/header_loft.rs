//! Integration tests for the DXF HEADER LOFT 3D defaults family
//! (`$LOFTANG1 / $LOFTANG2 / $LOFTMAG1 / $LOFTMAG2 / $LOFTNORMALS /
//! $LOFTPARAM`). 2026-04-22 loft-family plan — 6 variables, 4 × f64 +
//! 2 × i16, driving AutoCAD R2007+ LOFT command defaults.
//!
//! Ground-truth values pick common radian constants (π/6, π/3) for the
//! draft angles — exercising `format_f64`'s shortest round-trip
//! behaviour introduced in round 25 — and mutually distinct magnitudes
//! (1.5, 2.5) so any arm-wiring slip between `loft_ang1 / 2` or
//! `loft_mag1 / 2` would produce a value collision and trip the test.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const LOFT_ANG1: f64 = std::f64::consts::FRAC_PI_6;  // 30°
const LOFT_ANG2: f64 = std::f64::consts::FRAC_PI_3;  // 60°
const LOFT_MAG1: f64 = 1.5;
const LOFT_MAG2: f64 = 2.5;
const LOFT_NORMALS: i16 = 6;  // path-normals (default is 1 = smooth-fit)
const LOFT_PARAM: i16 = 9;    // bit 1 (no twist) + bit 8 (closed)

fn dxf_with_loft_family() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$LOFTANG1\n 40\n{a1}\n",
            "  9\n$LOFTANG2\n 40\n{a2}\n",
            "  9\n$LOFTMAG1\n 40\n{m1}\n",
            "  9\n$LOFTMAG2\n 40\n{m2}\n",
            "  9\n$LOFTNORMALS\n 70\n{n}\n",
            "  9\n$LOFTPARAM\n 70\n{p}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        a1 = LOFT_ANG1,
        a2 = LOFT_ANG2,
        m1 = LOFT_MAG1,
        m2 = LOFT_MAG2,
        n = LOFT_NORMALS,
        p = LOFT_PARAM,
    )
}

#[test]
fn header_reads_loft_family() {
    let doc = read_dxf(&dxf_with_loft_family()).expect("parse");

    assert_eq!(doc.header.loft_ang1, LOFT_ANG1, "$LOFTANG1 (π/6)");
    assert_eq!(doc.header.loft_ang2, LOFT_ANG2, "$LOFTANG2 (π/3)");
    assert_eq!(doc.header.loft_mag1, LOFT_MAG1, "$LOFTMAG1");
    assert_eq!(doc.header.loft_mag2, LOFT_MAG2, "$LOFTMAG2");
    assert_ne!(
        doc.header.loft_ang1, doc.header.loft_ang2,
        "loft_ang1 / 2 must not collide — both share DXF code 40 and \
         arm-wiring regressions would surface here first"
    );
    assert_ne!(
        doc.header.loft_mag1, doc.header.loft_mag2,
        "loft_mag1 / 2 must not collide — same rationale as the ang1/2 \
         guard above"
    );
    assert_eq!(doc.header.loft_normals, LOFT_NORMALS, "$LOFTNORMALS");
    assert_eq!(doc.header.loft_param, LOFT_PARAM, "$LOFTPARAM");
}

#[test]
fn header_writes_loft_family() {
    let mut doc = CadDocument::new();
    doc.header.loft_ang1 = LOFT_ANG1;
    doc.header.loft_ang2 = LOFT_ANG2;
    doc.header.loft_mag1 = LOFT_MAG1;
    doc.header.loft_mag2 = LOFT_MAG2;
    doc.header.loft_normals = LOFT_NORMALS;
    doc.header.loft_param = LOFT_PARAM;

    let text = write_dxf(&doc).expect("write");

    for name in [
        "$LOFTANG1",
        "$LOFTANG2",
        "$LOFTMAG1",
        "$LOFTMAG2",
        "$LOFTNORMALS",
        "$LOFTPARAM",
    ] {
        assert!(text.contains(name), "writer must emit {name}");
    }

    // Emission order must match reader arm order for deterministic
    // HEADER layout + roundtrip stability.
    let order = [
        "$LOFTANG1",
        "$LOFTANG2",
        "$LOFTMAG1",
        "$LOFTMAG2",
        "$LOFTNORMALS",
        "$LOFTPARAM",
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
fn header_roundtrip_preserves_loft_family() {
    let doc1 = read_dxf(&dxf_with_loft_family()).expect("first read");

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    // The draft-angle assertions are the real canary: π/6 and π/3 both
    // have > 15 significant decimal digits; only the round-25 shortest
    // round-trip `format_f64` upgrade lets them survive read → write →
    // read bit-identical. A regression here would mean someone lowered
    // `format_f64` precision without updating this guard.
    assert_eq!(doc2.header.loft_ang1, LOFT_ANG1);
    assert_eq!(doc2.header.loft_ang2, LOFT_ANG2);
    assert_eq!(doc2.header.loft_mag1, LOFT_MAG1);
    assert_eq!(doc2.header.loft_mag2, LOFT_MAG2);
    assert_eq!(doc2.header.loft_normals, LOFT_NORMALS);
    assert_eq!(doc2.header.loft_param, LOFT_PARAM);
}

#[test]
fn header_legacy_file_without_loft_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // AutoCAD factory defaults (documented in DocumentHeader::default).
    assert_eq!(doc.header.loft_ang1, 0.0);
    assert_eq!(doc.header.loft_ang2, 0.0);
    assert_eq!(doc.header.loft_mag1, 0.0);
    assert_eq!(doc.header.loft_mag2, 0.0);
    assert_eq!(doc.header.loft_normals, 1);
    assert_eq!(doc.header.loft_param, 7);
}
