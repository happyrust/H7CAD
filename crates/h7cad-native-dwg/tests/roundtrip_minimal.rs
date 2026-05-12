//! F5.M2 closing tracer-bullet integration tests for the native DWG writer.
//!
//! These tests exercise [`h7cad_native_dwg::write_dwg`] end-to-end
//! against [`h7cad_native_dwg::read_dwg`]. They are deliberately
//! stricter than the per-module unit tests in `writer/file_header.rs`
//! (which validate byte layout) and `bit_writer.rs` (which validate
//! reader↔writer primitive symmetry): here we lock the *behavioural*
//! contract that a writer-emitted empty AC1015 DWG re-parses into a
//! semantically empty `CadDocument`.
//!
//! See `docs/plans/2026-05-09-dwg-section-composer-empty-payloads-plan.md`
//! §5 for the test matrix this file implements.

use h7cad_native_dwg::{
    read_dwg, write_dwg, DwgFileHeader, DwgWriteError, KnownSection, SectionMap,
    AC1015_EMPTY_DWG_MIN_LEN, AC1015_FILE_HEADER_PREFIX_LEN, AC1015_KNOWN_SECTION_COUNT,
    AC1015_SECTION_LOCATOR_ENTRY_LEN,
};
use h7cad_native_model::{CadDocument, Entity, EntityData, Handle, LwVertex};

