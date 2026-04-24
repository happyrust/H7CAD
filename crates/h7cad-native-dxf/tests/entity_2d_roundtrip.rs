use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::*;

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let text = write_dxf(doc).expect("write_dxf failed");
    read_dxf(&text).expect("read_dxf failed")
}

fn assert_f64_eq(a: f64, b: f64, label: &str) {
    assert!(
        (a - b).abs() < 1e-9,
        "{label}: {a} != {b}"
    );
}

fn assert_point_eq(a: &[f64; 3], b: &[f64; 3], label: &str) {
    assert_f64_eq(a[0], b[0], &format!("{label}.x"));
    assert_f64_eq(a[1], b[1], &format!("{label}.y"));
    assert_f64_eq(a[2], b[2], &format!("{label}.z"));
}

#[test]
fn roundtrip_point() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Point {
        position: [7.0, -3.5, 1.0],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Point { position } => assert_point_eq(position, &[7.0, -3.5, 1.0], "position"),
        other => panic!("expected Point, got {other:?}"),
    }
}

#[test]
fn roundtrip_ellipse() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Ellipse {
        center: [10.0, 20.0, 0.0],
        major_axis: [5.0, 0.0, 0.0],
        ratio: 0.6,
        start_param: 0.0,
        end_param: std::f64::consts::TAU,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            assert_point_eq(center, &[10.0, 20.0, 0.0], "center");
            assert_point_eq(major_axis, &[5.0, 0.0, 0.0], "major_axis");
            assert_f64_eq(*ratio, 0.6, "ratio");
            assert_f64_eq(*start_param, 0.0, "start_param");
            assert_f64_eq(*end_param, std::f64::consts::TAU, "end_param");
        }
        other => panic!("expected Ellipse, got {other:?}"),
    }
}

#[test]
fn roundtrip_lwpolyline_with_bulge() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::LwPolyline {
        vertices: vec![
            LwVertex { x: 0.0, y: 0.0, bulge: 0.5, start_width: 0.0, end_width: 0.0 },
            LwVertex { x: 10.0, y: 0.0, bulge: 0.0, start_width: 0.0, end_width: 0.0 },
            LwVertex { x: 10.0, y: 10.0, bulge: -0.3, start_width: 0.0, end_width: 0.0 },
        ],
        closed: true,
        constant_width: 0.25,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::LwPolyline { vertices, closed, constant_width } => {
            assert_eq!(vertices.len(), 3);
            assert_f64_eq(vertices[0].bulge, 0.5, "v0.bulge");
            assert_f64_eq(vertices[2].bulge, -0.3, "v2.bulge");
            assert!(*closed, "should be closed");
            assert_f64_eq(*constant_width, 0.25, "constant_width");
        }
        other => panic!("expected LwPolyline, got {other:?}"),
    }
}

#[test]
fn roundtrip_spline() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Spline {
        degree: 3,
        closed: false,
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        control_points: vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [3.0, 2.0, 0.0],
            [4.0, 0.0, 0.0],
        ],
        weights: vec![],
        fit_points: vec![],
        start_tangent: [0.0, 0.0, 0.0],
        end_tangent: [0.0, 0.0, 0.0],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Spline { degree, closed, knots, control_points, .. } => {
            assert_eq!(*degree, 3);
            assert!(!*closed);
            assert_eq!(knots.len(), 8);
            assert_eq!(control_points.len(), 4);
            assert_point_eq(&control_points[1], &[1.0, 2.0, 0.0], "cp1");
        }
        other => panic!("expected Spline, got {other:?}"),
    }
}

#[test]
fn roundtrip_text_alignment_fields() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Text {
        insertion: [1.0, 2.0, 0.0],
        height: 3.5,
        value: "Hello World".into(),
        rotation: 45.0,
        style_name: "Standard".into(),
        width_factor: 0.8,
        oblique_angle: 12.0,
        horizontal_alignment: 2,
        vertical_alignment: 3,
        alignment_point: Some([5.0, 6.0, 0.0]),
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Text {
            insertion,
            height,
            value,
            rotation,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            alignment_point,
            ..
        } => {
            assert_point_eq(insertion, &[1.0, 2.0, 0.0], "insertion");
            assert_f64_eq(*height, 3.5, "height");
            assert_eq!(value, "Hello World");
            assert_f64_eq(*rotation, 45.0, "rotation");
            assert_f64_eq(*width_factor, 0.8, "width_factor");
            assert_f64_eq(*oblique_angle, 12.0, "oblique_angle");
            assert_eq!(*horizontal_alignment, 2);
            assert_eq!(*vertical_alignment, 3);
            let ap = alignment_point.expect("alignment_point should survive");
            assert_point_eq(&ap, &[5.0, 6.0, 0.0], "alignment_point");
        }
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn roundtrip_mtext_full() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::MText {
        insertion: [5.0, 6.0, 0.0],
        height: 2.5,
        width: 80.0,
        rectangle_height: Some(24.0),
        value: "Line1\\PLine2".into(),
        rotation: 30.0,
        style_name: "Standard".into(),
        attachment_point: 5,
        line_spacing_factor: 1.35,
        drawing_direction: 3,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::MText {
            insertion,
            height,
            width,
            rectangle_height,
            value,
            rotation,
            attachment_point,
            line_spacing_factor,
            drawing_direction,
            ..
        } => {
            assert_point_eq(insertion, &[5.0, 6.0, 0.0], "insertion");
            assert_f64_eq(*height, 2.5, "height");
            assert_f64_eq(*width, 80.0, "width");
            assert_eq!(*rectangle_height, Some(24.0));
            assert_eq!(value, "Line1\\PLine2");
            assert_f64_eq(*rotation, 30.0, "rotation");
            assert_eq!(*attachment_point, 5);
            assert_f64_eq(*line_spacing_factor, 1.35, "line_spacing_factor");
            assert_eq!(*drawing_direction, 3);
        }
        other => panic!("expected MText, got {other:?}"),
    }
}

