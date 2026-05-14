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
fn pdf_definition_object_survives_roundtrip() {
    let input = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nPDFDEFINITION\n",
        "  5\nA1\n",
        "330\n5\n",
        "100\nAcDbPdfDefinition\n",
        "  1\nC:/docs/spec.pdf\n",
        "  2\nPage 1\n",
        "  3\nSpecPdf\n",
        "280\n1\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );

    let doc = read_dxf(input).expect("PDFDEFINITION object should parse");
    let definition = doc
        .objects
        .iter()
        .find(|object| {
            matches!(
                &object.data,
                ObjectData::UnderlayDefinition {
                    definition_type, ..
                } if definition_type == "PDFDEFINITION"
            )
        })
        .expect("PDFDEFINITION should be modelled");

    match &definition.data {
        ObjectData::UnderlayDefinition {
            definition_type,
            file_path,
            page_name,
            name,
            raw_codes,
        } => {
            assert_eq!(definition_type, "PDFDEFINITION");
            assert_eq!(file_path, "C:/docs/spec.pdf");
            assert_eq!(page_name, "Page 1");
            assert_eq!(name, "SpecPdf");
            assert_eq!(raw_codes, &vec![(280, "1".to_string())]);
        }
        other => panic!("expected UnderlayDefinition, got {other:?}"),
    }

    let output = write_dxf(&doc).expect("PDFDEFINITION should write");
    assert!(has_pair(&output, 0, "PDFDEFINITION"));
    assert!(has_pair(&output, 100, "AcDbPdfDefinition"));
    assert!(has_pair(&output, 1, "C:/docs/spec.pdf"));
    assert!(has_pair(&output, 2, "Page 1"));
    assert!(has_pair(&output, 3, "SpecPdf"));
    assert!(has_pair(&output, 280, "1"));

    let roundtripped = read_dxf(&output).expect("written PDFDEFINITION should reparse");
    assert!(roundtripped.objects.iter().any(|object| matches!(
        &object.data,
        ObjectData::UnderlayDefinition {
            definition_type,
            file_path,
            page_name,
            name,
            raw_codes,
        } if definition_type == "PDFDEFINITION"
            && file_path == "C:/docs/spec.pdf"
            && page_name == "Page 1"
            && name == "SpecPdf"
            && raw_codes == &vec![(280, "1".to_string())]
    )));
}
