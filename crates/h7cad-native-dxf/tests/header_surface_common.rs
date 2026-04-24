use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_surface_common() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$SURFTAB1\n 70\n    12\n",
        "  9\n$SURFTAB2\n 70\n     8\n",
        "  9\n$SURFTYPE\n 70\n     5\n",
        "  9\n$SURFU\n 70\n    10\n",
        "  9\n$SURFV\n 70\n    10\n",
        "  9\n$PFACEVMAX\n 70\n     3\n",
        "  9\n$MEASUREMENT\n 70\n     1\n",
        "  9\n$EXTNAMES\n290\n     0\n",
        "  9\n$WORLDVIEW\n 70\n     0\n",
        "  9\n$UNITMODE\n 70\n     1\n",
        "  9\n$SPLMAXDEG\n 70\n     7\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_surface_and_common_vars() {
    let dxf = make_dxf_with_surface_common();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.surftab1, 12);
    assert_eq!(doc.header.surftab2, 8);
    assert_eq!(doc.header.surftype, 5);
    assert_eq!(doc.header.surfu, 10);
    assert_eq!(doc.header.surfv, 10);
    assert_eq!(doc.header.pfacevmax, 3);
    assert_eq!(doc.header.measurement, 1);
    assert!(!doc.header.extnames);
    assert_eq!(doc.header.worldview, 0);
    assert_eq!(doc.header.unitmode, 1);
    assert_eq!(doc.header.splmaxdeg, 7);
}

#[test]
fn header_writes_surface_and_common_vars() {
    let dxf = make_dxf_with_surface_common();
    let doc = read_dxf(&dxf).unwrap();
    let output = write_dxf(&doc).unwrap();
    for var in &[
        "$SURFTAB1", "$SURFTAB2", "$SURFTYPE", "$SURFU", "$SURFV",
        "$PFACEVMAX", "$MEASUREMENT", "$EXTNAMES", "$WORLDVIEW",
        "$UNITMODE", "$SPLMAXDEG",
    ] {
        assert!(output.contains(var), "output must contain {var}");
    }
}

#[test]
fn header_roundtrip_preserves_surface_and_common() {
    let dxf = make_dxf_with_surface_common();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.surftab1, doc2.header.surftab1);
    assert_eq!(doc1.header.surftab2, doc2.header.surftab2);
    assert_eq!(doc1.header.surftype, doc2.header.surftype);
    assert_eq!(doc1.header.surfu, doc2.header.surfu);
    assert_eq!(doc1.header.surfv, doc2.header.surfv);
    assert_eq!(doc1.header.pfacevmax, doc2.header.pfacevmax);
    assert_eq!(doc1.header.measurement, doc2.header.measurement);
    assert_eq!(doc1.header.extnames, doc2.header.extnames);
    assert_eq!(doc1.header.worldview, doc2.header.worldview);
    assert_eq!(doc1.header.unitmode, doc2.header.unitmode);
    assert_eq!(doc1.header.splmaxdeg, doc2.header.splmaxdeg);
}

#[test]
fn header_legacy_file_uses_correct_surface_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.surftab1, 6);
    assert_eq!(doc.header.surftab2, 6);
    assert_eq!(doc.header.surftype, 6);
    assert_eq!(doc.header.surfu, 6);
    assert_eq!(doc.header.surfv, 6);
    assert_eq!(doc.header.pfacevmax, 4);
    assert_eq!(doc.header.measurement, 0);
    assert!(doc.header.extnames);
    assert_eq!(doc.header.worldview, 1);
    assert_eq!(doc.header.unitmode, 0);
    assert_eq!(doc.header.splmaxdeg, 5);
}