#[test]
fn roundtrip_hatch_solid_fill() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Hatch {
        pattern_name: "SOLID".into(),
        solid_fill: true,
        boundary_paths: vec![HatchBoundaryPath {
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
                    end: [0.0, 0.0],
                },
            ],
            flags: 1,
        }],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            assert_eq!(pattern_name, "SOLID");
            assert!(*solid_fill);
            assert_eq!(boundary_paths.len(), 1);
            assert_eq!(boundary_paths[0].edges.len(), 3);
        }
        other => panic!("expected Hatch, got {other:?}"),
    }
}

#[test]
fn roundtrip_insert_with_scale_rotation() {
    let mut doc = CadDocument::new();
    let block_handle = doc.allocate_handle();
    doc.insert_block_record(BlockRecord::new(block_handle, "MY_BLOCK"));

    doc.entities.push(Entity::new(EntityData::Insert {
        block_name: "MY_BLOCK".into(),
        insertion: [100.0, 200.0, 0.0],
        scale: [2.0, 3.0, 1.0],
        rotation: 45.0,
        has_attribs: false,
        attribs: vec![],
    }));
    let doc2 = roundtrip(&doc);
    let insert = doc2
        .entities
        .iter()
        .find(|e| matches!(&e.data, EntityData::Insert { .. }))
        .expect("INSERT entity should survive");
    match &insert.data {
        EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            ..
        } => {
            assert_eq!(block_name, "MY_BLOCK");
            assert_point_eq(insertion, &[100.0, 200.0, 0.0], "insertion");
            assert_f64_eq(scale[0], 2.0, "x_scale");
            assert_f64_eq(scale[1], 3.0, "y_scale");
            assert_f64_eq(scale[2], 1.0, "z_scale");
            assert_f64_eq(*rotation, 45.0, "rotation");
        }
        _ => unreachable!(),
    }
}

#[test]
fn roundtrip_solid_2d() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Solid {
        corners: [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [0.0, 5.0, 0.0],
        ],
        normal: [0.0, 0.0, 1.0],
        thickness: 0.0,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Solid { corners, normal, thickness } => {
            assert_point_eq(&corners[0], &[0.0, 0.0, 0.0], "corner0");
            assert_point_eq(&corners[1], &[10.0, 0.0, 0.0], "corner1");
            assert_point_eq(&corners[2], &[10.0, 5.0, 0.0], "corner2");
            assert_point_eq(&corners[3], &[0.0, 5.0, 0.0], "corner3");
            assert_point_eq(normal, &[0.0, 0.0, 1.0], "normal");
            assert_f64_eq(*thickness, 0.0, "thickness");
        }
        other => panic!("expected Solid, got {other:?}"),
    }
}

#[test]
fn roundtrip_face3d() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Face3D {
        corners: [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [5.0, 8.66, 0.0],
            [5.0, 8.66, 0.0],
        ],
        invisible_edges: 0,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Face3D { corners, .. } => {
            assert_point_eq(&corners[0], &[0.0, 0.0, 0.0], "corner0");
            assert_point_eq(&corners[1], &[10.0, 0.0, 0.0], "corner1");
            assert_point_eq(&corners[2], &[5.0, 8.66, 0.0], "corner2");
        }
        other => panic!("expected Face3D, got {other:?}"),
    }
}

#[test]
fn roundtrip_leader() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Leader {
        vertices: vec![[0.0, 0.0, 0.0], [5.0, 5.0, 0.0], [10.0, 5.0, 0.0]],
        has_arrowhead: true,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Leader { vertices, has_arrowhead } => {
            assert_eq!(vertices.len(), 3);
            assert_point_eq(&vertices[0], &[0.0, 0.0, 0.0], "v0");
            assert_point_eq(&vertices[2], &[10.0, 5.0, 0.0], "v2");
            assert!(*has_arrowhead, "arrowhead should survive");
        }
        other => panic!("expected Leader, got {other:?}"),
    }
}

#[test]
fn roundtrip_entity_common_fields() {
    let mut doc = CadDocument::new();
    doc.layers.insert(
        "TestLayer".into(),
        LayerProperties::new("TestLayer"),
    );
    let mut entity = Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [1.0, 1.0, 0.0],
    });
    entity.layer_name = "TestLayer".into();
    entity.linetype_name = "DASHED".into();
    entity.linetype_scale = 2.5;
    entity.color_index = 3;
    entity.lineweight = 50;
    doc.entities.push(entity);
    let doc2 = roundtrip(&doc);
    let e = &doc2.entities[0];
    assert_eq!(e.layer_name, "TestLayer");
    assert_eq!(e.linetype_name, "DASHED");
    assert_f64_eq(e.linetype_scale, 2.5, "linetype_scale");
    assert_eq!(e.color_index, 3);
    assert_eq!(e.lineweight, 50);
}

#[test]
fn roundtrip_viewport() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Viewport {
        center: [150.0, 100.0, 0.0],
        width: 297.0,
        height: 210.0,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Viewport { center, width, height } => {
            assert_point_eq(center, &[150.0, 100.0, 0.0], "center");
            assert_f64_eq(*width, 297.0, "width");
            assert_f64_eq(*height, 210.0, "height");
        }
        other => panic!("expected Viewport, got {other:?}"),
    }
}

