use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_render_meta() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$PELEVATION\n 40\n5.0\n",
        "  9\n$FACETRES\n 40\n2.0\n",
        "  9\n$ISOLINES\n 70\n     8\n",
        "  9\n$TEXTQLTY\n 70\n   100\n",
        "  9\n$TSTACKALIGN\n 70\n     2\n",
        "  9\n$TSTACKSIZE\n 70\n    85\n",
        "  9\n$ACADMAINTVER\n 70\n     6\n",
        "  9\n$CDATE\n 40\n2460422.5\n",
        "  9\n$LASTSAVEDBY\n  1\nTestUser\n",
        "  9\n$MENU\n  1\nacad\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_render_and_meta() {
    let dxf = make_dxf_with_render_meta();
    let doc = read_dxf(&dxf).unwrap();
    assert!((doc.header.pelevation - 5.0).abs() < 1e-10);
    assert!((doc.header.facetres - 2.0).abs() < 1e-10);
    assert_eq!(doc.header.isolines, 8);
    assert_eq!(doc.header.textqlty, 100);
    assert_eq!(doc.header.tstackalign, 2);
    assert_eq!(doc.header.tstacksize, 85);
    assert_eq!(doc.header.acadmaintver, 6);
    assert!((doc.header.cdate - 2460422.5).abs() < 1e-6);
    assert_eq!(doc.header.lastsavedby, "TestUser");
    assert_eq!(doc.header.menu, "acad");
}

#[test]
fn header_roundtrip_preserves_render_meta() {
    let dxf = make_dxf_with_render_meta();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert!((doc1.header.pelevation - doc2.header.pelevation).abs() < 1e-15);
    assert!((doc1.header.facetres - doc2.header.facetres).abs() < 1e-15);
    assert_eq!(doc1.header.isolines, doc2.header.isolines);
    assert_eq!(doc1.header.textqlty, doc2.header.textqlty);
    assert_eq!(doc1.header.tstackalign, doc2.header.tstackalign);
    assert_eq!(doc1.header.tstacksize, doc2.header.tstacksize);
    assert_eq!(doc1.header.acadmaintver, doc2.header.acadmaintver);
    assert!((doc1.header.cdate - doc2.header.cdate).abs() < 1e-15);
    assert_eq!(doc1.header.lastsavedby, doc2.header.lastsavedby);
    assert_eq!(doc1.header.menu, doc2.header.menu);
}

#[test]
fn header_legacy_uses_render_meta_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.pelevation, 0.0);
    assert!((doc.header.facetres - 0.5).abs() < 1e-10);
    assert_eq!(doc.header.isolines, 4);
    assert_eq!(doc.header.textqlty, 50);
    assert_eq!(doc.header.tstackalign, 1);
    assert_eq!(doc.header.tstacksize, 70);
    assert_eq!(doc.header.acadmaintver, 0);
    assert_eq!(doc.header.cdate, 0.0);
    assert_eq!(doc.header.lastsavedby, "");
    assert_eq!(doc.header.menu, ".");
}
