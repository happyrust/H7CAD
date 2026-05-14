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
fn block_linear_parameter_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nBLOCKLINEARPARAMETER\n",
        "  5\nA81\n",
        "330\nA80\n",
        "100\nAcDbEvalExpr\n",
        " 90\n1\n",
        " 98\n33\n",
        " 99\n329\n",
        "100\nAcDbBlockElement\n",
        "300\nLinear\n",
        " 98\n25\n",
        " 99\n104\n",
        "1071\n0\n",
        "100\nAcDbBlockParameter\n",
        "280\n1\n",
        "281\n0\n",
        "100\nAcDbBlock2PtParameter\n",
        "1010\n-1.0\n",
        "1020\n0.0000000000000001\n",
        "1030\n0.0\n",
        "1011\n1.0\n",
        "1021\n0.0000000000000002\n",
        "1031\n0.0\n",
        "170\n4\n",
        " 91\n5\n",
        " 91\n2\n",
        " 91\n0\n",
        " 91\n0\n",
        "171\n1\n",
        " 92\n5\n",
        "301\nDisplacementX\n",
        "172\n1\n",
        " 93\n5\n",
        "302\nDisplacementY\n",
        "173\n1\n",
        " 94\n2\n",
        "303\nDisplacementX\n",
        "174\n1\n",
        " 95\n2\n",
        "304\nDisplacementY\n",
        "177\n0\n",
        "100\nAcDbBlockLinearParameter\n",
        "305\ndynamic-diameter\n",
        "306\ndinamic parameter for the circle diameter\n",
        "140\n-1.330337949062004\n",
        "307\n\n",
        " 96\n1\n",
        "141\n0.0\n",
        "142\n0.0\n",
        "143\n0.0\n",
        "175\n0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("block linear parameter should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::BlockLinearParameter { .. }))
        .expect("object should be semantic block linear parameter");

    match &object.data {
        ObjectData::BlockLinearParameter {
            eval_id,
            element_name,
            parameter_value_280,
            first_point,
            second_point,
            value_170,
            value_91_entries,
            value_301,
            value_302,
            value_303,
            value_304,
            label,
            description,
            label_offset,
            raw_codes,
            ..
        } => {
            assert_eq!(*eval_id, 1);
            assert_eq!(element_name, "Linear");
            assert_eq!(*parameter_value_280, 1);
            assert_eq!(*first_point, [-1.0, 0.0000000000000001, 0.0]);
            assert_eq!(*second_point, [1.0, 0.0000000000000002, 0.0]);
            assert_eq!(*value_170, 4);
            assert_eq!(value_91_entries, &vec![5, 2, 0, 0]);
            assert_eq!(value_301, "DisplacementX");
            assert_eq!(value_302, "DisplacementY");
            assert_eq!(value_303, "DisplacementX");
            assert_eq!(value_304, "DisplacementY");
            assert_eq!(label, "dynamic-diameter");
            assert_eq!(description, "dinamic parameter for the circle diameter");
            assert_eq!(*label_offset, -1.330337949062004);
            assert!(raw_codes.contains(&(307, String::new())));
            assert!(raw_codes.contains(&(96, "1".to_string())));
            assert!(raw_codes.contains(&(175, "0".to_string())));
        }
        other => panic!("expected BlockLinearParameter, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("block linear parameter should write");
    assert!(has_pair(&output, 0, "BLOCKLINEARPARAMETER"));
    assert!(has_pair(&output, 100, "AcDbBlockLinearParameter"));
    assert!(has_pair(&output, 300, "Linear"));
    assert!(has_pair(&output, 1010, "-1.0"));
    assert!(has_pair(&output, 1011, "1.0"));
    assert!(has_pair(&output, 305, "dynamic-diameter"));
    assert!(has_pair(
        &output,
        306,
        "dinamic parameter for the circle diameter"
    ));
    assert!(has_pair(&output, 140, "-1.330337949062004"));
    assert!(has_pair(&output, 96, "1"));

    let roundtripped = read_dxf(&output).expect("written object should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::BlockLinearParameter {
            eval_id: 1,
            element_name,
            label,
            ..
        } if element_name == "Linear" && label == "dynamic-diameter"
    )));
}