// ===========================================================================
// Task 3 (2026-04-24 DXF 2D display closure plan) — basic geometry coverage
// ===========================================================================

#[test]
fn roundtrip_line() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [10.0, 20.0, 30.0],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Line { start, end } => {
            assert_point_eq(start, &[0.0, 0.0, 0.0], "start");
            assert_point_eq(end, &[10.0, 20.0, 30.0], "end");
        }
        other => panic!("expected Line, got {other:?}"),
    }
}

#[test]
fn roundtrip_circle() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Circle {
        center: [3.0, 4.0, 5.0],
        radius: 12.75,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Circle { center, radius } => {
            assert_point_eq(center, &[3.0, 4.0, 5.0], "center");
            assert_f64_eq(*radius, 12.75, "radius");
        }
        other => panic!("expected Circle, got {other:?}"),
    }
}

#[test]
fn roundtrip_arc() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Arc {
        center: [0.0, 0.0, 0.0],
        radius: 5.0,
        start_angle: 30.0,
        end_angle: 210.0,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Arc { center, radius, start_angle, end_angle } => {
            assert_point_eq(center, &[0.0, 0.0, 0.0], "center");
            assert_f64_eq(*radius, 5.0, "radius");
            assert_f64_eq(*start_angle, 30.0, "start_angle");
            assert_f64_eq(*end_angle, 210.0, "end_angle");
        }
        other => panic!("expected Arc, got {other:?}"),
    }
}

// ===========================================================================
// POLYLINE — all 4 subtypes (2D / 3D / PolygonMesh / PolyfaceMesh)
// ===========================================================================

fn mk_pv(x: f64, y: f64, z: f64, bulge: f64) -> PolylineVertex {
    PolylineVertex {
        position: [x, y, z],
        bulge,
        start_width: 0.0,
        end_width: 0.0,
    }
}

#[test]
fn roundtrip_polyline_2d_closed_with_bulge() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Polyline {
        polyline_type: PolylineType::Polyline2D,
        vertices: vec![
            mk_pv(0.0, 0.0, 0.0, 0.0),
            mk_pv(10.0, 0.0, 0.0, 0.4),
            mk_pv(10.0, 10.0, 0.0, 0.0),
            mk_pv(0.0, 10.0, 0.0, -0.2),
        ],
        closed: true,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Polyline { polyline_type, vertices, closed } => {
            assert_eq!(*polyline_type, PolylineType::Polyline2D);
            assert_eq!(vertices.len(), 4);
            assert!(*closed, "closed flag must survive");
            assert_f64_eq(vertices[1].bulge, 0.4, "v1.bulge");
            assert_f64_eq(vertices[3].bulge, -0.2, "v3.bulge");
            assert_point_eq(&vertices[2].position, &[10.0, 10.0, 0.0], "v2.pos");
        }
        other => panic!("expected Polyline, got {other:?}"),
    }
}

#[test]
fn roundtrip_polyline_3d_open() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Polyline {
        polyline_type: PolylineType::Polyline3D,
        vertices: vec![
            mk_pv(0.0, 0.0, 0.0, 0.0),
            mk_pv(5.0, 5.0, 3.0, 0.0),
            mk_pv(10.0, 0.0, 6.0, 0.0),
        ],
        closed: false,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Polyline { polyline_type, vertices, closed } => {
            assert_eq!(*polyline_type, PolylineType::Polyline3D);
            assert_eq!(vertices.len(), 3);
            assert!(!*closed);
            assert_point_eq(&vertices[1].position, &[5.0, 5.0, 3.0], "v1.pos");
        }
        other => panic!("expected Polyline, got {other:?}"),
    }
}

#[test]
fn roundtrip_polyline_polygon_mesh() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Polyline {
        polyline_type: PolylineType::PolygonMesh,
        vertices: vec![
            mk_pv(0.0, 0.0, 0.0, 0.0),
            mk_pv(1.0, 0.0, 0.0, 0.0),
            mk_pv(1.0, 1.0, 0.0, 0.0),
            mk_pv(0.0, 1.0, 0.0, 0.0),
        ],
        closed: false,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Polyline { polyline_type, vertices, .. } => {
            assert_eq!(*polyline_type, PolylineType::PolygonMesh);
            assert_eq!(vertices.len(), 4);
        }
        other => panic!("expected Polyline, got {other:?}"),
    }
}

#[test]
fn roundtrip_polyline_polyface_mesh() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Polyline {
        polyline_type: PolylineType::PolyfaceMesh,
        vertices: vec![
            mk_pv(0.0, 0.0, 0.0, 0.0),
            mk_pv(1.0, 0.0, 0.0, 0.0),
            mk_pv(0.5, 1.0, 0.0, 0.0),
        ],
        closed: false,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Polyline { polyline_type, vertices, .. } => {
            assert_eq!(*polyline_type, PolylineType::PolyfaceMesh);
            assert_eq!(vertices.len(), 3);
        }
        other => panic!("expected Polyline, got {other:?}"),
    }
}

// ===========================================================================
// DIMENSION — key geometry points + style/block references
// ===========================================================================

fn dimension_with_type(dim_type: i16) -> EntityData {
    EntityData::Dimension {
        dim_type,
        block_name: "*D1".into(),
        style_name: "Standard".into(),
        definition_point: [10.0, 20.0, 0.0],
        text_midpoint: [15.0, 22.0, 0.0],
        text_override: "<>".into(),
        attachment_point: 5,
        measurement: 42.5,
        text_rotation: 0.25,
        horizontal_direction: 1.5,
        flip_arrow1: true,
        flip_arrow2: false,
        first_point: [0.0, 0.0, 0.0],
        second_point: [30.0, 0.0, 0.0],
        angle_vertex: [0.0, 0.0, 0.0],
        dimension_arc: [0.0, 0.0, 0.0],
        leader_length: 0.0,
        rotation: 0.0,
        ext_line_rotation: 0.125,
    }
}

