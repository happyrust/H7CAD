use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_paper_space_misc() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$PSTYLEMODE\n 70\n     0\n",
        "  9\n$TILEMODE\n 70\n     0\n",
        "  9\n$MAXACTVP\n 70\n    48\n",
        "  9\n$PSVPSCALE\n 40\n2.5\n",
        "  9\n$TREEDEPTH\n 70\n  3020\n",
        "  9\n$VISRETAIN\n 70\n     0\n",
        "  9\n$DELOBJ\n 70\n     0\n",
        "  9\n$PROXYGRAPHICS\n 70\n     0\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_paper_space_and_misc_flags() {
    let dxf = make_dxf_with_paper_space_misc();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.pstylemode, 0);
    assert_eq!(doc.header.tilemode, 0);
    assert_eq!(doc.header.maxactvp, 48);
    assert!((doc.header.psvpscale - 2.5).abs() < 1e-10);
    assert_eq!(doc.header.treedepth, 3020);
    assert_eq!(doc.header.visretain, 0);
    assert_eq!(doc.header.delobj, 0);
    assert_eq!(doc.header.proxygraphics, 0);
}

#[test]
fn header_writes_paper_space_and_misc_flags() {
    let dxf = make_dxf_with_paper_space_misc();
    let doc = read_dxf(&dxf).unwrap();
    let output = write_dxf(&doc).unwrap();
    for var in &[
        "$PSTYLEMODE", "$TILEMODE", "$MAXACTVP", "$PSVPSCALE",
        "$TREEDEPTH", "$VISRETAIN", "$DELOBJ", "$PROXYGRAPHICS",
    ] {
        assert!(output.contains(var), "output must contain {var}");
    }
}

#[test]
fn header_roundtrip_preserves_paper_space_and_misc() {
    let dxf = make_dxf_with_paper_space_misc();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.pstylemode, doc2.header.pstylemode);
    assert_eq!(doc1.header.tilemode, doc2.header.tilemode);
    assert_eq!(doc1.header.maxactvp, doc2.header.maxactvp);
    assert!((doc1.header.psvpscale - doc2.header.psvpscale).abs() < 1e-15);
    assert_eq!(doc1.header.treedepth, doc2.header.treedepth);
    assert_eq!(doc1.header.visretain, doc2.header.visretain);
    assert_eq!(doc1.header.delobj, doc2.header.delobj);
    assert_eq!(doc1.header.proxygraphics, doc2.header.proxygraphics);
}

#[test]
fn header_legacy_file_uses_correct_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.pstylemode, 1);
    assert_eq!(doc.header.tilemode, 1);
    assert_eq!(doc.header.maxactvp, 64);
    assert_eq!(doc.header.psvpscale, 0.0);
    assert_eq!(doc.header.treedepth, 3020);
    assert_eq!(doc.header.visretain, 1);
    assert_eq!(doc.header.delobj, 1);
    assert_eq!(doc.header.proxygraphics, 1);
}