#[test]
fn write_dwg_empty_doc_round_trips_through_read_dwg() {
    // Tracer bullet (per `2026-05-09-dwg-section-composer-empty-payloads-plan.md`
    // §4.1 step 1): an empty `CadDocument` writes to bytes and the
    // bytes re-parse into a `CadDocument` whose entity state is
    // observably empty and whose default-table state matches a freshly
    // constructed document. `CadDocument::new()` itself seeds the
    // default `0` layer, the three default linetypes, etc., so we
    // assert those tables remain *unchanged* by the round-trip — i.e.
    // the writer/reader pair did not invent or drop default-table
    // entries — rather than asserting they are empty.

    let doc = CadDocument::new();
    assert!(
        doc.entities.is_empty(),
        "freshly constructed doc must have no entities; otherwise this \
         test exercises the wrong precondition"
    );

    let bytes = write_dwg(&doc).expect("empty doc must write successfully");
    assert!(
        bytes.len() >= AC1015_EMPTY_DWG_MIN_LEN,
        "empty AC1015 DWG must be at least {AC1015_EMPTY_DWG_MIN_LEN} bytes (file header prefix + 6 directory entries)"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted empty DWG must read back");
    assert!(
        parsed.entities.is_empty(),
        "round-tripped doc should have no entities, got {} entities",
        parsed.entities.len()
    );
    let fresh = CadDocument::new();
    assert_eq!(
        parsed.layers, fresh.layers,
        "round-tripped layers should match a freshly constructed doc \
         (writer must not invent or drop default-table entries)"
    );
    assert_eq!(
        parsed.linetypes, fresh.linetypes,
        "round-tripped linetypes should match a freshly constructed doc"
    );
}

#[test]
fn write_dwg_rejects_unimplemented_entity_with_unsupported() {
    // Honesty contract (plan §2 invariant 4): the M4.E writer must
    // reject entity types it does not yet know how to encode. LINE,
    // CIRCLE, ARC, POINT, and LWPOLYLINE are implemented; UNKNOWN / TEXT / etc. still
    // surface a typed `Unsupported` error pointing at F5.M5.
    let mut doc = CadDocument::new();
    doc.entities.push(Entity {
        handle: Handle::new(0x1000),
        owner_handle: Handle::NULL,
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: -1,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Unknown {
            entity_type: "UNSUPPORTED_ENTITY".to_string(),
            raw_codes: Vec::new(),
        },
    });

    let err = write_dwg(&doc).expect_err("unimplemented entity must be rejected at F5.M5");
    match err {
        DwgWriteError::Unsupported(msg) => assert!(
            msg.contains("F5.M5"),
            "Unsupported error must point at F5.M5 milestone; got `{msg}`"
        ),
        other => panic!("expected DwgWriteError::Unsupported, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_line_entity_round_trips_through_read_dwg() {
    // F5.M4.E tracer bullet: the writer can now serialise at least
    // one real entity. Construct a document containing one LINE,
    // give the default `0` layer a non-NULL handle (the M4.C minimal
    // common-header writer requires this), serialise, re-parse, and
    // verify the entity geometry survives.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let line_handle = Handle::new(0x100);
    doc.entities.push(Entity {
        handle: line_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.5,
        color_index: 3,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: true,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Line {
            start: [1.0, 2.0, 0.0],
            end: [4.0, 5.0, 0.0],
        },
    });

    let bytes = write_dwg(&doc).expect("LINE-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted LINE DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, line_handle);
    assert_eq!(entity.color_index, 3);
    assert_eq!(entity.linetype_scale, 0.5);
    assert_eq!(entity.invisible, true);
    match &entity.data {
        EntityData::Line { start, end } => {
            assert_eq!(*start, [1.0, 2.0, 0.0]);
            assert_eq!(*end, [4.0, 5.0, 0.0]);
        }
        other => panic!("expected Line data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_circle_entity_round_trips_through_read_dwg() {
    // F5.M5.E1 tracer bullet: CIRCLE follows the same common-header
    // + object-slice path as LINE, with its own 3BD/BD/BT/BE body.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let circle_handle = Handle::new(0x101);
    doc.entities.push(Entity {
        handle: circle_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.75,
        color_index: 4,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: false,
        transparency: 0,
        thickness: 1.25,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Circle {
            center: [2.0, 3.0, 4.0],
            radius: 5.5,
        },
    });

    let bytes = write_dwg(&doc).expect("CIRCLE-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted CIRCLE DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, circle_handle);
    assert_eq!(entity.color_index, 4);
    assert_eq!(entity.linetype_scale, 0.75);
    assert_eq!(entity.invisible, false);
    assert_eq!(entity.thickness, 1.25);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Circle { center, radius } => {
            assert_eq!(*center, [2.0, 3.0, 4.0]);
            assert_eq!(*radius, 5.5);
        }
        other => panic!("expected Circle data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_arc_entity_round_trips_through_read_dwg() {
    // F5.M5.E2 tracer bullet: ARC reuses the CIRCLE body layout and
    // appends start/end angles.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let arc_handle = Handle::new(0x102);
    doc.entities.push(Entity {
        handle: arc_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.625,
        color_index: 5,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: true,
        transparency: 0,
        thickness: 0.5,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Arc {
            center: [3.0, 4.0, 5.0],
            radius: 6.5,
            start_angle: 0.25,
            end_angle: 2.75,
        },
    });

    let bytes = write_dwg(&doc).expect("ARC-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted ARC DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, arc_handle);
    assert_eq!(entity.color_index, 5);
    assert_eq!(entity.linetype_scale, 0.625);
    assert_eq!(entity.invisible, true);
    assert_eq!(entity.thickness, 0.5);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            assert_eq!(*center, [3.0, 4.0, 5.0]);
            assert_eq!(*radius, 6.5);
            assert_eq!(*start_angle, 0.25);
            assert_eq!(*end_angle, 2.75);
        }
        other => panic!("expected Arc data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_point_entity_round_trips_through_read_dwg() {
    // F5.M5.E3 tracer bullet: POINT carries its location in the body
    // and reuses common entity thickness/extrusion fields.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let point_handle = Handle::new(0x103);
    doc.entities.push(Entity {
        handle: point_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.875,
        color_index: 6,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: false,
        transparency: 0,
        thickness: 0.25,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Point {
            position: [7.0, 8.0, 9.0],
        },
    });

    let bytes = write_dwg(&doc).expect("POINT-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted POINT DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, point_handle);
    assert_eq!(entity.color_index, 6);
    assert_eq!(entity.linetype_scale, 0.875);
    assert_eq!(entity.invisible, false);
    assert_eq!(entity.thickness, 0.25);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Point { position } => {
            assert_eq!(*position, [7.0, 8.0, 9.0]);
        }
        other => panic!("expected Point data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_lwpolyline_entity_round_trips_through_read_dwg() {
    // F5.M5.E4 tracer bullet: LWPOLYLINE is the first variable-length
    // entity body writer. This case covers closed state, constant
    // width, per-vertex bulge/width arrays, and common thickness.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let lwpolyline_handle = Handle::new(0x104);
    let vertices = vec![
        LwVertex {
            x: 1.0,
            y: 2.0,
            bulge: 0.25,
            start_width: 0.1,
            end_width: 0.2,
        },
        LwVertex {
            x: 3.0,
            y: 4.0,
            bulge: 0.0,
            start_width: 0.3,
            end_width: 0.4,
        },
    ];
    doc.entities.push(Entity {
        handle: lwpolyline_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.9375,
        color_index: 2,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: true,
        transparency: 0,
        thickness: 0.625,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::LwPolyline {
            vertices: vertices.clone(),
            closed: true,
            constant_width: 0.5,
        },
    });

    let bytes = write_dwg(&doc).expect("LWPOLYLINE-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted LWPOLYLINE DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, lwpolyline_handle);
    assert_eq!(entity.color_index, 2);
    assert_eq!(entity.linetype_scale, 0.9375);
    assert_eq!(entity.invisible, true);
    assert_eq!(entity.thickness, 0.625);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::LwPolyline {
            vertices: actual,
            closed,
            constant_width,
        } => {
            assert_eq!(*actual, vertices);
            assert_eq!(*closed, true);
            assert_eq!(*constant_width, 0.5);
        }
        other => panic!("expected LwPolyline data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_text_entity_round_trips_through_read_dwg() {
    // F5.M5.E5 tracer bullet: TEXT is the first string-bearing entity
    // body writer. Style table records are still outside this writer
    // milestone, so the reader falls back to the style handle name.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;
    let standard_style_handle = Handle::new(0x20);
    doc.text_styles
        .get_mut("Standard")
        .expect("CadDocument::new seeds the default `Standard` text style")
        .handle = standard_style_handle;

    let text_handle = Handle::new(0x105);
    doc.entities.push(Entity {
        handle: text_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.125,
        color_index: 1,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: false,
        transparency: 0,
        thickness: 0.125,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Text {
            insertion: [1.0, 2.0, 3.0],
            height: 2.5,
            value: "Label-1".to_string(),
            rotation: 0.5,
            style_name: "Standard".to_string(),
            width_factor: 0.75,
            oblique_angle: 0.1,
            horizontal_alignment: 1,
            vertical_alignment: 2,
            alignment_point: Some([4.0, 5.0, 3.0]),
        },
    });

    let bytes = write_dwg(&doc).expect("TEXT-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted TEXT DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, text_handle);
    assert_eq!(entity.color_index, 1);
    assert_eq!(entity.linetype_scale, 1.125);
    assert_eq!(entity.invisible, false);
    assert_eq!(entity.thickness, 0.125);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Text {
            insertion,
            height,
            value,
            rotation,
            style_name,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            alignment_point,
        } => {
            assert_eq!(*insertion, [1.0, 2.0, 3.0]);
            assert_eq!(*height, 2.5);
            assert_eq!(value, "Label-1");
            assert_eq!(*rotation, 0.5);
            assert_eq!(style_name, "$STYLE_20");
            assert_eq!(*width_factor, 0.75);
            assert_eq!(*oblique_angle, 0.1);
            assert_eq!(*horizontal_alignment, 1);
            assert_eq!(*vertical_alignment, 2);
            assert_eq!(*alignment_point, Some([4.0, 5.0, 3.0]));
        }
        other => panic!("expected Text data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_attrib_entity_round_trips_through_read_dwg() {
    // F5.M5.E6 tracer bullet: ATTRIB is TEXT-like, but the native
    // model currently preserves only tag/value/insertion/height plus
    // common thickness/extrusion.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;
    doc.text_styles
        .get_mut("Standard")
        .expect("CadDocument::new seeds the default `Standard` text style")
        .handle = Handle::new(0x20);

    let attrib_handle = Handle::new(0x106);
    doc.entities.push(Entity {
        handle: attrib_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.25,
        color_index: 8,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: true,
        transparency: 0,
        thickness: 0.25,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Attrib {
            tag: "PIPE_ID".to_string(),
            value: "P-101".to_string(),
            insertion: [1.0, 2.0, 3.0],
            height: 2.5,
        },
    });

    let bytes = write_dwg(&doc).expect("ATTRIB-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted ATTRIB DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, attrib_handle);
    assert_eq!(entity.color_index, 8);
    assert_eq!(entity.linetype_scale, 1.25);
    assert_eq!(entity.invisible, true);
    assert_eq!(entity.thickness, 0.25);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Attrib {
            tag,
            value,
            insertion,
            height,
        } => {
            assert_eq!(tag, "PIPE_ID");
            assert_eq!(value, "P-101");
            assert_eq!(*insertion, [1.0, 2.0, 3.0]);
            assert_eq!(*height, 2.5);
        }
        other => panic!("expected Attrib data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_solid_entity_round_trips_through_read_dwg() {
    // F5.M5.E7 tracer bullet: SOLID is the first 4-corner planar
    // entity. The wire format shares a single BD elevation across
    // all four corners (2RD x/y pairs), so the writer requires the
    // caller-supplied corners to agree on z.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let solid_handle = Handle::new(0x107);
    doc.entities.push(Entity {
        handle: solid_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.5,
        color_index: 6,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: false,
        transparency: 0,
        // SOLID writes its own BT thickness on the body bitstream;
        // common-header `thickness` is independent.
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Solid {
            corners: [
                [10.0, 20.0, 5.0],
                [11.0, 20.0, 5.0],
                [11.0, 21.0, 5.0],
                [10.0, 21.0, 5.0],
            ],
            normal: [0.0, 0.0, 1.0],
            thickness: 0.75,
        },
    });

    let bytes = write_dwg(&doc).expect("SOLID-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted SOLID DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, solid_handle);
    assert_eq!(entity.color_index, 6);
    assert_eq!(entity.linetype_scale, 1.5);
    match &entity.data {
        EntityData::Solid {
            corners,
            normal,
            thickness,
        } => {
            assert_eq!(
                *corners,
                [
                    [10.0, 20.0, 5.0],
                    [11.0, 20.0, 5.0],
                    [11.0, 21.0, 5.0],
                    [10.0, 21.0, 5.0],
                ]
            );
            assert_eq!(*normal, [0.0, 0.0, 1.0]);
            assert_eq!(*thickness, 0.75);
        }
        other => panic!("expected Solid data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_face3d_entity_round_trips_through_read_dwg() {
    // F5.M5.E7 tracer bullet: 3DFACE has independent 3BD corners
    // (no shared elevation) and an i16 `invisible_edges` mask. The
    // writer always emits the `has_no_flags = 0` path so the mask
    // round-trips even when zero.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let face3d_handle = Handle::new(0x108);
    doc.entities.push(Entity {
        handle: face3d_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.875,
        color_index: 4,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: true,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Face3D {
            corners: [
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 1.0],
                [2.0, 2.0, 2.0],
                [0.0, 2.0, 1.0],
            ],
            invisible_edges: 0b1010,
        },
    });

    let bytes = write_dwg(&doc).expect("3DFACE-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted 3DFACE DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, face3d_handle);
    assert_eq!(entity.color_index, 4);
    assert_eq!(entity.linetype_scale, 0.875);
    assert_eq!(entity.invisible, true);
    match &entity.data {
        EntityData::Face3D {
            corners,
            invisible_edges,
        } => {
            assert_eq!(
                *corners,
                [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 1.0],
                    [2.0, 2.0, 2.0],
                    [0.0, 2.0, 1.0],
                ]
            );
            assert_eq!(*invisible_edges, 0b1010);
        }
        other => panic!("expected Face3D data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_ray_entity_round_trips_through_read_dwg() {
    // F5.M5.E8 tracer bullet: RAY is the simplest variable-direction
    // entity. Body is `3BD origin + 3BD direction`, identical to
    // XLINE on the wire — the only difference is the object-type code.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let ray_handle = Handle::new(0x109);
    doc.entities.push(Entity {
        handle: ray_handle,
        owner_handle: Handle::new(0x42), // M4.C minimal forces NULL on read
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.125,
        color_index: 2,
        true_color: 0,
        lineweight: 13, // M4.C minimal forces -3 on read
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Ray {
            origin: [3.5, -2.25, 7.0],
            direction: [1.0, 1.0, 1.0],
        },
    });

    let bytes = write_dwg(&doc).expect("RAY-bearing doc writes successfully");
    assert!(
        bytes.len() > AC1015_EMPTY_DWG_MIN_LEN,
        "non-empty doc must be larger than empty baseline"
    );

    let parsed = read_dwg(&bytes).expect("writer-emitted RAY DWG must read back");
    assert_eq!(
        parsed.entities.len(),
        1,
        "expected exactly 1 entity, got {}",
        parsed.entities.len()
    );
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, ray_handle);
    assert_eq!(entity.color_index, 2);
    assert_eq!(entity.linetype_scale, 1.125);
    match &entity.data {
        EntityData::Ray { origin, direction } => {
            assert_eq!(*origin, [3.5, -2.25, 7.0]);
            assert_eq!(*direction, [1.0, 1.0, 1.0]);
        }
        other => panic!("expected Ray data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_xline_entity_round_trips_through_read_dwg() {
    // F5.M5.E8 tracer bullet: XLINE shares RAY's body but emits a
    // distinct object_type. The test reuses RAY's structure on the
    // wire and proves the type-dispatch correctly routes
    // `EntityData::XLine` to `XLINE_OBJECT_TYPE` (40), not RAY (38).
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let xline_handle = Handle::new(0x10A);
    doc.entities.push(Entity {
        handle: xline_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 2.0,
        color_index: 1,
        true_color: 0,
        lineweight: 13,
        invisible: true,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::XLine {
            origin: [0.0, 0.0, 0.0],
            direction: [-1.0, -2.0, -3.0],
        },
    });

    let bytes = write_dwg(&doc).expect("XLINE-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted XLINE DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, xline_handle);
    assert_eq!(entity.color_index, 1);
    assert_eq!(entity.linetype_scale, 2.0);
    assert_eq!(entity.invisible, true);
    match &entity.data {
        EntityData::XLine { origin, direction } => {
            assert_eq!(*origin, [0.0, 0.0, 0.0]);
            assert_eq!(*direction, [-1.0, -2.0, -3.0]);
        }
        other => panic!("expected XLine data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_ellipse_entity_round_trips_through_read_dwg() {
    // F5.M5.E9 tracer bullet: ELLIPSE has 6 fields (3×3BD + 3×BD).
    // The body's extrusion is a plain 3BD triple — *not* the CIRCLE/ARC
    // short-circuit `BE` format — and the writer takes that value from
    // the common-header `entity.extrusion` to match how the reader's
    // `enrich_with_real_entities` propagates it back.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let ellipse_handle = Handle::new(0x10B);
    doc.entities.push(Entity {
        handle: ellipse_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 0.75,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        // Non-canonical extrusion so we prove the body's 3BD triple
        // carries an arbitrary direction, not just OCS Z.
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Ellipse {
            center: [3.5, -2.25, 7.0],
            major_axis: [2.0, 1.0, 0.5],
            ratio: 0.25,
            start_param: 0.25,
            end_param: 2.75,
        },
    });

    let bytes = write_dwg(&doc).expect("ELLIPSE-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted ELLIPSE DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, ellipse_handle);
    assert_eq!(entity.color_index, 7);
    assert_eq!(entity.linetype_scale, 0.75);
    assert_eq!(entity.extrusion, [0.0, 0.0, 1.0]);
    match &entity.data {
        EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            assert_eq!(*center, [3.5, -2.25, 7.0]);
            assert_eq!(*major_axis, [2.0, 1.0, 0.5]);
            assert_eq!(*ratio, 0.25);
            assert_eq!(*start_param, 0.25);
            assert_eq!(*end_param, 2.75);
        }
        other => panic!("expected Ellipse data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_spline_entity_round_trips_through_read_dwg() {
    // F5.M5.E10 tracer bullet: SPLINE is the first writer with
    // variable-length nested arrays (control_points + knots) and a
    // wire-format scenario branch (1 = control-point, 2 = fit-point).
    // This test exercises scenario 1 with non-trivial control points
    // and knots; the writer-internal unit tests cover scenario 2
    // (fit-points) and rational/weights paths.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let spline_handle = Handle::new(0x10C);
    doc.entities.push(Entity {
        handle: spline_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 9,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Spline {
            degree: 3,
            closed: true,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [2.0, 1.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            weights: Vec::new(),
            fit_points: Vec::new(),
            start_tangent: [0.0, 0.0, 0.0],
            end_tangent: [0.0, 0.0, 0.0],
        },
    });

    let bytes = write_dwg(&doc).expect("SPLINE-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted SPLINE DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, spline_handle);
    assert_eq!(entity.color_index, 9);
    match &entity.data {
        EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            start_tangent,
            end_tangent,
        } => {
            assert_eq!(*degree, 3);
            assert!(*closed);
            assert_eq!(knots, &vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
            assert_eq!(
                control_points,
                &vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [2.0, 1.0, 0.0],
                    [3.0, 0.0, 0.0],
                ]
            );
            assert!(weights.is_empty());
            assert!(fit_points.is_empty());
            assert_eq!(*start_tangent, [0.0, 0.0, 0.0]);
            assert_eq!(*end_tangent, [0.0, 0.0, 0.0]);
        }
        other => panic!("expected Spline data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_mtext_entity_round_trips_through_read_dwg() {
    // F5.M5.E11 tracer bullet: MTEXT is the first variable-length-string
    // entity that also emits a style handle into the handle bitstream
    // (hard-owner reference). The wire body carries 13 fields including
    // an `x_direction` triple from which the reader recovers `rotation`
    // via `atan2`.
    //
    // The test pins `rotation = 0.0` so the lossy `rotation →
    // x_direction → rotation` (cos/sin/atan2) chain produces bit-
    // exact zero. The writer's unit tests cover non-zero rotations
    // with an epsilon tolerance.
    //
    // Style-name round-trip is **not** asserted here. The STYLE
    // table writer is still outside this milestone, so the reader's
    // `resolve_style_name` falls back to `$STYLE_<HEX>` — matching
    // the M5.E5 TEXT integration test's known limitation.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;
    doc.text_styles
        .get_mut("Standard")
        .expect("CadDocument::new seeds the default `Standard` text style")
        .handle = Handle::new(0x20);

    let mtext_handle = Handle::new(0x10D);
    doc.entities.push(Entity {
        handle: mtext_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::MText {
            insertion: [10.0, 20.0, 0.0],
            height: 2.5,
            width: 80.0,
            rectangle_height: Some(12.0),
            value: "Hello MText".to_string(),
            rotation: 0.0,
            style_name: "Standard".to_string(),
            attachment_point: 1,
            line_spacing_factor: 1.0,
            drawing_direction: 5,
        },
    });

    let bytes = write_dwg(&doc).expect("MTEXT-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted MTEXT DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, mtext_handle);
    assert_eq!(entity.color_index, 7);
    match &entity.data {
        EntityData::MText {
            insertion,
            height,
            width,
            rectangle_height,
            value,
            rotation,
            style_name,
            attachment_point,
            line_spacing_factor,
            drawing_direction,
        } => {
            assert_eq!(*insertion, [10.0, 20.0, 0.0]);
            assert_eq!(*height, 2.5);
            assert_eq!(*width, 80.0);
            assert_eq!(*rectangle_height, Some(12.0));
            assert_eq!(value, "Hello MText");
            assert_eq!(*rotation, 0.0);
            // STYLE table writer is pending; reader falls back to
            // `$STYLE_<HEX>` based on the style_handle value. Same
            // contract as the M5.E5 TEXT integration test.
            assert_eq!(style_name, "$STYLE_20");
            assert_eq!(*attachment_point, 1);
            assert_eq!(*line_spacing_factor, 1.0);
            assert_eq!(*drawing_direction, 5);
        }
        other => panic!("expected MText data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_insert_entity_round_trips_through_read_dwg() {
    // F5.M5.E12 tracer bullet: INSERT is the first writer that
    // resolves a *block* handle from the BLOCK_RECORD table (vs
    // STYLE table for TEXT/MTEXT). The 2-bit scale_flag and
    // optional DD-default y/z encoding are also new.
    //
    // BLOCK_RECORD table writer is pending, so the reader's
    // `resolve_block_name` falls back to `$BLOCK_<HEX>` —
    // analogous to STYLE table for TEXT/MTEXT.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    // Seed a BLOCK_RECORD so the writer can resolve a handle for it.
    let block_handle = Handle::new(0x80);
    let block_record = h7cad_native_model::BlockRecord::new(block_handle, "MyBlock");
    doc.block_records.insert(block_handle, block_record);

    let insert_handle = Handle::new(0x10E);
    doc.entities.push(Entity {
        handle: insert_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Insert {
            block_name: "MyBlock".to_string(),
            insertion: [10.0, 20.0, 0.0],
            scale: [2.0, 3.0, 4.0],
            rotation: 0.5,
            // M5.E12 known limitation: writer always emits
            // has_attribs=0 and the parsed value is always false.
            has_attribs: false,
            attribs: Vec::new(),
        },
    });

    let bytes = write_dwg(&doc).expect("INSERT-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted INSERT DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, insert_handle);
    assert_eq!(entity.color_index, 7);
    match &entity.data {
        EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
        } => {
            assert_eq!(*insertion, [10.0, 20.0, 0.0]);
            assert_eq!(*scale, [2.0, 3.0, 4.0]);
            assert_eq!(*rotation, 0.5);
            // BLOCK_RECORD table writer pending; reader falls back
            // to `$BLOCK_<HEX>` based on the block_header_handle.
            assert_eq!(block_name, "$BLOCK_80");
            assert!(!*has_attribs);
            assert!(attribs.is_empty());
        }
        other => panic!("expected Insert data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_viewport_entity_round_trips_through_read_dwg() {
    // F5.M5.E21 tracer bullet: VIEWPORT body is minimal (3 fields).
    // The full VIEWPORT spec carries view direction / twist / lens
    // length / frozen layers / etc. which the native reader skips —
    // the writer matches that minimal shape so the round-trip is
    // strict on what the model actually carries.
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let viewport_handle = Handle::new(0x10F);
    doc.entities.push(Entity {
        handle: viewport_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Viewport {
            center: [42.0, -17.5, 3.25],
            width: 100.0,
            height: 50.0,
        },
    });

    let bytes = write_dwg(&doc).expect("VIEWPORT-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted VIEWPORT DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, viewport_handle);
    match &entity.data {
        EntityData::Viewport {
            center,
            width,
            height,
        } => {
            assert_eq!(*center, [42.0, -17.5, 3.25]);
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 50.0);
        }
        other => panic!("expected Viewport data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_solid_hatch_entity_round_trips_through_read_dwg() {
    // F5.M5.E20 tracer bullet (E20a — solid + edge framework):
    // HATCH is the most complex M5 entity. This test exercises the
    // solid_fill=true path with a simple boundary made of two Line
    // edges, validating the core boundary_paths / edges nesting.
    use h7cad_native_model::{HatchBoundaryPath, HatchEdge};
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let hatch_handle = Handle::new(0x110);
    doc.entities.push(Entity {
        handle: hatch_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Hatch {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![
                    HatchEdge::Line {
                        start: [0.0, 0.0],
                        end: [10.0, 0.0],
                    },
                    HatchEdge::Line {
                        start: [10.0, 0.0],
                        end: [10.0, 10.0],
                    },
                    HatchEdge::Line {
                        start: [10.0, 10.0],
                        end: [0.0, 10.0],
                    },
                    HatchEdge::Line {
                        start: [0.0, 10.0],
                        end: [0.0, 0.0],
                    },
                ],
            }],
        },
    });

    let bytes = write_dwg(&doc).expect("HATCH-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted HATCH DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    assert_eq!(entity.handle, hatch_handle);
    match &entity.data {
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            assert_eq!(pattern_name, "SOLID");
            assert!(*solid_fill);
            assert_eq!(boundary_paths.len(), 1);
            assert_eq!(boundary_paths[0].flags, 0);
            assert_eq!(boundary_paths[0].edges.len(), 4);
            // Check first edge survives byte-equal.
            match &boundary_paths[0].edges[0] {
                HatchEdge::Line { start, end } => {
                    assert_eq!(*start, [0.0, 0.0]);
                    assert_eq!(*end, [10.0, 0.0]);
                }
                other => panic!("expected Line edge, got {other:?}"),
            }
        }
        other => panic!("expected Hatch data, got {other:?}"),
    }
}

#[test]
fn write_dwg_with_single_pattern_hatch_entity_round_trips_through_read_dwg() {
    // F5.M5.E20b tracer bullet: !solid_fill triggers the minimal
    // pattern block on the wire (angle=0, scale=1, num_lines=0).
    // Model-level pattern definition fidelity is a known
    // limitation — only `pattern_name`, `solid_fill`, and
    // `boundary_paths` round-trip.
    use h7cad_native_model::{HatchBoundaryPath, HatchEdge};
    let mut doc = CadDocument::new();
    let zero_layer_handle = Handle::new(0x10);
    doc.layers
        .get_mut("0")
        .expect("CadDocument::new seeds the default `0` layer")
        .handle = zero_layer_handle;

    let hatch_handle = Handle::new(0x111);
    doc.entities.push(Entity {
        handle: hatch_handle,
        owner_handle: Handle::new(0x42),
        layer_name: "0".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: 13,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Hatch {
            pattern_name: "ANSI31".to_string(),
            solid_fill: false,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![HatchEdge::CircularArc {
                    center: [5.0, 5.0],
                    radius: 3.0,
                    start_angle: 0.0,
                    end_angle: std::f64::consts::TAU,
                    is_ccw: true,
                }],
            }],
        },
    });

    let bytes = write_dwg(&doc).expect("pattern HATCH-bearing doc writes successfully");
    let parsed = read_dwg(&bytes).expect("writer-emitted pattern HATCH DWG must read back");
    assert_eq!(parsed.entities.len(), 1);
    let entity = &parsed.entities[0];
    match &entity.data {
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            assert_eq!(pattern_name, "ANSI31");
            assert!(!*solid_fill);
            assert_eq!(boundary_paths.len(), 1);
            match &boundary_paths[0].edges[0] {
                HatchEdge::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    is_ccw,
                } => {
                    assert_eq!(*center, [5.0, 5.0]);
                    assert_eq!(*radius, 3.0);
                    assert_eq!(*start_angle, 0.0);
                    assert_eq!(*end_angle, std::f64::consts::TAU);
                    assert!(*is_ccw);
                }
                other => panic!("expected CircularArc edge, got {other:?}"),
            }
        }
        other => panic!("expected Hatch data, got {other:?}"),
    }
}

#[test]
fn write_dwg_rejects_line_referencing_unknown_layer() {
    // Entity references a layer name not in `doc.layers`. The writer
    // must surface this as `InvalidDocument` rather than panic or
    // silently drop the layer link.
    let mut doc = CadDocument::new();
    doc.entities.push(Entity {
        handle: Handle::new(0x100),
        owner_handle: Handle::NULL,
        layer_name: "missing-layer".to_string(),
        linetype_name: "Continuous".to_string(),
        linetype_scale: 1.0,
        color_index: 7,
        true_color: 0,
        lineweight: -1,
        invisible: false,
        transparency: 0,
        thickness: 0.0,
        extrusion: [0.0, 0.0, 1.0],
        xdata: Vec::new(),
        data: EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        },
    });

    let err = write_dwg(&doc).expect_err("unknown layer must be rejected");
    match err {
        DwgWriteError::InvalidDocument(msg) => assert!(
            msg.contains("missing-layer"),
            "InvalidDocument error must include the layer name; got `{msg}`"
        ),
        other => panic!("expected DwgWriteError::InvalidDocument, got {other:?}"),
    }
}

#[test]
fn write_dwg_directory_offsets_are_within_file_bounds() {
    // Byte-level invariant (plan §2 invariant 3): every section
    // descriptor's `[offset, offset + size)` window must fit inside
    // the file the writer just produced. Without this the reader
    // happily indexes off the end of `bytes` and either panics or
    // returns garbage.

    let doc = CadDocument::new();
    let bytes = write_dwg(&doc).expect("empty doc must write successfully");

    let header = DwgFileHeader::parse(&bytes).expect("file header parses");
    assert_eq!(
        header.section_count, AC1015_KNOWN_SECTION_COUNT,
        "writer must emit exactly {AC1015_KNOWN_SECTION_COUNT} known sections"
    );
    assert_eq!(
        header.section_directory_offset, AC1015_FILE_HEADER_PREFIX_LEN,
        "section directory must start immediately after the {AC1015_FILE_HEADER_PREFIX_LEN}-byte prefix"
    );

    let map = SectionMap::parse(&bytes, &header).expect("section map parses");
    let directory_byte_len =
        (AC1015_KNOWN_SECTION_COUNT as usize) * AC1015_SECTION_LOCATOR_ENTRY_LEN;
    let payload_region_start = AC1015_FILE_HEADER_PREFIX_LEN + directory_byte_len;
    for descriptor in &map.descriptors {
        let start = descriptor.offset as usize;
        let end = start + descriptor.size as usize;
        assert!(
            start >= payload_region_start,
            "descriptor {} offset {start} must land at or after payload region start \
             {payload_region_start}",
            descriptor.index
        );
        assert!(
            end <= bytes.len(),
            "descriptor {} window [{start}, {end}) must fit inside file size {}",
            descriptor.index,
            bytes.len()
        );
    }
}

#[test]
fn write_dwg_section_payloads_are_byte_for_byte_empty() {
    // Surface invariant: at the M2 milestone every section payload
    // is exactly 0 bytes. This test traps any future drift where a
    // composer starts emitting placeholder bytes without explicit
    // intent (e.g. an accidental sentinel rewrite).

    let doc = CadDocument::new();
    let bytes = write_dwg(&doc).expect("empty doc must write successfully");

    let header = DwgFileHeader::parse(&bytes).expect("file header parses");
    let map = SectionMap::parse(&bytes, &header).expect("section map parses");

    // All six descriptors must be empty and arranged in canonical
    // record-number order (Header → Classes → Handles → ObjFreeSpace →
    // Template → AuxHeader).
    let expected_record_numbers: Vec<u8> = vec![
        KnownSection::record_number_from_name("AcDb:Header").unwrap(),
        KnownSection::record_number_from_name("AcDb:Classes").unwrap(),
        KnownSection::record_number_from_name("AcDb:Handles").unwrap(),
        KnownSection::record_number_from_name("AcDb:ObjFreeSpace").unwrap(),
        KnownSection::record_number_from_name("AcDb:Template").unwrap(),
        KnownSection::record_number_from_name("AcDb:AuxHeader").unwrap(),
    ];
    let actual_record_numbers: Vec<u8> = map.descriptors.iter().map(|d| d.record_number).collect();
    assert_eq!(actual_record_numbers, expected_record_numbers);
    for descriptor in &map.descriptors {
        assert_eq!(
            descriptor.size, 0,
            "M2 milestone: section {} (record_number={}) payload must be 0 bytes",
            descriptor.index, descriptor.record_number
        );
    }
}