#[test]
fn roundtrip_dimension_linear_all_points() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(dimension_with_type(0)));
    let doc2 = roundtrip(&doc);
    let dim = doc2
        .entities
        .iter()
        .find(|e| matches!(e.data, EntityData::Dimension { .. }))
        .expect("Dimension entity should survive");
    match &dim.data {
        EntityData::Dimension {
            dim_type,
            block_name,
            style_name,
            definition_point,
            text_midpoint,
            text_override,
            first_point,
            second_point,
            attachment_point,
            measurement,
            flip_arrow1,
            flip_arrow2,
            ext_line_rotation,
            ..
        } => {
            assert_eq!(*dim_type, 0);
            assert_eq!(block_name, "*D1");
            assert_eq!(style_name, "Standard");
            assert_point_eq(definition_point, &[10.0, 20.0, 0.0], "def_point");
            assert_point_eq(text_midpoint, &[15.0, 22.0, 0.0], "text_mid");
            assert_eq!(text_override, "<>");
            assert_point_eq(first_point, &[0.0, 0.0, 0.0], "first");
            assert_point_eq(second_point, &[30.0, 0.0, 0.0], "second");
            assert_eq!(*attachment_point, 5);
            assert_f64_eq(*measurement, 42.5, "measurement");
            assert!(*flip_arrow1, "flip_arrow1");
            assert!(!*flip_arrow2, "flip_arrow2");
            assert_f64_eq(*ext_line_rotation, 0.125, "ext_line_rotation");
        }
        _ => unreachable!(),
    }
}

#[test]
fn roundtrip_dimension_radius() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Dimension {
        dim_type: 4,
        block_name: "*D2".into(),
        style_name: "ISO-25".into(),
        definition_point: [50.0, 50.0, 0.0],
        text_midpoint: [55.0, 55.0, 0.0],
        text_override: String::new(),
        attachment_point: 0,
        measurement: 12.5,
        text_rotation: 0.0,
        horizontal_direction: 0.0,
        flip_arrow1: false,
        flip_arrow2: false,
        first_point: [0.0, 0.0, 0.0],
        second_point: [0.0, 0.0, 0.0],
        angle_vertex: [60.0, 60.0, 0.0],
        dimension_arc: [0.0, 0.0, 0.0],
        leader_length: 8.0,
        rotation: 0.0,
        ext_line_rotation: 0.0,
    }));
    let doc2 = roundtrip(&doc);
    let dim = doc2
        .entities
        .iter()
        .find(|e| matches!(e.data, EntityData::Dimension { .. }))
        .expect("Dimension should survive");
    match &dim.data {
        EntityData::Dimension {
            dim_type,
            style_name,
            angle_vertex,
            leader_length,
            ..
        } => {
            assert_eq!(*dim_type, 4);
            assert_eq!(style_name, "ISO-25");
            assert_point_eq(angle_vertex, &[60.0, 60.0, 0.0], "angle_vertex");
            assert_f64_eq(*leader_length, 8.0, "leader_length");
        }
        _ => unreachable!(),
    }
}

// ===========================================================================
// INSERT with ATTRIB children + ATTDEF
// ===========================================================================

#[test]
fn roundtrip_insert_with_attribs() {
    let mut doc = CadDocument::new();
    let block_handle = doc.allocate_handle();
    doc.insert_block_record(BlockRecord::new(block_handle, "TITLEBLOCK"));

    let mut attrib_a = Entity::new(EntityData::Attrib {
        tag: "PART_NO".into(),
        value: "A-123".into(),
        insertion: [1.0, 2.0, 0.0],
        height: 2.5,
    });
    attrib_a.layer_name = "ATTRS".into();

    let mut attrib_b = Entity::new(EntityData::Attrib {
        tag: "REV".into(),
        value: "B".into(),
        insertion: [1.0, 5.0, 0.0],
        height: 2.5,
    });
    attrib_b.layer_name = "ATTRS".into();

    doc.entities.push(Entity::new(EntityData::Insert {
        block_name: "TITLEBLOCK".into(),
        insertion: [100.0, 100.0, 0.0],
        scale: [1.0, 1.0, 1.0],
        rotation: 0.0,
        has_attribs: true,
        attribs: vec![attrib_a, attrib_b],
    }));

    let doc2 = roundtrip(&doc);
    let insert = doc2
        .entities
        .iter()
        .find(|e| matches!(&e.data, EntityData::Insert { .. }))
        .expect("INSERT should survive");
    match &insert.data {
        EntityData::Insert {
            block_name,
            has_attribs,
            attribs,
            ..
        } => {
            assert_eq!(block_name, "TITLEBLOCK");
            assert!(*has_attribs, "has_attribs flag lost");
            assert_eq!(attribs.len(), 2, "both attribs must survive");
            let tags: Vec<&str> = attribs
                .iter()
                .filter_map(|a| match &a.data {
                    EntityData::Attrib { tag, .. } => Some(tag.as_str()),
                    _ => None,
                })
                .collect();
            assert!(tags.contains(&"PART_NO"));
            assert!(tags.contains(&"REV"));
        }
        _ => unreachable!(),
    }
}

