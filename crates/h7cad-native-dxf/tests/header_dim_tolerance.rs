use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_dim_tol() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMTP\n 40\n0.05\n",
        "  9\n$DIMTM\n 40\n0.02\n",
        "  9\n$DIMTOL\n 70\n     1\n",
        "  9\n$DIMLIM\n 70\n     0\n",
        "  9\n$DIMTVP\n 40\n0.75\n",
        "  9\n$DIMTFAC\n 40\n0.8\n",
        "  9\n$DIMTOLJ\n 70\n     2\n",
        "  9\n$COORDS\n 70\n     2\n",
        "  9\n$SPLTKNOTS\n 70\n     1\n",
        "  9\n$BLIPMODE\n 70\n     1\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_dim_tolerance_and_misc() {
    let dxf = make_dxf_with_dim_tol();
    let doc = read_dxf(&dxf).unwrap();
    assert!((doc.header.dim_tp - 0.05).abs() < 1e-10);
    assert!((doc.header.dim_tm - 0.02).abs() < 1e-10);
    assert_eq!(doc.header.dim_tol, 1);
    assert_eq!(doc.header.dim_lim, 0);
    assert!((doc.header.dim_tvp - 0.75).abs() < 1e-10);
    assert!((doc.header.dim_tfac - 0.8).abs() < 1e-10);
    assert_eq!(doc.header.dim_tolj, 2);
    assert_eq!(doc.header.coords, 2);
    assert_eq!(doc.header.spltknots, 1);
    assert_eq!(doc.header.blipmode, 1);
}

#[test]
fn header_roundtrip_preserves_dim_tolerance() {
    let dxf = make_dxf_with_dim_tol();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert!((doc1.header.dim_tp - doc2.header.dim_tp).abs() < 1e-15);
    assert!((doc1.header.dim_tm - doc2.header.dim_tm).abs() < 1e-15);
    assert_eq!(doc1.header.dim_tol, doc2.header.dim_tol);
    assert_eq!(doc1.header.dim_lim, doc2.header.dim_lim);
    assert!((doc1.header.dim_tvp - doc2.header.dim_tvp).abs() < 1e-15);
    assert!((doc1.header.dim_tfac - doc2.header.dim_tfac).abs() < 1e-15);
    assert_eq!(doc1.header.dim_tolj, doc2.header.dim_tolj);
    assert_eq!(doc1.header.coords, doc2.header.coords);
    assert_eq!(doc1.header.spltknots, doc2.header.spltknots);
    assert_eq!(doc1.header.blipmode, doc2.header.blipmode);
}

#[test]
fn header_legacy_uses_dim_tolerance_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.dim_tp, 0.0);
    assert_eq!(doc.header.dim_tm, 0.0);
    assert_eq!(doc.header.dim_tol, 0);
    assert_eq!(doc.header.dim_lim, 0);
    assert_eq!(doc.header.dim_tvp, 0.0);
    assert!((doc.header.dim_tfac - 1.0).abs() < 1e-10);
    assert_eq!(doc.header.dim_tolj, 1);
    assert_eq!(doc.header.coords, 1);
    assert_eq!(doc.header.spltknots, 0);
    assert_eq!(doc.header.blipmode, 0);
}
