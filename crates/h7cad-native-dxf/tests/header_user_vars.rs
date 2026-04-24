use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_user_vars() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$USERI1\n 70\n    10\n",
        "  9\n$USERI2\n 70\n    20\n",
        "  9\n$USERI3\n 70\n    30\n",
        "  9\n$USERI4\n 70\n    40\n",
        "  9\n$USERI5\n 70\n    50\n",
        "  9\n$USERR1\n 40\n1.1\n",
        "  9\n$USERR2\n 40\n2.2\n",
        "  9\n$USERR3\n 40\n3.3\n",
        "  9\n$USERR4\n 40\n4.4\n",
        "  9\n$USERR5\n 40\n5.5\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_user_vars() {
    let doc = read_dxf(&make_dxf_with_user_vars()).unwrap();
    assert_eq!(doc.header.useri1, 10);
    assert_eq!(doc.header.useri2, 20);
    assert_eq!(doc.header.useri3, 30);
    assert_eq!(doc.header.useri4, 40);
    assert_eq!(doc.header.useri5, 50);
    assert!((doc.header.userr1 - 1.1).abs() < 1e-10);
    assert!((doc.header.userr2 - 2.2).abs() < 1e-10);
    assert!((doc.header.userr3 - 3.3).abs() < 1e-10);
    assert!((doc.header.userr4 - 4.4).abs() < 1e-10);
    assert!((doc.header.userr5 - 5.5).abs() < 1e-10);
}

#[test]
fn header_roundtrip_preserves_user_vars() {
    let doc1 = read_dxf(&make_dxf_with_user_vars()).unwrap();
    let doc2 = read_dxf(&write_dxf(&doc1).unwrap()).unwrap();
    assert_eq!(doc1.header.useri1, doc2.header.useri1);
    assert_eq!(doc1.header.useri5, doc2.header.useri5);
    assert!((doc1.header.userr1 - doc2.header.userr1).abs() < 1e-15);
    assert!((doc1.header.userr5 - doc2.header.userr5).abs() < 1e-15);
}

#[test]
fn header_legacy_user_vars_default_zero() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.useri1, 0);
    assert_eq!(doc.header.userr1, 0.0);
}