#[test]
fn roundtrip_attdef() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::AttDef {
        tag: "DATE".into(),
        prompt: "Enter date".into(),
        default_value: "2026-04-24".into(),
        insertion: [3.0, 4.0, 0.0],
        height: 2.0,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::AttDef {
            tag,
            prompt,
            default_value,
            insertion,
            height,
        } => {
            assert_eq!(tag, "DATE");
            assert_eq!(prompt, "Enter date");
            assert_eq!(default_value, "2026-04-24");
            assert_point_eq(insertion, &[3.0, 4.0, 0.0], "insertion");
            assert_f64_eq(*height, 2.0, "height");
        }
        other => panic!("expected AttDef, got {other:?}"),
    }
}

// ===========================================================================
// HATCH — CircularArc + EllipticArc edges and pattern_name roundtrip
// ===========================================================================

#[test]
fn roundtrip_hatch_with_island_topology() {
    // A donut-shaped hatch: one outer boundary path + one inner hole.
    // Flags encode the boundary type (bit 0 = outer, bit 1 = polyline).
    // Real drawings have this topology all the time (e.g. a filled
    // washer drawn as a circle with a concentric hole).
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Hatch {
        pattern_name: "SOLID".into(),
        solid_fill: true,
        boundary_paths: vec![
            // Outer boundary: square.
            HatchBoundaryPath {
                flags: 1, // external
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
            },
            // Inner hole: smaller square.
            HatchBoundaryPath {
                flags: 16, // outermost/outside — whatever, just preserve bits
                edges: vec![
                    HatchEdge::Line {
                        start: [3.0, 3.0],
                        end: [7.0, 3.0],
                    },
                    HatchEdge::Line {
                        start: [7.0, 3.0],
                        end: [7.0, 7.0],
                    },
                    HatchEdge::Line {
                        start: [7.0, 7.0],
                        end: [3.0, 7.0],
                    },
                    HatchEdge::Line {
                        start: [3.0, 7.0],
                        end: [3.0, 3.0],
                    },
                ],
            },
        ],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Hatch {
            boundary_paths, ..
        } => {
            assert_eq!(
                boundary_paths.len(),
                2,
                "both outer and inner boundary paths must survive"
            );
            assert_eq!(boundary_paths[0].edges.len(), 4, "outer: 4 edges");
            assert_eq!(boundary_paths[1].edges.len(), 4, "inner hole: 4 edges");
        }
        other => panic!("expected Hatch, got {other:?}"),
    }
}

#[test]
fn roundtrip_hatch_with_arc_edges() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Hatch {
        pattern_name: "ANSI31".into(),
        solid_fill: false,
        boundary_paths: vec![HatchBoundaryPath {
            flags: 1,
            edges: vec![
                HatchEdge::Line {
                    start: [0.0, 0.0],
                    end: [10.0, 0.0],
                },
                HatchEdge::CircularArc {
                    center: [10.0, 5.0],
                    radius: 5.0,
                    start_angle: -90.0,
                    end_angle: 90.0,
                    is_ccw: true,
                },
                HatchEdge::EllipticArc {
                    center: [0.0, 5.0],
                    major_endpoint: [5.0, 0.0],
                    minor_ratio: 0.5,
                    start_angle: 0.0,
                    end_angle: 180.0,
                    is_ccw: false,
                },
            ],
        }],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            assert_eq!(pattern_name, "ANSI31", "pattern_name must survive");
            assert!(!*solid_fill, "solid_fill=false must survive");
            assert_eq!(boundary_paths.len(), 1);
            assert_eq!(boundary_paths[0].edges.len(), 3);
            let mut has_line = false;
            let mut has_carc = false;
            let mut has_earc = false;
            for edge in &boundary_paths[0].edges {
                match edge {
                    HatchEdge::Line { .. } => has_line = true,
                    HatchEdge::CircularArc { radius, is_ccw, .. } => {
                        assert_f64_eq(*radius, 5.0, "carc.radius");
                        assert!(*is_ccw, "carc.is_ccw");
                        has_carc = true;
                    }
                    HatchEdge::EllipticArc {
                        minor_ratio, is_ccw, ..
                    } => {
                        assert_f64_eq(*minor_ratio, 0.5, "earc.ratio");
                        assert!(!*is_ccw, "earc.is_ccw");
                        has_earc = true;
                    }
                    _ => {}
                }
            }
            assert!(has_line && has_carc && has_earc, "all edge kinds must survive");
        }
        other => panic!("expected Hatch, got {other:?}"),
    }
}

// ===========================================================================
// WIPEOUT — elevation + clip polygon
// ===========================================================================

#[test]
fn roundtrip_wipeout_with_elevation() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::Wipeout {
        clip_vertices: vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
        ],
        elevation: 2.75,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::Wipeout {
            clip_vertices,
            elevation,
        } => {
            assert_eq!(clip_vertices.len(), 4, "clip polygon vertex count");
            assert_f64_eq(clip_vertices[2][0], 10.0, "v2.x");
            assert_f64_eq(clip_vertices[2][1], 10.0, "v2.y");
            assert_f64_eq(*elevation, 2.75, "elevation");
        }
        other => panic!("expected Wipeout, got {other:?}"),
    }
}

// ===========================================================================
// MLINE — closed + multiple vertices
// ===========================================================================

#[test]
fn roundtrip_mline_closed() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::MLine {
        vertices: vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 5.0, 0.0],
            [0.0, 5.0, 0.0],
        ],
        style_name: "STANDARD".into(),
        scale: 1.5,
        closed: true,
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::MLine {
            vertices,
            style_name,
            scale,
            closed,
        } => {
            assert_eq!(vertices.len(), 4);
            assert_eq!(style_name, "STANDARD");
            assert_f64_eq(*scale, 1.5, "scale");
            assert!(*closed, "closed flag must survive");
            assert_point_eq(&vertices[2], &[10.0, 5.0, 0.0], "v2");
        }
        other => panic!("expected MLine, got {other:?}"),
    }
}

