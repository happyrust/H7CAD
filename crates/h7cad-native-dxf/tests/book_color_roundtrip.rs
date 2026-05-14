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
fn dbcolor_object_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nDBCOLOR\n",
        "  5\nA3\n",
        "330\n5\n",
        "100\nAcDbColor\n",
        "  1\nPANTONE 300 C\n",
        "  2\nPANTONE+ Solid Coated\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("DBCOLOR object should parse");
    let object = doc
        .objects
        .iter()
        .find(|object| matches!(&object.data, ObjectData::BookColor { .. }))
        .expect("DBCOLOR should be modelled");

    match &object.data {
        ObjectData::BookColor {
            color_name,
            book_name,
        } => {
            assert_eq!(color_name, "PANTONE 300 C");
            assert_eq!(book_name, "PANTONE+ Solid Coated");
        }
        other => panic!("expected BookColor, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("DBCOLOR should write");
    assert!(has_pair(&output, 0, "DBCOLOR"));
    assert!(has_pair(&output, 100, "AcDbColor"));
    assert!(has_pair(&output, 1, "PANTONE 300 C"));
    assert!(has_pair(&output, 2, "PANTONE+ Solid Coated"));

    let roundtripped = read_dxf(&output).expect("written DBCOLOR should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::BookColor {
            color_name,
            book_name,
        } if color_name == "PANTONE 300 C" && book_name == "PANTONE+ Solid Coated"
    )));
}
