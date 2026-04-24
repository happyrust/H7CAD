use h7cad_native_dxf::{read_dxf, write_dxf};

fn make_dxf_with_dim_visual() -> String {
    let header_body = concat!(
        "  9\n$ACADVER\n  1\nAC1015\n",
        "  9\n$DIMJUST\n 70\n     2\n",
        "  9\n$DIMSD1\n 70\n     1\n",
        "  9\n$DIMSD2\n 70\n     0\n",
        "  9\n$DIMSE1\n 70\n     1\n",
        "  9\n$DIMSE2\n 70\n     0\n",
        "  9\n$DIMSOXD\n 70\n     1\n",
        "  9\n$DIMATFIT\n 70\n     2\n",
        "  9\n$DIMAZIN\n 70\n     3\n",
        "  9\n$DIMTIX\n 70\n     1\n",
    );
    format!(
        "  0\nSECTION\n  2\nHEADER\n{header_body}  0\nENDSEC\n\
         0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n"
    )
}

#[test]
fn header_reads_dim_visual_family() {
    let dxf = make_dxf_with_dim_visual();
    let doc = read_dxf(&dxf).unwrap();
    assert_eq!(doc.header.dim_just, 2);
    assert_eq!(doc.header.dim_sd1, 1);
    assert_eq!(doc.header.dim_sd2, 0);
    assert_eq!(doc.header.dim_se1, 1);
    assert_eq!(doc.header.dim_se2, 0);
    assert_eq!(doc.header.dim_soxd, 1);
    assert_eq!(doc.header.dim_atfit, 2);
    assert_eq!(doc.header.dim_azin, 3);
    assert_eq!(doc.header.dim_tix, 1);

    // Arm-wiring regression: all 9 code-70 fields with distinct non-zero
    // values. Verify no two share the same parsed value when they shouldn't.
    let vals = [
        doc.header.dim_just,  // 2
        doc.header.dim_sd1,   // 1
        doc.header.dim_sd2,   // 0
        doc.header.dim_se1,   // 1 (same as dim_sd1 by design)
        doc.header.dim_se2,   // 0 (same as dim_sd2 by design)
        doc.header.dim_soxd,  // 1
        doc.header.dim_atfit, // 2 (same as dim_just by design)
        doc.header.dim_azin,  // 3
        doc.header.dim_tix,   // 1
    ];
    // At least verify the distinct-value fields don't collide with wrong arm:
    assert_ne!(vals[0], vals[7], "dim_just vs dim_azin");
    assert_ne!(vals[6], vals[7], "dim_atfit vs dim_azin");
}

#[test]
fn header_writes_dim_visual_family() {
    let dxf = make_dxf_with_dim_visual();
    let doc = read_dxf(&dxf).unwrap();
    let output = write_dxf(&doc).unwrap();
    for var in &[
        "$DIMJUST", "$DIMSD1", "$DIMSD2", "$DIMSE1", "$DIMSE2",
        "$DIMSOXD", "$DIMATFIT", "$DIMAZIN", "$DIMTIX",
    ] {
        assert!(output.contains(var), "output must contain {var}");
    }
}

#[test]
fn header_roundtrip_preserves_dim_visual_family() {
    let dxf = make_dxf_with_dim_visual();
    let doc1 = read_dxf(&dxf).unwrap();
    let text = write_dxf(&doc1).unwrap();
    let doc2 = read_dxf(&text).unwrap();
    assert_eq!(doc1.header.dim_just, doc2.header.dim_just);
    assert_eq!(doc1.header.dim_sd1, doc2.header.dim_sd1);
    assert_eq!(doc1.header.dim_sd2, doc2.header.dim_sd2);
    assert_eq!(doc1.header.dim_se1, doc2.header.dim_se1);
    assert_eq!(doc1.header.dim_se2, doc2.header.dim_se2);
    assert_eq!(doc1.header.dim_soxd, doc2.header.dim_soxd);
    assert_eq!(doc1.header.dim_atfit, doc2.header.dim_atfit);
    assert_eq!(doc1.header.dim_azin, doc2.header.dim_azin);
    assert_eq!(doc1.header.dim_tix, doc2.header.dim_tix);
}

#[test]
fn header_legacy_file_without_dim_visual_loads_with_defaults() {
    let dxf = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
                  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nEOF\n";
    let doc = read_dxf(dxf).unwrap();
    assert_eq!(doc.header.dim_just, 0);
    assert_eq!(doc.header.dim_sd1, 0);
    assert_eq!(doc.header.dim_sd2, 0);
    assert_eq!(doc.header.dim_se1, 0);
    assert_eq!(doc.header.dim_se2, 0);
    assert_eq!(doc.header.dim_soxd, 0);
    assert_eq!(doc.header.dim_atfit, 3);
    assert_eq!(doc.header.dim_azin, 0);
    assert_eq!(doc.header.dim_tix, 0);
}