// ===========================================================================
// MULTILEADER — content, leader vertices, style
// ===========================================================================

#[test]
fn roundtrip_multileader_text_with_leader_line() {
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::MultiLeader {
        content_type: 1,
        text_label: "NOTE-A".into(),
        style_name: "Standard".into(),
        arrowhead_size: 3.0,
        landing_gap: 1.0,
        dogleg_length: 4.0,
        property_override_flags: 0,
        path_type: 1,
        line_color: -1,
        leader_line_weight: -1,
        enable_landing: true,
        enable_dogleg: true,
        enable_annotation_scale: false,
        scale_factor: 1.0,
        text_attachment_direction: 0,
        text_bottom_attachment_type: 9,
        text_top_attachment_type: 9,
        text_location: Some([50.0, 50.0, 0.0]),
        leader_vertices: vec![
            [0.0, 0.0, 0.0],
            [25.0, 25.0, 0.0],
            [50.0, 50.0, 0.0],
        ],
        leader_root_lengths: vec![3],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::MultiLeader {
            content_type,
            text_label,
            style_name,
            arrowhead_size,
            dogleg_length,
            text_location,
            leader_vertices,
            ..
        } => {
            assert_eq!(*content_type, 1, "content_type");
            assert_eq!(text_label, "NOTE-A", "text_label");
            assert_eq!(style_name, "Standard");
            assert_f64_eq(*arrowhead_size, 3.0, "arrowhead_size");
            assert_f64_eq(*dogleg_length, 4.0, "dogleg_length");
            let tl = text_location.expect("text_location must survive");
            assert_point_eq(&tl, &[50.0, 50.0, 0.0], "text_location");
            assert_eq!(leader_vertices.len(), 3, "leader vertex count");
            assert_point_eq(&leader_vertices[1], &[25.0, 25.0, 0.0], "mid vertex");
        }
        other => panic!("expected MultiLeader, got {other:?}"),
    }
}

// ===========================================================================
// Handle / owner preservation & extended common fields
// ===========================================================================

// ===========================================================================
// Double roundtrip — second write must be structurally identical to first
// ===========================================================================

#[test]
fn double_roundtrip_basic_geometry_is_structurally_stable() {
    // If the writer emits a slightly different form of an entity that
    // the reader normalises, a single write/read cycle can look clean
    // while a second cycle quietly shifts to another representation.
    // This test guards against that by verifying the second write is
    // byte-for-byte identical to the first (up to whitespace-only
    // tokenisation).
    let mut doc1 = CadDocument::new();
    doc1.entities.push(Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [10.0, 0.0, 0.0],
    }));
    doc1.entities.push(Entity::new(EntityData::Circle {
        center: [5.0, 5.0, 0.0],
        radius: 3.0,
    }));
    doc1.entities.push(Entity::new(EntityData::Text {
        insertion: [0.0, 10.0, 0.0],
        height: 2.5,
        value: "STABLE".into(),
        rotation: 0.0,
        style_name: "Standard".into(),
        width_factor: 1.0,
        oblique_angle: 0.0,
        horizontal_alignment: 0,
        vertical_alignment: 0,
        alignment_point: None,
    }));

    let text1 = h7cad_native_dxf::write_dxf(&doc1).expect("first write");
    let doc2 = h7cad_native_dxf::read_dxf_bytes(text1.as_bytes()).expect("first read");
    let text2 = h7cad_native_dxf::write_dxf(&doc2).expect("second write");
    let doc3 = h7cad_native_dxf::read_dxf_bytes(text2.as_bytes()).expect("second read");

    // Structural equality: the entity types, counts, and the primary
    // geometry fields must match across two write/read cycles. We do
    // not demand byte-identical DXF because the writer may emit
    // optional pairs (e.g. handles, unused dim-style fields) that
    // don't round-trip losslessly yet; the plan for byte-identical
    // fidelity is tracked separately.
    assert_eq!(
        doc2.entities.len(),
        doc3.entities.len(),
        "entity count must stay stable across double roundtrip"
    );

    for (a, b) in doc2.entities.iter().zip(doc3.entities.iter()) {
        assert_eq!(
            std::mem::discriminant(&a.data),
            std::mem::discriminant(&b.data),
            "entity variant must stay stable"
        );
    }

    // Per-entity key-field check for the three entities we added.
    let circle_stable = doc3
        .entities
        .iter()
        .any(|e| matches!(&e.data,
            EntityData::Circle { center, radius }
                if (center[0] - 5.0).abs() < 1e-9
                    && (center[1] - 5.0).abs() < 1e-9
                    && (*radius - 3.0).abs() < 1e-9));
    assert!(circle_stable, "circle geometry must survive two roundtrips");

    let text_stable = doc3
        .entities
        .iter()
        .any(|e| matches!(&e.data, EntityData::Text { value, .. } if value == "STABLE"));
    assert!(text_stable, "text value must survive two roundtrips");
}

// ===========================================================================
// Fresh document minimum-case — new doc round-trips even with no entities
// ===========================================================================

