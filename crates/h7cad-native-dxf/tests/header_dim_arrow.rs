use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_dim_arrow() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMBLK\n  1\n_DOT\n",
        "  9\n$DIMBLK1\n  1\n_OPEN\n",
        "  9\n$DIMBLK2\n  1\n_CLOSED\n",
        "  9\n$DIMLDRBLK\n  1\n_DOTBLANK\n",
        "  9\n$DIMARCSYM\n 70\n     1\n",
        "  9\n$DIMJOGANG\n 40\n1.5708\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_dim_arrow_family() {
    let dxf = make_dxf_with_dim_arrow();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.dim_blk, "_DOT");
    assert_eq!(doc.header.dim_blk1, "_OPEN");
    assert_eq!(doc.header.dim_blk2, "_CLOSED");
    assert_eq!(doc.header.dim_ldrblk, "_DOTBLANK");
    assert_eq!(doc.header.dim_arcsym, 1);
    assert!((doc.header.dim_jogang - 1.5708).abs() < 1e-4);
}

#[test]
fn header_writes_dim_arrow_family() {
    let dxf = make_dxf_with_dim_arrow();
    let doc = read_dxf(&dxf).unwrap();
    let output = write_dxf(&doc).unwrap();
    assert!(output.contains("$DIMBLK"));
    assert!(output.contains("_DOT"));
    assert!(output.contains("$DIMBLK1"));
    assert!(output.contains("_OPEN"));
    assert!(output.contains("$DIMBLK2"));
    assert!(output.contains("_CLOSED"));
    assert!(output.contains("$DIMLDRBLK"));
    assert!(output.contains("_DOTBLANK"));
    assert!(output.contains("$DIMARCSYM"));
    assert!(output.contains("$DIMJOGANG"));
}

#[test]
fn header_roundtrip_preserves_dim_arrow_family() {
    let dxf = make_dxf_with_dim_arrow();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.dim_blk, doc2.header.dim_blk);
    assert_eq!(doc1.header.dim_blk1, doc2.header.dim_blk1);
    assert_eq!(doc1.header.dim_blk2, doc2.header.dim_blk2);
    assert_eq!(doc1.header.dim_ldrblk, doc2.header.dim_ldrblk);
    assert_eq!(doc1.header.dim_arcsym, doc2.header.dim_arcsym);
    assert!((doc1.header.dim_jogang - doc2.header.dim_jogang).abs() < 1e-15);
}

#[test]
fn header_legacy_file_without_dim_arrow_loads_with_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.dim_blk, "");
    assert_eq!(doc.header.dim_blk1, "");
    assert_eq!(doc.header.dim_blk2, "");
    assert_eq!(doc.header.dim_ldrblk, "");
    assert_eq!(doc.header.dim_arcsym, 0);
    assert!((doc.header.dim_jogang - std::f64::consts::FRAC_PI_4).abs() < 1e-15);
}
