//! Integration tests for the writer's `ensure_image_defs` pre-pass.
//!
//! Covers the case where an IMAGE entity was constructed via the UI or
//! bridge (so it has `file_path` but no `image_def_handle`) and verifies
//! that `write_dxf` emits a standard-conforming DXF: a proper IMAGEDEF
//! in OBJECTS + a code 340 hard-pointer on IMAGE. Also checks
//! idempotency and block-scoped IMAGE handling.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{
    BlockRecord, CadDocument, CadObject, Entity, EntityData, Handle, ObjectData,
};

fn make_image_entity(handle: Handle, image_def_handle: Handle, file_path: &str) -> Entity {
    let mut e = Entity::new(EntityData::Image {
        insertion: [0.0, 0.0, 0.0],
        u_vector: [1.0, 0.0, 0.0],
        v_vector: [0.0, 1.0, 0.0],
        image_size: [128.0, 64.0],
        image_def_handle,
        file_path: file_path.to_string(),
        display_flags: 0,
    });
    e.handle = handle;
    e
}

fn count_imagedefs(doc: &CadDocument) -> usize {
    doc.objects
        .iter()
        .filter(|o| matches!(o.data, ObjectData::ImageDef { .. }))
        .count()
}

fn first_imagedef<'a>(doc: &'a CadDocument) -> Option<(Handle, &'a str)> {
    doc.objects.iter().find_map(|o| match &o.data {
        ObjectData::ImageDef { file_name, .. } => Some((o.handle, file_name.as_str())),
        _ => None,
    })
}

fn first_image_handle(doc: &CadDocument) -> Option<Handle> {
    doc.entities.iter().find_map(|e| match &e.data {
        EntityData::Image {
            image_def_handle, ..
        } => Some(*image_def_handle),
        _ => None,
    })
}

#[test]
fn ensure_creates_imagedef_for_top_level_image_with_file_path_only() {
    let mut doc = CadDocument::new();
    doc.entities.push(make_image_entity(
        Handle::new(0x300),
        Handle::NULL,
        "solo.png",
    ));

    assert_eq!(
        count_imagedefs(&doc),
        0,
        "test precondition: no IMAGEDEF before write"
    );

    let text = write_dxf(&doc).expect("write_dxf must succeed");
    let restored = read_dxf(&text).expect("read_dxf must succeed");

    assert_eq!(
        count_imagedefs(&restored),
        1,
        "writer must auto-create exactly one IMAGEDEF for the unlinked IMAGE"
    );
    let (imagedef_handle, file_name) =
        first_imagedef(&restored).expect("IMAGEDEF must be present after roundtrip");
    assert_eq!(file_name, "solo.png");
    let image_handle = first_image_handle(&restored).expect("IMAGE must be present after roundtrip");
    assert_ne!(image_handle, Handle::NULL, "IMAGE must now carry a 340 link");
    assert_eq!(
        image_handle, imagedef_handle,
        "IMAGE.image_def_handle must match IMAGEDEF.handle"
    );
}

#[test]
fn ensure_skips_image_with_empty_file_path() {
    let mut doc = CadDocument::new();
    doc.entities.push(make_image_entity(
        Handle::new(0x301),
        Handle::NULL,
        "",
    ));

    let text = write_dxf(&doc).expect("write_dxf");
    let restored = read_dxf(&text).expect("read_dxf");

    assert_eq!(
        count_imagedefs(&restored),
        0,
        "writer must NOT auto-create IMAGEDEF when file_path is empty"
    );
    assert_eq!(
        first_image_handle(&restored),
        Some(Handle::NULL),
        "IMAGE with no file_path must stay unlinked"
    );
}