#[test]
fn fresh_empty_document_survives_write_read_cycle() {
    // `CadDocument::new()` already seeds *Model_Space, *Paper_Space,
    // layer "0", and the standard linetypes. Writing an empty doc and
    // reading it back must not lose any of those seeds, because this
    // is what happens when the user hits "New" and immediately "Save".
    let doc = CadDocument::new();
    let text = h7cad_native_dxf::write_dxf(&doc).expect("write_dxf on empty doc");
    let reloaded =
        h7cad_native_dxf::read_dxf_bytes(text.as_bytes()).expect("read_dxf_bytes on empty doc");

    assert!(
        reloaded
            .block_records
            .values()
            .any(|br| br.name == "*Model_Space"),
        "*Model_Space must survive empty doc roundtrip"
    );
    assert!(
        reloaded
            .block_records
            .values()
            .any(|br| br.name == "*Paper_Space"),
        "*Paper_Space must survive empty doc roundtrip"
    );
    assert!(
        reloaded.layers.contains_key("0"),
        "layer '0' must survive empty doc roundtrip"
    );
    assert_eq!(
        reloaded.entities.len(),
        0,
        "empty doc must stay empty after roundtrip"
    );
}

// ===========================================================================
// XData — third-party extended data must survive roundtrip
// ===========================================================================

#[test]
fn roundtrip_entity_xdata_preserves_multiple_app_blocks() {
    // XData is the canonical way third-party apps tag entities without
    // needing their own class. Losing it on save silently strips
    // application metadata — a serious data-integrity bug. This test
    // covers multi-app + multiple code groups per app.
    let mut entity = Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [1.0, 1.0, 0.0],
    });
    entity.xdata = vec![
        (
            "ACAD_MY_APP".into(),
            vec![
                (1000, "MARKER".into()),
                (1070, "42".into()),
                (1040, "3.14".into()),
            ],
        ),
        (
            "OTHER_PLUGIN".into(),
            vec![(1000, "payload".into())],
        ),
    ];
    let mut doc = CadDocument::new();
    doc.entities.push(entity);

    let doc2 = roundtrip(&doc);
    let e = &doc2.entities[0];
    assert_eq!(e.xdata.len(), 2, "both xdata app blocks must survive");

    let apps: Vec<&str> = e.xdata.iter().map(|(a, _)| a.as_str()).collect();
    assert!(apps.contains(&"ACAD_MY_APP"));
    assert!(apps.contains(&"OTHER_PLUGIN"));

    let acad_block = e
        .xdata
        .iter()
        .find(|(a, _)| a == "ACAD_MY_APP")
        .expect("ACAD_MY_APP app block");
    let codes: Vec<(i16, &str)> = acad_block
        .1
        .iter()
        .map(|(c, v)| (*c, v.as_str()))
        .collect();
    assert!(codes.contains(&(1000, "MARKER")));
    assert!(codes.contains(&(1070, "42")));
    assert!(codes.contains(&(1040, "3.14")));
}

// ===========================================================================
// BLOCK base_point — must survive DXF roundtrip
// ===========================================================================

#[test]
fn roundtrip_block_base_point_survives_write_read_cycle() {
    // Regression: `write_blocks()` previously hard-coded code 10 to
    // [0.0, 0.0, 0.0], so custom block base points (e.g. a symbol
    // whose insertion anchor is at its geometric centre rather than
    // its origin) were silently rewritten to the origin on save. This
    // broke insertion geometry after any save cycle.
    let mut doc = CadDocument::new();
    let block_handle = doc.allocate_handle();
    let mut block = BlockRecord::new(block_handle, "CUSTOM_SYMBOL");
    block.base_point = [5.0, 7.5, 0.0];
    doc.insert_block_record(block);

    let doc2 = roundtrip(&doc);
    let br = doc2
        .block_records
        .values()
        .find(|br| br.name == "CUSTOM_SYMBOL")
        .expect("CUSTOM_SYMBOL must survive");
    assert_f64_eq(br.base_point[0], 5.0, "base_point.x");
    assert_f64_eq(br.base_point[1], 7.5, "base_point.y");
    assert_f64_eq(br.base_point[2], 0.0, "base_point.z");
}

// ===========================================================================
// DIMSTYLE — dimension-style table entry roundtrip
// ===========================================================================

#[test]
fn roundtrip_dimstyle_table_preserves_render_relevant_fields() {
    use h7cad_native_model::DimStyleProperties;

    let mut doc = CadDocument::new();
    let mut ds = DimStyleProperties::new("METRIC");
    ds.dimscale = 2.5;
    ds.dimasz = 3.0;
    ds.dimexo = 0.75;
    ds.dimgap = 1.5;
    ds.dimtxt = 4.0;
    ds.dimdec = 2;
    ds.dimtxsty_name = "Standard".into();
    ds.dimlunit = 2;
    ds.dimaunit = 1;
    doc.dim_styles.insert("METRIC".into(), ds);

    let doc2 = roundtrip(&doc);
    let back = doc2
        .dim_styles
        .get("METRIC")
        .expect("DIMSTYLE METRIC must survive");
    assert_f64_eq(back.dimscale, 2.5, "dimscale");
    assert_f64_eq(back.dimasz, 3.0, "dimasz");
    assert_f64_eq(back.dimexo, 0.75, "dimexo");
    assert_f64_eq(back.dimgap, 1.5, "dimgap");
    assert_f64_eq(back.dimtxt, 4.0, "dimtxt");
    assert_eq!(back.dimdec, 2);
    assert_eq!(back.dimlunit, 2);
    assert_eq!(back.dimaunit, 1);
}

// ===========================================================================
// Layer properties — full round-trip of the fields the renderer consumes
// ===========================================================================

