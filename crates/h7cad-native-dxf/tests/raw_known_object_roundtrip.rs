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
fn raw_known_object_preserves_codes_through_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nBLOCKSCALEACTION\n",
        "  5\nC1\n",
        "330\n5\n",
        "100\nAcDbBlockScaleAction\n",
        " 90\n7\n",
        "300\ngrip-payload\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("raw-known object should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::RawKnown { .. }))
        .expect("known raw object should be classified");

    match &object.data {
        ObjectData::RawKnown {
            object_type,
            raw_codes,
        } => {
            assert_eq!(object_type, "BLOCKSCALEACTION");
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbBlockScaleAction".to_string()),
                    (90, "7".to_string()),
                    (300, "grip-payload".to_string())
                ]
            );
        }
        other => panic!("expected RawKnown, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("raw-known object should write");
    assert!(has_pair(&output, 0, "BLOCKSCALEACTION"));
    assert!(has_pair(&output, 100, "AcDbBlockScaleAction"));
    assert!(has_pair(&output, 90, "7"));
    assert!(has_pair(&output, 300, "grip-payload"));

    let roundtripped = read_dxf(&output).expect("written raw-known object should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::RawKnown {
            object_type,
            raw_codes,
        } if object_type == "BLOCKSCALEACTION"
            && raw_codes.iter().any(|(code, value)| *code == 300 && value == "grip-payload")
    )));
}
