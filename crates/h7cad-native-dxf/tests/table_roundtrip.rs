use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{CadDocument, Entity, EntityData};

#[test]
fn table_row_heights_column_widths_survive_roundtrip() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Table {
        num_rows: 3,
        num_cols: 2,
        insertion: [10.0, 20.0, 0.0],
        horizontal_direction: [1.0, 0.0, 0.0],
        version: 1,
        value_flag: 0,
        row_heights: vec![15.0, 25.0, 35.0],
        column_widths: vec![50.0, 80.0],
    }));

    let dxf_text = write_dxf(&doc).unwrap();

    assert!(dxf_text.contains("141"), "must emit code 141 for row heights");
    assert!(dxf_text.contains("142"), "must emit code 142 for column widths");

    let doc2 = read_dxf(&dxf_text).unwrap();
    assert_eq!(doc2.entities.len(), 1);

    if let EntityData::Table {
        num_rows,
        num_cols,
        insertion,
        row_heights,
        column_widths,
        ..
    } = &doc2.entities[0].data
    {
        assert_eq!(*num_rows, 3);
        assert_eq!(*num_cols, 2);
        assert_eq!(*insertion, [10.0, 20.0, 0.0]);
        assert_eq!(row_heights, &[15.0, 25.0, 35.0]);
        assert_eq!(column_widths, &[50.0, 80.0]);
    } else {
        panic!("expected Table entity data");
    }
}

#[test]
fn table_without_heights_widths_loads_with_empty_vecs() {
    let dxf = concat!(
        "  0\nSECTION\n  2\nHEADER\n",
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  0\nENDSEC\n",
        "  0\nSECTION\n  2\nENTITIES\n",
        "  0\nACAD_TABLE\n",
        "  8\n0\n",
        " 90\n0\n",
        "280\n     1\n",
        " 10\n5.0\n 20\n10.0\n 30\n0.0\n",
        " 11\n1.0\n 21\n0.0\n 31\n0.0\n",
        " 91\n2\n 92\n3\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(dxf).unwrap();
    let table = doc.entities.iter().find(|e| matches!(&e.data, EntityData::Table { .. }));
    assert!(table.is_some());
    if let EntityData::Table { row_heights, column_widths, .. } = &table.unwrap().data {
        assert!(row_heights.is_empty());
        assert!(column_widths.is_empty());
    }
}