#[test]
fn ensure_skips_image_that_already_has_handle() {
    let mut doc = CadDocument::new();
    let preset_imagedef_handle = Handle::new(0xABC);

    doc.entities.push(make_image_entity(
        Handle::new(0x302),
        preset_imagedef_handle,
        "preset.png",
    ));
    doc.objects.push(CadObject {
        handle: preset_imagedef_handle,
        owner_handle: Handle::NULL,
        data: ObjectData::ImageDef {
            file_name: "preset.png".to_string(),
            image_size: [128.0, 64.0],
            pixel_size: [0.5, 0.5],
            class_version: 0,
            image_is_loaded: true,
            resolution_unit: 2,
        },
    });

    let text = write_dxf(&doc).expect("write_dxf");
    let restored = read_dxf(&text).expect("read_dxf");

    assert_eq!(
        count_imagedefs(&restored),
        1,
        "preset IMAGEDEF must survive without a duplicate being appended"
    );
    let image_handle = first_image_handle(&restored).expect("IMAGE present");
    assert_eq!(
        image_handle, preset_imagedef_handle,
        "preset handle must be preserved"
    );
}

#[test]
fn ensure_handles_image_inside_block_record() {
    let mut doc = CadDocument::new();
    let br_handle = doc.allocate_handle();
    let mut block = BlockRecord::new(br_handle, "TestBlock");
    block.block_entity_handle = doc.allocate_handle();
    block.entities.push(make_image_entity(
        doc.allocate_handle(),
        Handle::NULL,
        "inside_block.png",
    ));
    doc.block_records.insert(br_handle, block);

    let text = write_dxf(&doc).expect("write_dxf");
    let restored = read_dxf(&text).expect("read_dxf");

    let imagedef_count = count_imagedefs(&restored);
    assert_eq!(
        imagedef_count, 1,
        "writer must auto-create IMAGEDEF for block-scoped IMAGE"
    );
    let (imagedef_handle, file_name) = first_imagedef(&restored).expect("IMAGEDEF present");
    assert_eq!(file_name, "inside_block.png");

    let restored_block = restored
        .block_records
        .values()
        .find(|br| br.name == "TestBlock")
        .expect("block record must roundtrip");
    let block_image_handle = restored_block.entities.iter().find_map(|e| match &e.data {
        EntityData::Image {
            image_def_handle, ..
        } => Some(*image_def_handle),
        _ => None,
    });
    assert_eq!(
        block_image_handle,
        Some(imagedef_handle),
        "block-scoped IMAGE must link to the auto-created IMAGEDEF"
    );
}

#[test]
fn ensure_auto_created_imagedef_is_readable_after_roundtrip() {
    let mut doc = CadDocument::new();
    doc.entities.push(make_image_entity(
        Handle::new(0x304),
        Handle::NULL,
        "resolved.jpg",
    ));

    let text1 = write_dxf(&doc).expect("first write");
    let doc2 = read_dxf(&text1).expect("first read");
    let text2 = write_dxf(&doc2).expect("second write — should be idempotent");
    let doc3 = read_dxf(&text2).expect("second read");

    // After the first round-trip the IMAGE already has a handle, so the
    // second write should NOT add any extra IMAGEDEF. Both docs should
    // therefore carry exactly one.
    assert_eq!(count_imagedefs(&doc2), 1);
    assert_eq!(
        count_imagedefs(&doc3),
        1,
        "ensure_image_defs must be idempotent across successive writes"
    );

    let file_path = doc3
        .entities
        .iter()
        .find_map(|e| match &e.data {
            EntityData::Image { file_path, .. } => Some(file_path.clone()),
            _ => None,
        })
        .expect("IMAGE present after two-stage roundtrip");
    assert_eq!(file_path, "resolved.jpg");

    let image_handle_2 = first_image_handle(&doc2).expect("doc2 IMAGE");
    let image_handle_3 = first_image_handle(&doc3).expect("doc3 IMAGE");
    assert_ne!(image_handle_2, Handle::NULL);
    assert_eq!(
        image_handle_2, image_handle_3,
        "handle must be stable across idempotent second write"
    );
}