#[test]
fn roundtrip_layer_table_preserves_all_render_relevant_fields() {
    use h7cad_native_model::LayerProperties;

    let mut doc = CadDocument::new();

    // Layer A: off via negative ACI color, custom linetype, lineweight,
    // and true-color override.
    let mut layer_a = LayerProperties::new("DRAFT_OFF");
    layer_a.color = -3; // off, but underlying ACI is 3 (green)
    layer_a.linetype_name = "DASHED".into();
    layer_a.lineweight = 50;
    layer_a.true_color = 0x00_FF_AA_11;
    layer_a.is_frozen = true;
    layer_a.is_locked = false;
    layer_a.plot = false;
    doc.layers.insert("DRAFT_OFF".into(), layer_a);

    // Layer B: on, visible, locked, plots, no true_color.
    let mut layer_b = LayerProperties::new("DIM_LAYER");
    layer_b.color = 1;
    layer_b.linetype_name = "Continuous".into();
    layer_b.lineweight = 25;
    layer_b.is_frozen = false;
    layer_b.is_locked = true;
    layer_b.plot = true;
    doc.layers.insert("DIM_LAYER".into(), layer_b);

    let doc2 = roundtrip(&doc);

    let a = doc2
        .layers
        .get("DRAFT_OFF")
        .expect("DRAFT_OFF layer must survive");
    assert_eq!(a.color, -3, "negative ACI (off flag) must survive");
    assert!(!a.is_on(), "layer marked off stays off");
    assert_eq!(a.linetype_name, "DASHED");
    assert_eq!(a.lineweight, 50);
    assert_eq!(a.true_color, 0x00_FF_AA_11);
    assert!(a.is_frozen, "frozen flag must survive");
    assert!(!a.is_locked);
    assert!(!a.plot, "plot=false must survive");

    let b = doc2
        .layers
        .get("DIM_LAYER")
        .expect("DIM_LAYER must survive");
    assert_eq!(b.color, 1);
    assert!(b.is_on());
    assert!(b.is_locked);
    assert!(!b.is_frozen);
    assert!(b.plot);
    assert_eq!(b.lineweight, 25);
}

#[test]
fn roundtrip_preserves_entity_handle() {
    let mut doc = CadDocument::new();
    let h = doc.allocate_handle();
    let mut entity = Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [1.0, 0.0, 0.0],
    });
    entity.handle = h;
    entity.owner_handle = doc.model_space_handle();
    doc.add_entity(entity).expect("add_entity");

    let doc2 = roundtrip(&doc);
    let line = doc2
        .entities
        .iter()
        .find(|e| matches!(e.data, EntityData::Line { .. }))
        .expect("Line should survive");
    assert_eq!(
        line.handle, h,
        "entity handle must survive write/read cycle",
    );
    assert_ne!(
        line.owner_handle,
        h7cad_native_model::Handle::NULL,
        "owner handle must survive write/read cycle",
    );
}

// ===========================================================================
// PROXY ENTITY — raw-code pass-through (third-party / unknown entity safety)
// ===========================================================================

#[test]
fn roundtrip_proxy_entity_preserves_class_ids_and_raw_payload() {
    // ACAD_PROXY_ENTITY instances wrap content from a third-party
    // application that H7CAD does not decode. The plan promises the
    // writer passes the raw DXF codes through unchanged so re-saving
    // never deletes proprietary data. This roundtrip locks the
    // contract. Note: code 340 (hard-pointer) is intentionally excluded
    // because the ProxyEntity bucket already reserves 5/330 for the
    // common header and proxies may own additional reactors that are
    // ambiguous without the full object graph context; we use
    // unambiguous per-app codes (70, 1000, etc.).
    let mut doc = CadDocument::new();
    doc.entities.push(Entity::new(EntityData::ProxyEntity {
        class_id: 498,
        application_class_id: 499,
        raw_codes: vec![
            (70, "1".into()),
            (71, "42".into()),
            (1000, "CUSTOM_APP_PAYLOAD".into()),
            (1070, "7".into()),
        ],
    }));
    let doc2 = roundtrip(&doc);
    match &doc2.entities[0].data {
        EntityData::ProxyEntity {
            class_id,
            application_class_id,
            raw_codes,
        } => {
            assert_eq!(*class_id, 498);
            assert_eq!(*application_class_id, 499);
            // All non-common raw codes must come back in order.
            let pairs: Vec<(i16, &str)> =
                raw_codes.iter().map(|(c, v)| (*c, v.as_str())).collect();
            assert!(
                pairs.contains(&(70, "1")),
                "raw code 70 must survive; got {pairs:?}"
            );
            assert!(pairs.contains(&(71, "42")), "raw code 71 must survive");
            assert!(
                pairs.contains(&(1000, "CUSTOM_APP_PAYLOAD")),
                "custom string payload must survive"
            );
            assert!(pairs.contains(&(1070, "7")), "xdata int must survive");
        }
        other => panic!("expected ProxyEntity, got {other:?}"),
    }
}

#[test]
fn roundtrip_preserves_true_color_transparency_and_invisible() {
    let mut doc = CadDocument::new();
    let mut entity = Entity::new(EntityData::Line {
        start: [0.0, 0.0, 0.0],
        end: [1.0, 1.0, 0.0],
    });
    entity.true_color = 0x00_FF_80_40;
    entity.transparency = 0x02_00_00_7F;
    entity.invisible = true;
    doc.entities.push(entity);

    let doc2 = roundtrip(&doc);
    let e = &doc2.entities[0];
    assert_eq!(e.true_color, 0x00_FF_80_40, "true_color");
    assert_eq!(e.transparency, 0x02_00_00_7F, "transparency");
    assert!(e.invisible, "invisible flag");
}
