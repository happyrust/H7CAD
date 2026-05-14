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
fn spatial_filter_object_preserves_raw_codes_through_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nSPATIAL_FILTER\n",
        "  5\nA4\n",
        "330\n5\n",
        "100\nAcDbFilter\n",
        "100\nAcDbSpatialFilter\n",
        " 70\n1\n",
        " 10\n0.0\n",
        " 20\n0.0\n",
        " 11\n10.0\n",
        " 21\n10.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("SPATIAL_FILTER object should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::SpatialFilter { .. }))
        .expect("SPATIAL_FILTER should be classified");

    match &object.data {
        ObjectData::SpatialFilter {
            object_type,
            raw_codes,
        } => {
            assert_eq!(object_type, "SPATIAL_FILTER");
            assert_eq!(
                raw_codes,
                &vec![
                    (100, "AcDbFilter".to_string()),
                    (100, "AcDbSpatialFilter".to_string()),
                    (70, "1".to_string()),
                    (10, "0.0".to_string()),
                    (20, "0.0".to_string()),
                    (11, "10.0".to_string()),
                    (21, "10.0".to_string()),
                ]
            );
        }
        other => panic!("expected SpatialFilter, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("SPATIAL_FILTER should write");
    assert!(has_pair(&output, 0, "SPATIAL_FILTER"));
    assert!(has_pair(&output, 100, "AcDbSpatialFilter"));
    assert!(has_pair(&output, 70, "1"));
    assert!(has_pair(&output, 11, "10.0"));

    let roundtripped = read_dxf(&output).expect("written SPATIAL_FILTER should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::SpatialFilter {
            object_type,
            raw_codes,
        } if object_type == "SPATIAL_FILTER"
            && raw_codes.iter().any(|(code, value)| *code == 100 && value == "AcDbSpatialFilter")
    )));
}
