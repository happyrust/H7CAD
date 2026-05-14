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
fn block_grip_location_component_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nBLOCKGRIPLOCATIONCOMPONENT\n",
        "  5\nA83\n",
        "330\nA80\n",
        "100\nAcDbEvalExpr\n",
        " 90\n3\n",
        " 98\n33\n",
        " 99\n329\n",
        "  1\n\n",
        " 70\n40\n",
        "140\n1.797693134862314E+99\n",
        "100\nAcDbBlockGripExpr\n",
        " 91\n1\n",
        "300\nUpdatedEndX\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("block grip location component should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::BlockGripLocationComponent { .. }))
        .expect("object should be semantic block grip location component");

    match &object.data {
        ObjectData::BlockGripLocationComponent {
            eval_id,
            value_98,
            value_99,
            eval_value_codes,
            value_91,
            expression_name,
            raw_codes,
        } => {
            assert_eq!(*eval_id, 3);
            assert_eq!(*value_98, 33);
            assert_eq!(*value_99, 329);
            assert_eq!(*value_91, 1);
            assert_eq!(expression_name, "UpdatedEndX");
            assert_eq!(
                eval_value_codes,
                &vec![
                    (1, String::new()),
                    (70, "40".to_string()),
                    (140, "1.797693134862314E+99".to_string())
                ]
            );
            assert!(raw_codes.is_empty());
        }
        other => panic!("expected BlockGripLocationComponent, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("block grip location component should write");
    assert!(has_pair(&output, 0, "BLOCKGRIPLOCATIONCOMPONENT"));
    assert!(has_pair(&output, 100, "AcDbEvalExpr"));
    assert!(has_pair(&output, 90, "3"));
    assert!(has_pair(&output, 98, "33"));
    assert!(has_pair(&output, 99, "329"));
    assert!(has_pair(&output, 70, "40"));
    assert!(has_pair(&output, 140, "1.797693134862314E+99"));
    assert!(has_pair(&output, 100, "AcDbBlockGripExpr"));
    assert!(has_pair(&output, 91, "1"));
    assert!(has_pair(&output, 300, "UpdatedEndX"));

    let roundtripped = read_dxf(&output).expect("written object should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::BlockGripLocationComponent {
            eval_id: 3,
            value_91: 1,
            expression_name,
            ..
        } if expression_name == "UpdatedEndX"
    )));
}
