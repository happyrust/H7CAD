use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::ObjectData;

fn has_pair(text: &str, code: i16, value: &str) -> bool {
    let mut lines = text.lines();
    while let Some(code_line) = lines.next() {
        let Some(value_line) = lines.next() else {
            return false;
        };
        if code_line.trim() == code.to_string() && value_line.trim() == value {
            return true;
        }
    }
    false
}

#[test]
fn tablecontent_and_tablegeometry_preserve_raw_codes_through_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nTABLECONTENT\n",
        "  5\nB1\n",
        "330\n5\n",
        "100\nAcDbTableContent\n",
        " 90\n2\n",
        "300\ncell-payload\n",
        "  0\nTABLEGEOMETRY\n",
        "  5\nB2\n",
        "330\n5\n",
        "100\nAcDbTableGeometry\n",
        " 90\n3\n",
        " 10\n1.0\n",
        " 20\n2.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("table objects should parse");
    let content = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::TableContent { .. }))
        .expect("TABLECONTENT should be classified");
    let geometry = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::TableGeometry { .. }))
        .expect("TABLEGEOMETRY should be classified");

    match &content.data {
        ObjectData::TableContent { raw_codes } => {
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbTableContent".to_string()),
                    (90, "2".to_string()),
                    (300, "cell-payload".to_string())
                ]
            );
        }
        other => panic!("expected TableContent, got {other:?}"),
    }
    match &geometry.data {
        ObjectData::TableGeometry { raw_codes } => {
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbTableGeometry".to_string()),
                    (90, "3".to_string()),
                    (10, "1.0".to_string()),
                    (20, "2.0".to_string())
                ]
            );
        }
        other => panic!("expected TableGeometry, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("table objects should write");
    assert!(has_pair(&output, 0, "TABLECONTENT"));
    assert!(has_pair(&output, 100, "AcDbTableContent"));
    assert!(has_pair(&output, 300, "cell-payload"));
    assert!(has_pair(&output, 0, "TABLEGEOMETRY"));
    assert!(has_pair(&output, 100, "AcDbTableGeometry"));
    assert!(has_pair(&output, 10, "1.0"));

    let roundtripped = read_dxf(&output).expect("written table objects should reparse");
    assert!(roundtripped
        .objects
        .iter()
        .any(|object| matches!(&object.data, ObjectData::TableContent { .. })));
    assert!(roundtripped
        .objects
        .iter()
        .any(|object| matches!(&object.data, ObjectData::TableGeometry { .. })));
}
