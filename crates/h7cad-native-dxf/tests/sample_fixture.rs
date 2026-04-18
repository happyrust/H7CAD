//! Regression test covering the shared `tests/fixtures/sample.dxf` fixture.
//!
//! This fixture doubles as the file the top-level `H7CAD` binary opens via the
//! `H7CAD.exe <path>` CLI smoke test. Keeping a parser-level assertion here
//! makes sure any refactor that silently regresses the reader shows up in CI.

use h7cad_native_dxf::read_dxf_bytes;
use h7cad_native_model::{DxfVersion, EntityData};

const SAMPLE_BYTES: &[u8] = include_bytes!("../../../tests/fixtures/sample.dxf");

#[test]
fn sample_dxf_parses_with_expected_shape() {
    let doc = read_dxf_bytes(SAMPLE_BYTES).expect("sample.dxf must parse");

    assert_eq!(doc.header.version, DxfVersion::R2000);
    assert_eq!(doc.tables.layer.entries.len(), 1);
    assert_eq!(doc.tables.block_record.entries.len(), 2);
    assert_eq!(doc.entities.len(), 7);

    let mut lines = 0;
    let mut circles = 0;
    let mut arcs = 0;
    let mut texts = 0;
    for e in &doc.entities {
        match &e.data {
            EntityData::Line { .. } => lines += 1,
            EntityData::Circle { .. } => circles += 1,
            EntityData::Arc { .. } => arcs += 1,
            EntityData::Text { .. } => texts += 1,
            other => panic!("unexpected entity variant in fixture: {other:?}"),
        }
    }
    assert_eq!(lines, 4, "sample.dxf should have 4 LINE entities");
    assert_eq!(circles, 1, "sample.dxf should have 1 CIRCLE entity");
    assert_eq!(arcs, 1, "sample.dxf should have 1 ARC entity");
    assert_eq!(texts, 1, "sample.dxf should have 1 TEXT entity");
}
