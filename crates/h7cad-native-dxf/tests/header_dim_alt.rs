//! Integration tests for the DXF HEADER DIM alternate-units family
//! (`$DIMALT / $DIMALTD / $DIMALTF / $DIMALTRND / $DIMALTTD /
//! $DIMALTTZ / $DIMALTU / $DIMALTZ / $DIMAPOST`). 2026-04-22
//! dimalt-family plan — 9 variables, the largest single-round
//! expansion so far and an arm-wiring stress test (6 fields share
//! DXF code 70, 2 share code 40).
//!
//! Ground-truth values for the six code-70 fields are chosen so they
//! are **all pairwise distinct**: `1, 3, 4, 12, 6, 5`. Any swap of
//! two arms would therefore collapse at least two of the six into a
//! shared value — an `assert_ne!` matrix in the read test catches
//! that instantly.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const DIM_ALT: i16 = 1;
const DIM_ALTD: i16 = 3;
const DIM_ALTF: f64 = 2.54;
const DIM_ALTRND: f64 = 0.5;
const DIM_ALTTD: i16 = 4;
const DIM_ALTTZ: i16 = 12; // bit 4 + bit 8 (suppress 0-feet + 0-inches)
const DIM_ALTU: i16 = 6;   // architectural units
const DIM_ALTZ: i16 = 5;   // bit 1 + bit 4 (suppress leading zero + 0-feet)
const DIM_APOST: &str = "<> mm";

fn dxf_with_dim_alt_family() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$DIMALT\n 70\n{a}\n",
            "  9\n$DIMALTD\n 70\n{ad}\n",
            "  9\n$DIMALTF\n 40\n{af}\n",
            "  9\n$DIMALTRND\n 40\n{ar}\n",
            "  9\n$DIMALTTD\n 70\n{atd}\n",
            "  9\n$DIMALTTZ\n 70\n{atz}\n",
            "  9\n$DIMALTU\n 70\n{au}\n",
            "  9\n$DIMALTZ\n 70\n{az}\n",
            "  9\n$DIMAPOST\n  1\n{ap}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        a = DIM_ALT,
        ad = DIM_ALTD,
        af = DIM_ALTF,
        ar = DIM_ALTRND,
        atd = DIM_ALTTD,
        atz = DIM_ALTTZ,
        au = DIM_ALTU,
        az = DIM_ALTZ,
        ap = DIM_APOST,
    )
}

#[test]
fn header_reads_dim_alt_family() {
    let doc = read_dxf(&dxf_with_dim_alt_family()).expect("parse");

    // Direct value checks.
    assert_eq!(doc.header.dim_alt, DIM_ALT, "$DIMALT");
    assert_eq!(doc.header.dim_altd, DIM_ALTD, "$DIMALTD");
    assert_eq!(doc.header.dim_altf, DIM_ALTF, "$DIMALTF");
    assert_eq!(doc.header.dim_altrnd, DIM_ALTRND, "$DIMALTRND");
    assert_eq!(doc.header.dim_alttd, DIM_ALTTD, "$DIMALTTD");
    assert_eq!(doc.header.dim_alttz, DIM_ALTTZ, "$DIMALTTZ");
    assert_eq!(doc.header.dim_altu, DIM_ALTU, "$DIMALTU");
    assert_eq!(doc.header.dim_altz, DIM_ALTZ, "$DIMALTZ");
    assert_eq!(doc.header.dim_apost, DIM_APOST, "$DIMAPOST");

    // Arm-wiring regression guard: the six code-70 fields must stay
    // pairwise distinct. A swap of any two arms would collapse at
    // least one pair into equality and trip the corresponding
    // `assert_ne!` below. Comprehensive 6-choose-2 = 15 pairs.
    let vals = [
        ("dim_alt", doc.header.dim_alt),
        ("dim_altd", doc.header.dim_altd),
        ("dim_alttd", doc.header.dim_alttd),
        ("dim_alttz", doc.header.dim_alttz),
        ("dim_altu", doc.header.dim_altu),
        ("dim_altz", doc.header.dim_altz),
    ];
    for i in 0..vals.len() {
        for j in (i + 1)..vals.len() {
            assert_ne!(
                vals[i].1, vals[j].1,
                "{} and {} must not collide (both code 70) — ground-truth \
                 was picked to keep them distinct",
                vals[i].0, vals[j].0
            );
        }
    }
}

#[test]
fn header_writes_dim_alt_family() {
    let mut doc = CadDocument::new();
    doc.header.dim_alt = DIM_ALT;
    doc.header.dim_altd = DIM_ALTD;
    doc.header.dim_altf = DIM_ALTF;
    doc.header.dim_altrnd = DIM_ALTRND;
    doc.header.dim_alttd = DIM_ALTTD;
    doc.header.dim_alttz = DIM_ALTTZ;
    doc.header.dim_altu = DIM_ALTU;
    doc.header.dim_altz = DIM_ALTZ;
    doc.header.dim_apost = DIM_APOST.to_string();

    let text = write_dxf(&doc).expect("write");

    let order = [
        "$DIMALT",
        "$DIMALTD",
        "$DIMALTF",
        "$DIMALTRND",
        "$DIMALTTD",
        "$DIMALTTZ",
        "$DIMALTU",
        "$DIMALTZ",
        "$DIMAPOST",
    ];
    for name in order {
        assert!(text.contains(name), "writer must emit {name}");
    }

    // `$DIMAPOST` carries the classic "<>" placeholder — writers that
    // reach for fancy escaping would mangle it here.
    assert!(
        text.contains("<> mm"),
        "writer must preserve the `<>` placeholder verbatim"
    );

    // Emission order must match reader arm order.
    let mut cursor = 0usize;
    for name in order {
        let hit = text[cursor..]
            .find(name)
            .unwrap_or_else(|| panic!("{name} not found in expected order"));
        cursor += hit + name.len();
    }
}

#[test]
fn header_roundtrip_preserves_dim_alt_family() {
    let doc1 = read_dxf(&dxf_with_dim_alt_family()).expect("first read");

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc2.header.dim_alt, DIM_ALT);
    assert_eq!(doc2.header.dim_altd, DIM_ALTD);
    assert_eq!(
        doc2.header.dim_altf, DIM_ALTF,
        "`2.54` must survive `format_f64` shortest round-trip — round \
         25's precision upgrade is the safety net here"
    );
    assert_eq!(doc2.header.dim_altrnd, DIM_ALTRND);
    assert_eq!(doc2.header.dim_alttd, DIM_ALTTD);
    assert_eq!(doc2.header.dim_alttz, DIM_ALTTZ);
    assert_eq!(doc2.header.dim_altu, DIM_ALTU);
    assert_eq!(doc2.header.dim_altz, DIM_ALTZ);
    assert_eq!(doc2.header.dim_apost, DIM_APOST);
}

#[test]
fn header_legacy_file_without_dim_alt_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // AutoCAD factory defaults (documented in `DocumentHeader::default`
    // and mirrored in the plan's §"默认值选型").
    assert_eq!(doc.header.dim_alt, 0);
    assert_eq!(doc.header.dim_altd, 2);
    assert_eq!(doc.header.dim_altf, 25.4);
    assert_eq!(doc.header.dim_altrnd, 0.0);
    assert_eq!(doc.header.dim_alttd, 2);
    assert_eq!(doc.header.dim_alttz, 0);
    assert_eq!(doc.header.dim_altu, 2);
    assert_eq!(doc.header.dim_altz, 0);
    assert_eq!(doc.header.dim_apost, "");
}
