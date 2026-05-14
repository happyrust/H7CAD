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
fn raster_variables_object_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nRASTERVARIABLES\n",
        "  5\nA2\n",
        "330\n5\n",
        "100\nAcDbRasterVariables\n",
        " 90\n1\n",
        " 70\n2\n",
        " 71\n1\n",
        " 72\n3\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("RASTERVARIABLES object should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::RasterVariables { .. }))
        .expect("RASTERVARIABLES should be modelled");

    match &object.data {
        ObjectData::RasterVariables {
            class_version,
            display_image_frame,
            image_quality,
            units,
        } => {
            assert_eq!(*class_version, 1);
            assert_eq!(*display_image_frame, 2);
            assert_eq!(*image_quality, 1);
            assert_eq!(*units, 3);
        }
        other => panic!("expected RasterVariables, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("RASTERVARIABLES should write");
    assert!(has_pair(&output, 0, "RASTERVARIABLES"));
    assert!(has_pair(&output, 100, "AcDbRasterVariables"));
    assert!(has_pair(&output, 90, "1"));
    assert!(has_pair(&output, 70, "2"));
    assert!(has_pair(&output, 71, "1"));
    assert!(has_pair(&output, 72, "3"));

    let roundtripped = read_dxf(&output).expect("written RASTERVARIABLES should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::RasterVariables {
            class_version: 1,
            display_image_frame: 2,
            image_quality: 1,
            units: 3,
        }
    )));
}
