use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_dim_render() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMCLRD\n 70\n     1\n",
        "  9\n$DIMCLRE\n 70\n     2\n",
        "  9\n$DIMCLRT\n 70\n     3\n",
        "  9\n$DIMLWD\n 70\n    25\n",
        "  9\n$DIMLWE\n 70\n    18\n",
        "  9\n$DIMTAD\n 70\n     1\n",
        "  9\n$DIMTIH\n 70\n     0\n",
        "  9\n$DIMTOH\n 70\n     0\n",
        "  9\n$DIMDLE\n 40\n1.5\n",
        "  9\n$DIMCEN\n 40\n3.0\n",
        "  9\n$DIMTSZ\n 40\n0.5\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_dim_render_family() {
    let dxf = make_dxf_with_dim_render();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.dim_clrd, 1);
    assert_eq!(doc.header.dim_clre, 2);
    assert_eq!(doc.header.dim_clrt, 3);
    assert_eq!(doc.header.dim_lwd, 25);
    assert_eq!(doc.header.dim_lwe, 18);
    assert_eq!(doc.header.dim_tad, 1);
    assert_eq!(doc.header.dim_tih, 0);
    assert_eq!(doc.header.dim_toh, 0);
    assert!((doc.header.dim_dle - 1.5).abs() < 1e-10);
    assert!((doc.header.dim_cen - 3.0).abs() < 1e-10);
    assert!((doc.header.dim_tsz - 0.5).abs() < 1e-10);
}

#[test]
fn header_roundtrip_preserves_dim_render() {
    let dxf = make_dxf_with_dim_render();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.dim_clrd, doc2.header.dim_clrd);
    assert_eq!(doc1.header.dim_clre, doc2.header.dim_clre);
    assert_eq!(doc1.header.dim_clrt, doc2.header.dim_clrt);
    assert_eq!(doc1.header.dim_lwd, doc2.header.dim_lwd);
    assert_eq!(doc1.header.dim_lwe, doc2.header.dim_lwe);
    assert_eq!(doc1.header.dim_tad, doc2.header.dim_tad);
    assert_eq!(doc1.header.dim_tih, doc2.header.dim_tih);
    assert_eq!(doc1.header.dim_toh, doc2.header.dim_toh);
    assert!((doc1.header.dim_dle - doc2.header.dim_dle).abs() < 1e-15);
    assert!((doc1.header.dim_cen - doc2.header.dim_cen).abs() < 1e-15);
    assert!((doc1.header.dim_tsz - doc2.header.dim_tsz).abs() < 1e-15);
}

#[test]
fn header_legacy_file_uses_dim_render_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.dim_clrd, 0);
    assert_eq!(doc.header.dim_clre, 0);
    assert_eq!(doc.header.dim_clrt, 0);
    assert_eq!(doc.header.dim_lwd, -2);
    assert_eq!(doc.header.dim_lwe, -2);
    assert_eq!(doc.header.dim_tad, 0);
    assert_eq!(doc.header.dim_tih, 1);
    assert_eq!(doc.header.dim_toh, 1);
    assert_eq!(doc.header.dim_dle, 0.0);
    assert!((doc.header.dim_cen - 2.5).abs() < 1e-10);
    assert_eq!(doc.header.dim_tsz, 0.0);
}
