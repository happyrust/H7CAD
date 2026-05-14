use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{EntityData, ObjectData};

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
fn unknown_entity_preserves_raw_codes_through_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nENTITIES\n",
        "  0\nCUSTOM_WIDGET\n",
        "  5\nE0\n",
        "330\n1F\n",
        "  8\nLayerA\n",
        "100\nAcDbCustomWidget\n",
        " 70\n42\n",
        "  1\npayload\n",
        "1001\nAPPID\n",
        "1000\nxdata-value\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("unknown entity input should parse");
    let entity = &doc.entities[0];
    assert_eq!(entity.layer_name, "LayerA");
    assert_eq!(entity.xdata.len(), 1);
    match &entity.data {
        EntityData::Unknown {
            entity_type,
            raw_codes,
        } => {
            assert_eq!(entity_type, "CUSTOM_WIDGET");
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbCustomWidget".to_string()),
                    (70, "42".to_string()),
                    (1, "payload".to_string())
                ]
            );
        }
        other => panic!("expected Unknown entity, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("unknown entity should write");
    assert!(has_pair(&output, 0, "CUSTOM_WIDGET"));
    assert!(has_pair(&output, 100, "AcDbCustomWidget"));
    assert!(has_pair(&output, 70, "42"));
    assert!(has_pair(&output, 1, "payload"));
    assert!(has_pair(&output, 1001, "APPID"));
    assert!(has_pair(&output, 1000, "xdata-value"));

    let roundtripped = read_dxf(&output).expect("written unknown entity should reparse");
    match &roundtripped.entities[0].data {
        EntityData::Unknown { raw_codes, .. } => {
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbCustomWidget".to_string()),
                    (70, "42".to_string()),
                    (1, "payload".to_string())
                ]
            );
        }
        other => panic!("expected Unknown entity after roundtrip, got {other:?}"),
    }
}

#[test]
fn unknown_object_preserves_raw_codes_through_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nCUSTOM_OBJECT\n",
        "  5\nA0\n",
        "330\n5\n",
        "100\nAcDbCustomObject\n",
        "280\n1\n",
        "300\nobject-payload\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("unknown object input should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::Unknown { object_type, .. } if object_type == "CUSTOM_OBJECT"))
        .expect("custom object should be preserved");
    match &object.data {
        ObjectData::Unknown {
            object_type,
            raw_codes,
        } => {
            assert_eq!(object_type, "CUSTOM_OBJECT");
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbCustomObject".to_string()),
                    (280, "1".to_string()),
                    (300, "object-payload".to_string())
                ]
            );
        }
        other => panic!("expected Unknown object, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("unknown object should write");
    assert!(has_pair(&output, 0, "CUSTOM_OBJECT"));
    assert!(has_pair(&output, 100, "AcDbCustomObject"));
    assert!(has_pair(&output, 280, "1"));
    assert!(has_pair(&output, 300, "object-payload"));

    let roundtripped = read_dxf(&output).expect("written unknown object should reparse");
    let object = roundtripped
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::Unknown { object_type, .. } if object_type == "CUSTOM_OBJECT"))
        .expect("custom object should survive roundtrip");
    match &object.data {
        ObjectData::Unknown { raw_codes, .. } => {
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbCustomObject".to_string()),
                    (280, "1".to_string()),
                    (300, "object-payload".to_string())
                ]
            );
        }
        other => panic!("expected Unknown object after roundtrip, got {other:?}"),
    }
}
