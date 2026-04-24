use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{CadDocument, Entity, EntityData};

#[test]
fn wipeout_clip_vertices_survive_roundtrip() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Wipeout {
        clip_vertices: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]],
        elevation: 5.5,
    }));

    let dxf_text = write_dxf(&doc).unwrap();
    let doc2 = read_dxf(&dxf_text).unwrap();

    assert_eq!(doc2.entities.len(), 1);
    if let EntityData::Wipeout {
        clip_vertices,
        elevation,
    } = &doc2.entities[0].data
    {
        assert_eq!(clip_vertices.len(), 4);
        assert_eq!(clip_vertices[0], [0.0, 0.0]);
        assert_eq!(clip_vertices[1], [100.0, 0.0]);
        assert_eq!(clip_vertices[2], [100.0, 50.0]);
        assert_eq!(clip_vertices[3], [0.0, 50.0]);
        assert_eq!(*elevation, 5.5);
    } else {
        panic!("expected Wipeout entity data");
    }
}

#[test]
fn wipeout_empty_clip_vertices_roundtrip() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Wipeout {
        clip_vertices: vec![],
        elevation: 0.0,
    }));

    let dxf_text = write_dxf(&doc).unwrap();
    let doc2 = read_dxf(&dxf_text).unwrap();

    assert_eq!(doc2.entities.len(), 1);
    if let EntityData::Wipeout {
        clip_vertices,
        elevation,
    } = &doc2.entities[0].data
    {
        assert!(clip_vertices.is_empty());
        assert_eq!(*elevation, 0.0);
    } else {
        panic!("expected Wipeout entity data");
    }
}
