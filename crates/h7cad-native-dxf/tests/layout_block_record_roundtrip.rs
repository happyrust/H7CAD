use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{CadDocument, CadObject, Handle, ObjectData};

#[test]
fn layout_block_record_handle_survives_roundtrip() {
    let mut doc = CadDocument::new();
    let br_handle = Handle::new(0xAB);

    doc.objects.push(CadObject {
        handle: Handle::new(0x100),
        owner_handle: Handle::NULL,
        data: ObjectData::Layout {
            name: "TestLayout".into(),
            tab_order: 2,
            block_record_handle: br_handle,
            plot_paper_size: [210.0, 297.0],
            plot_origin: [5.0, 10.0],
        },
    });

    let dxf_text = write_dxf(&doc).unwrap();

    assert!(
        dxf_text.contains("340"),
        "writer must emit code 340 for block_record_handle"
    );

    let doc2 = read_dxf(&dxf_text).unwrap();
    let layout = doc2.objects.iter().find(|o| {
        matches!(&o.data, ObjectData::Layout { name, .. } if name == "TestLayout")
    });
    assert!(layout.is_some(), "LAYOUT object must survive roundtrip");

    if let ObjectData::Layout {
        block_record_handle,
        tab_order,
        plot_paper_size,
        plot_origin,
        ..
    } = &layout.unwrap().data
    {
        assert_eq!(
            *block_record_handle, br_handle,
            "block_record_handle must be preserved via code 340"
        );
        assert_eq!(*tab_order, 2);
        assert_eq!(*plot_paper_size, [210.0, 297.0]);
        assert_eq!(*plot_origin, [5.0, 10.0]);
    } else {
        panic!("expected Layout data");
    }
}
