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
fn block_linear_grip_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nBLOCKLINEARGRIP\n",
        "  5\nA82\n",
        "330\nA80\n",
        "100\nAcDbEvalExpr\n",
        " 90\n2\n",
        " 98\n33\n",
        " 99\n329\n",
        "100\nAcDbBlockElement\n",
        "300\nEnd Grip\n",
        " 98\n25\n",
        " 99\n104\n",
        "1071\n0\n",
        "100\nAcDbBlockGrip\n",
        " 91\n3\n",
        " 92\n4\n",
        "1010\n1.0\n",
        "1020\n0.0000000000000002\n",
        "1030\n0.0\n",
        "280\n1\n",
        " 93\n-1\n",
        "100\nAcDbBlockLinearGrip\n",
        "140\n2.0\n",
        "141\n0.0000000000000001\n",
        "142\n0.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("block linear grip should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::BlockLinearGrip { .. }))
        .expect("object should be semantic block linear grip");

    match &object.data {
        ObjectData::BlockLinearGrip {
            eval_id,
            eval_value_98,
            eval_value_99,
            element_name,
            element_value_98,
            element_value_99,
            element_value_1071,
            grip_value_91,
            grip_value_92,
            location,
            grip_flag_280,
            grip_value_93,
            linear_vector,
            raw_codes,
        } => {
            assert_eq!(*eval_id, 2);
            assert_eq!(*eval_value_98, 33);
            assert_eq!(*eval_value_99, 329);
            assert_eq!(element_name, "End Grip");
            assert_eq!(*element_value_98, 25);
            assert_eq!(*element_value_99, 104);
            assert_eq!(*element_value_1071, 0);
            assert_eq!(*grip_value_91, 3);
            assert_eq!(*grip_value_92, 4);
            assert_eq!(*location, [1.0, 0.0000000000000002, 0.0]);
            assert_eq!(*grip_flag_280, 1);
            assert_eq!(*grip_value_93, -1);
            assert_eq!(*linear_vector, [2.0, 0.0000000000000001, 0.0]);
            assert!(raw_codes.is_empty());
        }
        other => panic!("expected BlockLinearGrip, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("block linear grip should write");
    assert!(has_pair(&output, 0, "BLOCKLINEARGRIP"));
    assert!(has_pair(&output, 100, "AcDbEvalExpr"));
    assert!(has_pair(&output, 90, "2"));
    assert!(has_pair(&output, 300, "End Grip"));
    assert!(has_pair(&output, 1010, "1.0"));
    assert!(has_pair(&output, 1020, "0.0000000000000002"));
    assert!(has_pair(&output, 140, "2.0"));
    assert!(has_pair(&output, 141, "0.0000000000000001"));

    let roundtripped = read_dxf(&output).expect("written object should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::BlockLinearGrip {
            eval_id: 2,
            element_name,
            grip_value_91: 3,
            ..
        } if element_name == "End Grip"
    )));
}
