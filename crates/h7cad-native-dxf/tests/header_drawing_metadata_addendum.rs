//! Integration tests for the DXF HEADER drawing-metadata addendum
//! family (`$PROJECTNAME / $HYPERLINKBASE / $INDEXCTL / $OLESTARTUP`).
//! 2026-04-22 drawing-metadata-addendum plan — 4 variables, first
//! round with mixed types (1 × String × 2, 1 × i16, 1 × bool) that
//! exercises `sv(1) / i16v(70) / bv(290)` in a single test file.
//!
//! Ground-truth values are chosen so each field can fail independently
//! AND so the two String fields would trigger arm-wiring regressions
//! immediately if crossed (both share DXF code 1):
//!
//!   project_name   = "my-proj/sub-dir 项目 α"         (≠ default "")
//!   hyperlink_base = "https://example.com/docs/日本語/" (≠ default "")
//!   indexctl       = 3    (two bits set; ≠ default 0)
//!   olestartup     = true (≠ default false)
//!
//! Both String values embed Unicode (CJK + Greek) as a mild smoke test
//! that the HEADER string path does not silently squash non-ASCII.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::CadDocument;

const PROJECT_NAME: &str = "my-proj/sub-dir 项目 α";
const HYPERLINK_BASE: &str = "https://example.com/docs/日本語/";
const INDEXCTL: i16 = 3;
const OLESTARTUP: bool = true;

fn dxf_with_drawing_metadata_addendum() -> String {
    format!(
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  9\n$PROJECTNAME\n  1\n{pn}\n",
            "  9\n$HYPERLINKBASE\n  1\n{hb}\n",
            "  9\n$INDEXCTL\n 70\n{ic}\n",
            "  9\n$OLESTARTUP\n290\n{os}\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        ),
        pn = PROJECT_NAME,
        hb = HYPERLINK_BASE,
        ic = INDEXCTL,
        os = if OLESTARTUP { 1 } else { 0 },
    )
}

#[test]
fn header_reads_drawing_metadata_addendum_family() {
    let doc = read_dxf(&dxf_with_drawing_metadata_addendum()).expect("parse");

    assert_eq!(doc.header.project_name, PROJECT_NAME, "$PROJECTNAME");
    assert_eq!(doc.header.hyperlink_base, HYPERLINK_BASE, "$HYPERLINKBASE");
    assert_ne!(
        doc.header.project_name, doc.header.hyperlink_base,
        "project_name must not cross-read hyperlink_base — both are DXF code 1 \
         so arm-wiring regressions would show up here first"
    );
    assert_eq!(doc.header.indexctl, INDEXCTL, "$INDEXCTL");
    assert_eq!(doc.header.olestartup, OLESTARTUP, "$OLESTARTUP");
}

#[test]
fn header_writes_drawing_metadata_addendum_family() {
    let mut doc = CadDocument::new();
    doc.header.project_name = PROJECT_NAME.to_string();
    doc.header.hyperlink_base = HYPERLINK_BASE.to_string();
    doc.header.indexctl = INDEXCTL;
    doc.header.olestartup = OLESTARTUP;

    let text = write_dxf(&doc).expect("write");

    for name in ["$PROJECTNAME", "$HYPERLINKBASE", "$INDEXCTL", "$OLESTARTUP"] {
        assert!(text.contains(name), "writer must emit {name}");
    }
    assert!(
        text.contains(PROJECT_NAME),
        "writer must preserve project_name verbatim (Unicode included)"
    );
    assert!(
        text.contains(HYPERLINK_BASE),
        "writer must preserve hyperlink_base verbatim (Unicode included)"
    );

    // Emission order must match reader arm order — deterministic HEADER
    // layout keeps roundtrip stable.
    let order = ["$PROJECTNAME", "$HYPERLINKBASE", "$INDEXCTL", "$OLESTARTUP"];
    let mut cursor = 0usize;
    for name in order {
        let hit = text[cursor..]
            .find(name)
            .unwrap_or_else(|| panic!("{name} not found in expected order"));
        cursor += hit + name.len();
    }
}

#[test]
fn header_roundtrip_preserves_drawing_metadata_addendum_family() {
    let doc1 = read_dxf(&dxf_with_drawing_metadata_addendum()).expect("first read");

    let text = write_dxf(&doc1).expect("write");
    let doc2 = read_dxf(&text).expect("second read");

    assert_eq!(doc2.header.project_name, PROJECT_NAME);
    assert_eq!(doc2.header.hyperlink_base, HYPERLINK_BASE);
    assert_eq!(doc2.header.indexctl, INDEXCTL);
    assert_eq!(doc2.header.olestartup, OLESTARTUP);
}

#[test]
fn header_legacy_file_without_drawing_metadata_addendum_loads_with_defaults() {
    let legacy = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("legacy parse");

    // `DocumentHeader::default()` — AutoCAD factory baseline. Any change
    // to these defaults must be synced with the plan doc's "默认值选型"
    // section AND this test.
    assert_eq!(doc.header.project_name, "");
    assert_eq!(doc.header.hyperlink_base, "");
    assert_eq!(doc.header.indexctl, 0);
    assert!(!doc.header.olestartup);
}
