use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_pucs_osmode() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$PUCSBASE\n  2\nCustomPUCS\n",
        "  9\n$PUCSNAME\n  2\nMyPaperUCS\n",
        "  9\n$PUCSORG\n 10\n1.0\n 20\n2.0\n 30\n3.0\n",
        "  9\n$PUCSXDIR\n 10\n0.0\n 20\n1.0\n 30\n0.0\n",
        "  9\n$PUCSYDIR\n 10\n0.0\n 20\n0.0\n 30\n1.0\n",
        "  9\n$DIMPOST\n  1\nmm\n",
        "  9\n$DIMLUNIT\n 70\n     4\n",
        "  9\n$OSMODE\n 70\n   175\n",
        "  9\n$PICKSTYLE\n 70\n     2\n",
        "  9\n$LIMCHECK\n 70\n     1\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_pucs_and_osmode() {
    let dxf = make_dxf_with_pucs_osmode();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.pucsbase, "CustomPUCS");
    assert_eq!(doc.header.pucsname, "MyPaperUCS");
    assert_eq!(doc.header.pucsorg, [1.0, 2.0, 3.0]);
    assert_eq!(doc.header.pucsxdir, [0.0, 1.0, 0.0]);
    assert_eq!(doc.header.pucsydir, [0.0, 0.0, 1.0]);
    assert_eq!(doc.header.dim_post, "mm");
    assert_eq!(doc.header.dim_lunit, 4);
    assert_eq!(doc.header.osmode, 175);
    assert_eq!(doc.header.pickstyle, 2);
    assert_eq!(doc.header.limcheck, 1);
}

#[test]
fn header_roundtrip_preserves_pucs_and_osmode() {
    let dxf = make_dxf_with_pucs_osmode();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.pucsbase, doc2.header.pucsbase);
    assert_eq!(doc1.header.pucsname, doc2.header.pucsname);
    assert_eq!(doc1.header.pucsorg, doc2.header.pucsorg);
    assert_eq!(doc1.header.pucsxdir, doc2.header.pucsxdir);
    assert_eq!(doc1.header.pucsydir, doc2.header.pucsydir);
    assert_eq!(doc1.header.dim_post, doc2.header.dim_post);
    assert_eq!(doc1.header.dim_lunit, doc2.header.dim_lunit);
    assert_eq!(doc1.header.osmode, doc2.header.osmode);
    assert_eq!(doc1.header.pickstyle, doc2.header.pickstyle);
    assert_eq!(doc1.header.limcheck, doc2.header.limcheck);
}

#[test]
fn header_legacy_file_uses_pucs_osmode_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.pucsbase, "");
    assert_eq!(doc.header.pucsname, "");
    assert_eq!(doc.header.pucsorg, [0.0, 0.0, 0.0]);
    assert_eq!(doc.header.pucsxdir, [1.0, 0.0, 0.0]);
    assert_eq!(doc.header.pucsydir, [0.0, 1.0, 0.0]);
    assert_eq!(doc.header.dim_post, "");
    assert_eq!(doc.header.dim_lunit, 2);
    assert_eq!(doc.header.osmode, 4133);
    assert_eq!(doc.header.pickstyle, 1);
    assert_eq!(doc.header.limcheck, 0);
}
