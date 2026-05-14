use h7cad_native_dxf::{
    write_dxf_bytes_strict, write_dxf_strict, write_dxf_string, DxfOutputFormat, DxfWriteError,
};
use h7cad_native_model::{CadDocument, CadObject, Entity, EntityData, Handle, ObjectData};

#[test]
fn write_dxf_strict_returns_ok_for_minimal_doc() {
    let doc = CadDocument::new();
    let result = write_dxf_strict(&doc);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("$ACADVER"));
    assert!(output.contains("EOF"));
}

#[test]
fn write_dxf_strict_matches_write_dxf_string() {
    let doc = CadDocument::new();
    let strict_output = write_dxf_strict(&doc).unwrap();
    let legacy_output = write_dxf_string(&doc).unwrap();
    assert_eq!(strict_output, legacy_output);
}

#[test]
fn write_dxf_bytes_strict_ascii_matches_strict_text_output() {
    let doc = CadDocument::new();
    let bytes = write_dxf_bytes_strict(&doc, DxfOutputFormat::Ascii).unwrap();
    let text = write_dxf_strict(&doc).unwrap();

    assert_eq!(bytes, text.into_bytes());
}

#[test]
fn write_dxf_bytes_strict_rejects_binary_output_explicitly() {
    let doc = CadDocument::new();
    let err = write_dxf_bytes_strict(&doc, DxfOutputFormat::Binary)
        .expect_err("binary DXF writer should be explicit unsupported");

    assert!(
        matches!(&err, DxfWriteError::Unsupported(message) if message.contains("binary DXF output")),
        "unexpected binary writer error: {err}"
    );
}

#[test]
fn dxf_write_error_display_roundtrip() {
    let err = DxfWriteError::InvalidDocument("test error".to_string());
    assert_eq!(err.to_string(), "invalid document: test error");

    let err = DxfWriteError::Unsupported("binary DXF".to_string());
    assert_eq!(err.to_string(), "unsupported: binary DXF");

    let err = DxfWriteError::Io("write failed".to_string());
    assert_eq!(err.to_string(), "io: write failed");

    let from_str: DxfWriteError = "auto wrap".into();
    assert_eq!(
        from_str,
        DxfWriteError::InvalidDocument("auto wrap".to_string())
    );

    let from_string: DxfWriteError = String::from("string wrap").into();
    assert_eq!(
        from_string,
        DxfWriteError::InvalidDocument("string wrap".to_string())
    );
}

#[test]
fn write_dxf_strict_rejects_missing_imagedef_link() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Image {
        insertion: [0.0, 0.0, 0.0],
        u_vector: [1.0, 0.0, 0.0],
        v_vector: [0.0, 1.0, 0.0],
        image_size: [10.0, 20.0],
        image_def_handle: Handle::new(0xAA),
        file_path: String::new(),
        display_flags: 1,
    }));

    let err = write_dxf_strict(&doc).expect_err("missing IMAGEDEF should fail strict write");
    assert!(
        matches!(&err, DxfWriteError::InvalidDocument(message) if message.contains("IMAGEDEF") && message.contains("AA")),
        "unexpected strict writer error: {err}"
    );
}

#[test]
fn write_dxf_strict_auto_creates_missing_imagedef_for_inline_image_path() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Image {
        insertion: [0.0, 0.0, 0.0],
        u_vector: [1.0, 0.0, 0.0],
        v_vector: [0.0, 1.0, 0.0],
        image_size: [10.0, 20.0],
        image_def_handle: Handle::NULL,
        file_path: "image.png".into(),
        display_flags: 1,
    }));

    let output = write_dxf_strict(&doc).expect("inline image path should be promoted to IMAGEDEF");
    assert!(output.contains("IMAGEDEF"));
    assert!(output.contains("image.png"));
}

#[test]
fn write_dxf_strict_rejects_layout_object_missing_block_record() {
    let mut doc = CadDocument::new();
    doc.objects.push(CadObject {
        handle: Handle::new(0x200),
        owner_handle: Handle::NULL,
        data: ObjectData::Layout {
            name: "BrokenLayout".into(),
            tab_order: 2,
            block_record_handle: Handle::new(0xBEEF),
            plot_paper_size: [210.0, 297.0],
            plot_origin: [0.0, 0.0],
        },
    });

    let err =
        write_dxf_strict(&doc).expect_err("missing layout block record should fail strict write");
    assert!(
        matches!(&err, DxfWriteError::InvalidDocument(message) if message.contains("BrokenLayout") && message.contains("BEEF")),
        "unexpected strict writer error: {err}"
    );

    assert!(
        write_dxf_string(&doc).is_ok(),
        "legacy writer should keep accepting documents that strict mode rejects"
    );
}
