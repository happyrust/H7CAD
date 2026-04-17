//! Bridge between h7cad-native-model and acadrust type systems.

use acadrust::entities as ar;
use crate::types::{Color, Handle, LineWeight, Vector2, Vector3};
use h7cad_native_model as nm;

fn normalize_face3d_invisible_edges(bits: u8) -> i16 {
    i16::from(bits & 0x0F)
}

fn infer_polygon_mesh_dimensions(vertex_count: usize) -> (i16, i16) {
    if vertex_count == 0 {
        return (0, 0);
    }
    let target = (vertex_count as f64).sqrt().floor() as usize;
    for m in (1..=target.max(1)).rev() {
        if vertex_count.is_multiple_of(m) {
            return (m as i16, (vertex_count / m) as i16);
        }
    }
    (1, vertex_count as i16)
}

pub fn native_doc_to_acadrust(native: &nm::CadDocument) -> acadrust::CadDocument {
    let mut doc = acadrust::CadDocument::new();

    for entity in &native.entities {
        if let Some(ar_entity) = native_entity_to_acadrust(entity) {
            let _ = doc.add_entity(ar_entity);
        }
    }

    for (name, layer) in &native.layers {
        let mut ar_layer = acadrust::tables::Layer::new(name.clone());
        ar_layer.color = if layer.true_color != 0 {
            Color::from_rgb(
                ((layer.true_color >> 16) & 0xFF) as u8,
                ((layer.true_color >> 8) & 0xFF) as u8,
                (layer.true_color & 0xFF) as u8,
            )
        } else {
            Color::from_index(layer.color.abs())
        };
        ar_layer.line_weight = native_lineweight(layer.lineweight);
        ar_layer.line_type = layer.linetype_name.clone();
        ar_layer.flags.frozen = layer.is_frozen;
        ar_layer.flags.locked = layer.is_locked;
        ar_layer.flags.off = layer.color < 0;
        let _ = doc.layers.add_or_replace(ar_layer);
    }

    doc
}

pub fn acadrust_doc_to_native(doc: &acadrust::CadDocument) -> nm::CadDocument {
    let mut native = nm::CadDocument::new();
    native.layers.clear();
    native.tables.layer.entries.clear();

    for layer in doc.layers.iter() {
        let (color_index, true_color) = color_to_native(&layer.color);
        let handle = nm::Handle::new(layer.handle.value());
        native.layers.insert(
            layer.name.clone(),
            nm::LayerProperties {
                handle,
                name: layer.name.clone(),
                color: if layer.flags.off {
                    -color_index.abs().max(1)
                } else {
                    color_index
                },
                linetype_name: if layer.line_type.is_empty() {
                    "Continuous".into()
                } else {
                    layer.line_type.clone()
                },
                lineweight: lineweight_to_native(&layer.line_weight),
                is_frozen: layer.flags.frozen,
                is_locked: layer.flags.locked,
                true_color,
                plot: true,
            },
        );
        native.tables.layer.insert(layer.name.clone(), handle);
    }

    if !native.layers.contains_key("0") {
        let zero = nm::LayerProperties::new("0");
        native.tables.layer.insert("0", zero.handle);
        native.layers.insert("0".into(), zero);
    }

    for entity in doc.entities() {
        if let Some(native_entity) = acadrust_entity_to_native(entity) {
            if native.add_entity(native_entity.clone()).is_err() {
                let mut fallback = native_entity;
                fallback.owner_handle = native.model_space_handle();
                let _ = native.add_entity(fallback);
            }
        }
    }

    native.repair_ownership();
    native
}

pub fn native_entity_to_acadrust(entity: &nm::Entity) -> Option<ar::EntityType> {
    match &entity.data {
        nm::EntityData::Line { start, end } => {
            let mut e = ar::Line::from_points(v3(start), v3(end));
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Line(e))
        }
        nm::EntityData::Circle { center, radius } => {
            let mut e = ar::Circle::new();
            e.center = v3(center);
            e.radius = *radius;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Circle(e))
        }
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let mut e = ar::Arc::new();
            e.center = v3(center);
            e.radius = *radius;
            e.start_angle = *start_angle;
            e.end_angle = *end_angle;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Arc(e))
        }
        nm::EntityData::Point { position } => {
            let mut e = ar::Point::new();
            e.location = v3(position);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Point(e))
        }
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            let mut e = ar::Ellipse::default();
            e.center = v3(center);
            e.major_axis = v3(major_axis);
            e.minor_axis_ratio = *ratio;
            e.start_parameter = *start_param;
            e.end_parameter = *end_param;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Ellipse(e))
        }
        nm::EntityData::Text {
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
            let mut e = ar::Text::new();
            e.insertion_point = v3(insertion);
            e.height = *height;
            e.value = value.clone();
            e.rotation = rotation.to_radians();
            e.style = style_name.clone();
            e.width_factor = *width_factor;
            e.oblique_angle = oblique_angle.to_radians();
            e.horizontal_alignment = native_text_halign(*horizontal_alignment);
            e.vertical_alignment = native_text_valign(*vertical_alignment);
            e.alignment_point = alignment_point.map(|point| v3(&point));
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Text(e))
        }
        nm::EntityData::MText {
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
            let mut e = ar::MText::new();
            e.insertion_point = v3(insertion);
            e.height = *height;
            e.rectangle_width = *width;
            e.rectangle_height = *rectangle_height;
            e.value = value.clone();
            e.rotation = rotation.to_radians();
            e.style = style_name.clone();
            e.attachment_point = native_mtext_attachment(*attachment_point);
            e.line_spacing_factor = *line_spacing_factor;
            e.drawing_direction = native_mtext_direction(*drawing_direction);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::MText(e))
        }
        nm::EntityData::LwPolyline { vertices, closed } => {
            let mut e = ar::LwPolyline {
                vertices: vertices
                    .iter()
                    .map(|vertex| {
                        let mut out = ar::LwVertex::new(Vector2::new(vertex.x, vertex.y));
                        out.bulge = vertex.bulge;
                        out
                    })
                    .collect(),
                is_closed: *closed,
                ..Default::default()
            };
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::LwPolyline(e))
        }
        nm::EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            start_tangent: _,
            end_tangent: _,
        } => {
            let mut e = ar::Spline::default();
            e.degree = *degree;
            e.knots = knots.clone();
            e.control_points = control_points.iter().map(v3).collect();
            e.weights = weights.clone();
            e.fit_points = fit_points.iter().map(v3).collect();
            e.flags.closed = *closed;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Spline(e))
        }
        nm::EntityData::Face3D {
            corners,
            invisible_edges,
        } => {
            let mut e = ar::Face3D::new(
                v3(&corners[0]),
                v3(&corners[1]),
                v3(&corners[2]),
                v3(&corners[3]),
            );
            e.invisible_edges = ar::InvisibleEdgeFlags::from_bits(*invisible_edges as u8);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Face3D(e))
        }
        nm::EntityData::Solid {
            corners,
            normal,
            thickness,
        } => {
            let mut e = ar::Solid::new(
                v3(&corners[0]),
                v3(&corners[1]),
                v3(&corners[2]),
                v3(&corners[3]),
            );
            e.normal = v3(normal);
            e.thickness = *thickness;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Solid(e))
        }
        nm::EntityData::Ray { origin, direction } => {
            let mut e = ar::Ray::new(v3(origin), v3(direction));
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Ray(e))
        }
        nm::EntityData::XLine { origin, direction } => {
            let mut e = ar::XLine::new(v3(origin), v3(direction));
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::XLine(e))
        }
        nm::EntityData::Shape {
            insertion,
            size,
            shape_number,
            name,
            rotation,
            relative_x_scale,
            oblique_angle,
            style_name,
            normal,
            thickness,
        } => {
            let mut e = ar::Shape::default();
            e.insertion_point = v3(insertion);
            e.size = *size;
            e.shape_number = i32::from(*shape_number);
            e.shape_name = name.clone();
            e.rotation = rotation.to_radians();
            e.relative_x_scale = *relative_x_scale;
            e.oblique_angle = oblique_angle.to_radians();
            e.style_name = style_name.clone();
            e.normal = v3(normal);
            e.thickness = *thickness;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Shape(e))
        }
        nm::EntityData::Attrib {
            tag,
            value,
            insertion,
            height,
        } => {
            let mut e = ar::AttributeEntity::new(tag.clone(), value.clone());
            e.insertion_point = v3(insertion);
            e.alignment_point = v3(insertion);
            e.height = *height;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::AttributeEntity(e))
        }
        nm::EntityData::AttDef {
            tag,
            prompt,
            default_value,
            insertion,
            height,
        } => {
            let mut e =
                ar::AttributeDefinition::new(tag.clone(), prompt.clone(), default_value.clone());
            e.insertion_point = v3(insertion);
            e.alignment_point = v3(insertion);
            e.height = *height;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::AttributeDefinition(e))
        }
        nm::EntityData::Leader {
            vertices,
            has_arrowhead,
        } => {
            let mut e = ar::Leader::from_vertices(vertices.iter().map(v3).collect());
            e.arrow_enabled = *has_arrowhead;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Leader(e))
        }
        nm::EntityData::MLine {
            vertices,
            style_name,
            scale,
            closed,
        } => {
            let mut e = ar::MLine::from_points(&vertices.iter().map(v3).collect::<Vec<_>>());
            e.style_name = style_name.clone();
            e.scale_factor = *scale;
            if *closed {
                e.close();
            }
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::MLine(e))
        }
        nm::EntityData::Image {
            insertion,
            u_vector,
            v_vector,
            image_size,
        } => {
            let mut e = ar::RasterImage::new("", v3(insertion), image_size[0], image_size[1]);
            e.u_vector = v3(u_vector);
            e.v_vector = v3(v_vector);
            e.size = Vector2::new(image_size[0], image_size[1]);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::RasterImage(e))
        }
        nm::EntityData::Wipeout {
            clip_vertices,
            elevation,
        } => {
            let mut e = if clip_vertices.len() >= 3 {
                ar::Wipeout::polygonal(
                    &clip_vertices
                        .iter()
                        .map(|vertex| Vector2::new(vertex[0], vertex[1]))
                        .collect::<Vec<_>>(),
                    *elevation,
                )
            } else if clip_vertices.len() == 2 {
                ar::Wipeout::from_corners(
                    Vector3::new(clip_vertices[0][0], clip_vertices[0][1], *elevation),
                    Vector3::new(clip_vertices[1][0], clip_vertices[1][1], *elevation),
                )
            } else {
                ar::Wipeout::new()
            };
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Wipeout(e))
        }
        nm::EntityData::Tolerance { text, insertion } => {
            let mut e = ar::Tolerance::with_text(v3(insertion), text.clone());
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Tolerance(e))
        }
        nm::EntityData::Solid3D { acis_data } => {
            let mut e = ar::Solid3D::from_sat(acis_data);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Solid3D(e))
        }
        nm::EntityData::Region { acis_data } => {
            let mut e = ar::Region::from_sat(acis_data);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Region(e))
        }
        nm::EntityData::PdfUnderlay { insertion, scale } => {
            let mut e = ar::Underlay::pdf_at(v3(insertion));
            e.set_scale_xyz(scale[0], scale[1], scale[2]);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Underlay(e))
        }
        nm::EntityData::Unknown { entity_type } => {
            let mut e = ar::UnknownEntity::new(entity_type.clone());
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Unknown(e))
        }
        nm::EntityData::Polyline {
            polyline_type,
            vertices,
            closed,
        } => Some(match polyline_type {
            nm::PolylineType::Polyline2D => {
                let mut e = ar::Polyline2D::new();
                e.vertices = vertices
                    .iter()
                    .map(|vertex| {
                        let mut out = ar::Vertex2D::new(v3(&vertex.position));
                        out.bulge = vertex.bulge;
                        out.start_width = vertex.start_width;
                        out.end_width = vertex.end_width;
                        out
                    })
                    .collect();
                e.flags.set_closed(*closed);
                apply_common(&mut e.common, entity);
                ar::EntityType::Polyline2D(e)
            }
            nm::PolylineType::Polyline3D => {
                let mut e = ar::Polyline3D::new();
                e.vertices = vertices
                    .iter()
                    .map(|vertex| ar::Vertex3DPolyline::new(v3(&vertex.position)))
                    .collect();
                e.flags.closed = *closed;
                apply_common(&mut e.common, entity);
                ar::EntityType::Polyline3D(e)
            }
            nm::PolylineType::PolygonMesh => {
                let mut e = ar::PolygonMeshEntity::new();
                if !vertices.is_empty() {
                    let (m_count, n_count) = infer_polygon_mesh_dimensions(vertices.len());
                    e.m_vertex_count = m_count;
                    e.n_vertex_count = n_count;
                }
                e.vertices = vertices
                    .iter()
                    .map(|vertex| {
                        let mut out = ar::PolygonMeshVertex::at(v3(&vertex.position));
                        out.common.layer = entity.layer_name.clone();
                        out
                    })
                    .collect();
                if *closed {
                    e.flags.insert(ar::PolygonMeshFlags::CLOSED_M);
                }
                apply_common(&mut e.common, entity);
                ar::EntityType::PolygonMesh(e)
            }
            nm::PolylineType::PolyfaceMesh => {
                let mut e = ar::PolyfaceMesh::new();
                e.vertices = vertices
                    .iter()
                    .map(|vertex| {
                        let mut out = ar::PolyfaceVertex::new(v3(&vertex.position));
                        out.start_width = vertex.start_width;
                        out.end_width = vertex.end_width;
                        out.bulge = vertex.bulge;
                        out
                    })
                    .collect();
                if *closed {
                    e.flags.insert(ar::PolyfaceMeshFlags::CLOSED);
                }
                apply_common(&mut e.common, entity);
                ar::EntityType::PolyfaceMesh(e)
            }
        }),
        nm::EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            let mut e = ar::Hatch::new();
            e.pattern.name = pattern_name.clone();
            e.is_solid = *solid_fill;
            e.paths = boundary_paths
                .iter()
                .map(native_hatch_path_to_acadrust)
                .collect();
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Hatch(e))
        }
        nm::EntityData::Dimension { .. } => native_dimension_to_acadrust(entity),
        nm::EntityData::MultiLeader { .. } => native_multileader_to_acadrust(entity),
        nm::EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            has_attribs: _,
            attribs,
        } => {
            let mut e = ar::Insert::new(block_name.clone(), v3(insertion));
            e.set_x_scale(scale[0]);
            e.set_y_scale(scale[1]);
            e.set_z_scale(scale[2]);
            e.rotation = rotation.to_radians();
            for attrib in attribs {
                if let nm::EntityData::Attrib {
                    tag,
                    value,
                    insertion,
                    height,
                } = &attrib.data
                {
                    let attr = ar::AttributeEntity {
                        tag: tag.clone(),
                        value: value.clone(),
                        insertion_point: v3(insertion),
                        height: *height,
                        ..Default::default()
                    };
                    e.attributes.push(attr);
                }
            }
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Insert(e))
        }
        nm::EntityData::Viewport {
            center,
            width,
            height,
        } => {
            let mut viewport = ar::Viewport::new();
            viewport.center = v3(center);
            viewport.width = *width;
            viewport.height = *height;
            apply_common(&mut viewport.common, entity);
            Some(ar::EntityType::Viewport(viewport))
        }
        nm::EntityData::Table {
            num_rows,
            num_cols,
            insertion,
            horizontal_direction,
            version,
            value_flag,
        } => {
            let mut table = ar::Table::new(v3(insertion), *num_rows as usize, *num_cols as usize);
            table.horizontal_direction = v3(horizontal_direction);
            table.data_version = *version;
            table.value_flags = *value_flag;
            apply_common(&mut table.common, entity);
            Some(ar::EntityType::Table(table))
        }
        nm::EntityData::Mesh {
            vertices,
            face_indices,
            ..
        } => {
            let mut mesh = ar::Mesh::new();
            for v in vertices {
                mesh.add_vertex(v3(v));
            }
            let mut i = 0;
            while i < face_indices.len() {
                let n = face_indices[i] as usize;
                if i + 1 + n <= face_indices.len() {
                    let verts: Vec<usize> =
                        face_indices[i + 1..i + 1 + n].iter().map(|&v| v as usize).collect();
                    mesh.add_face(ar::MeshFace::new(verts));
                    i += 1 + n;
                } else {
                    break;
                }
            }
            apply_common(&mut mesh.common, entity);
            Some(ar::EntityType::Mesh(mesh))
        }
        _ => None,
    }
}

pub fn acadrust_entity_to_native(entity: &ar::EntityType) -> Option<nm::Entity> {
    match entity {
        ar::EntityType::Line(line) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Line {
                start: [line.start.x, line.start.y, line.start.z],
                end: [line.end.x, line.end.y, line.end.z],
            },
        )),
        ar::EntityType::Circle(circle) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Circle {
                center: [circle.center.x, circle.center.y, circle.center.z],
                radius: circle.radius,
            },
        )),
        ar::EntityType::Arc(arc) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Arc {
                center: [arc.center.x, arc.center.y, arc.center.z],
                radius: arc.radius,
                start_angle: arc.start_angle,
                end_angle: arc.end_angle,
            },
        )),
        ar::EntityType::Point(point) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Point {
                position: [point.location.x, point.location.y, point.location.z],
            },
        )),
        ar::EntityType::Ellipse(ellipse) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Ellipse {
                center: [ellipse.center.x, ellipse.center.y, ellipse.center.z],
                major_axis: [
                    ellipse.major_axis.x,
                    ellipse.major_axis.y,
                    ellipse.major_axis.z,
                ],
                ratio: ellipse.minor_axis_ratio,
                start_param: ellipse.start_parameter,
                end_param: ellipse.end_parameter,
            },
        )),
        ar::EntityType::Text(text) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Text {
                insertion: [
                    text.insertion_point.x,
                    text.insertion_point.y,
                    text.insertion_point.z,
                ],
                height: text.height,
                value: text.value.clone(),
                rotation: text.rotation.to_degrees(),
                style_name: text.style.clone(),
                width_factor: text.width_factor,
                oblique_angle: text.oblique_angle.to_degrees(),
                horizontal_alignment: acad_text_halign(text.horizontal_alignment),
                vertical_alignment: acad_text_valign(text.vertical_alignment),
                alignment_point: text.alignment_point.map(|point| [point.x, point.y, point.z]),
            },
        )),
        ar::EntityType::MText(text) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::MText {
                insertion: [
                    text.insertion_point.x,
                    text.insertion_point.y,
                    text.insertion_point.z,
                ],
                height: text.height,
                width: text.rectangle_width,
                rectangle_height: text.rectangle_height,
                value: text.value.clone(),
                rotation: text.rotation.to_degrees(),
                style_name: text.style.clone(),
                attachment_point: acad_mtext_attachment(text.attachment_point),
                line_spacing_factor: text.line_spacing_factor,
                drawing_direction: acad_mtext_direction(text.drawing_direction),
            },
        )),
        ar::EntityType::LwPolyline(pline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::LwPolyline {
                vertices: pline
                    .vertices
                    .iter()
                    .map(|vertex| nm::LwVertex {
                        x: vertex.location.x,
                        y: vertex.location.y,
                        bulge: vertex.bulge,
                    })
                    .collect(),
                closed: pline.is_closed,
            },
        )),
        ar::EntityType::Spline(spline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Spline {
                degree: spline.degree,
                closed: spline.flags.closed,
                knots: spline.knots.clone(),
                control_points: spline
                    .control_points
                    .iter()
                    .map(|point| [point.x, point.y, point.z])
                    .collect(),
                weights: spline.weights.clone(),
                fit_points: spline
                    .fit_points
                    .iter()
                    .map(|point| [point.x, point.y, point.z])
                    .collect(),
                start_tangent: [0.0, 0.0, 0.0],
                end_tangent: [0.0, 0.0, 0.0],
            },
        )),
        ar::EntityType::Face3D(face) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Face3D {
                corners: [
                    [face.first_corner.x, face.first_corner.y, face.first_corner.z],
                    [face.second_corner.x, face.second_corner.y, face.second_corner.z],
                    [face.third_corner.x, face.third_corner.y, face.third_corner.z],
                    [face.fourth_corner.x, face.fourth_corner.y, face.fourth_corner.z],
                ],
                invisible_edges: normalize_face3d_invisible_edges(face.invisible_edges.bits()),
            },
        )),
        ar::EntityType::Solid(solid) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Solid {
                corners: [
                    [solid.first_corner.x, solid.first_corner.y, solid.first_corner.z],
                    [solid.second_corner.x, solid.second_corner.y, solid.second_corner.z],
                    [solid.third_corner.x, solid.third_corner.y, solid.third_corner.z],
                    [solid.fourth_corner.x, solid.fourth_corner.y, solid.fourth_corner.z],
                ],
                normal: [solid.normal.x, solid.normal.y, solid.normal.z],
                thickness: solid.thickness,
            },
        )),
        ar::EntityType::Ray(ray) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Ray {
                origin: [ray.base_point.x, ray.base_point.y, ray.base_point.z],
                direction: [ray.direction.x, ray.direction.y, ray.direction.z],
            },
        )),
        ar::EntityType::XLine(xline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::XLine {
                origin: [xline.base_point.x, xline.base_point.y, xline.base_point.z],
                direction: [xline.direction.x, xline.direction.y, xline.direction.z],
            },
        )),
        ar::EntityType::Shape(shape) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Shape {
                insertion: [
                    shape.insertion_point.x,
                    shape.insertion_point.y,
                    shape.insertion_point.z,
                ],
                size: shape.size,
                shape_number: shape.shape_number as i16,
                name: shape.shape_name.clone(),
                rotation: shape.rotation.to_degrees(),
                relative_x_scale: shape.relative_x_scale,
                oblique_angle: shape.oblique_angle.to_degrees(),
                style_name: shape.style_name.clone(),
                normal: [shape.normal.x, shape.normal.y, shape.normal.z],
                thickness: shape.thickness,
            },
        )),
        ar::EntityType::AttributeDefinition(attdef) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::AttDef {
                tag: attdef.tag.clone(),
                prompt: attdef.prompt.clone(),
                default_value: attdef.default_value.clone(),
                insertion: [
                    attdef.insertion_point.x,
                    attdef.insertion_point.y,
                    attdef.insertion_point.z,
                ],
                height: attdef.height,
            },
        )),
        ar::EntityType::AttributeEntity(attrib) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Attrib {
                tag: attrib.tag.clone(),
                value: attrib.value.clone(),
                insertion: [
                    attrib.insertion_point.x,
                    attrib.insertion_point.y,
                    attrib.insertion_point.z,
                ],
                height: attrib.height,
            },
        )),
        ar::EntityType::Leader(leader) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Leader {
                vertices: leader
                    .vertices
                    .iter()
                    .map(|vertex| [vertex.x, vertex.y, vertex.z])
                    .collect(),
                has_arrowhead: leader.arrow_enabled,
            },
        )),
        ar::EntityType::MLine(mline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::MLine {
                vertices: mline
                    .vertices
                    .iter()
                    .map(|vertex| [vertex.position.x, vertex.position.y, vertex.position.z])
                    .collect(),
                style_name: mline.style_name.clone(),
                scale: mline.scale_factor,
                closed: mline.is_closed(),
            },
        )),
        ar::EntityType::RasterImage(image) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Image {
                insertion: [
                    image.insertion_point.x,
                    image.insertion_point.y,
                    image.insertion_point.z,
                ],
                u_vector: [image.u_vector.x, image.u_vector.y, image.u_vector.z],
                v_vector: [image.v_vector.x, image.v_vector.y, image.v_vector.z],
                image_size: [image.size.x, image.size.y],
            },
        )),
        ar::EntityType::Wipeout(wipeout) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Wipeout {
                clip_vertices: wipeout
                    .clip_boundary_vertices
                    .iter()
                    .map(|vertex| [vertex.x, vertex.y])
                    .collect(),
                elevation: wipeout.insertion_point.z,
            },
        )),
        ar::EntityType::Tolerance(tolerance) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Tolerance {
                text: tolerance.text.clone(),
                insertion: [
                    tolerance.insertion_point.x,
                    tolerance.insertion_point.y,
                    tolerance.insertion_point.z,
                ],
            },
        )),
        ar::EntityType::Solid3D(solid) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Solid3D {
                acis_data: solid.acis_data.sat_data.clone(),
            },
        )),
        ar::EntityType::Region(region) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Region {
                acis_data: region.acis_data.sat_data.clone(),
            },
        )),
        ar::EntityType::Underlay(underlay) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::PdfUnderlay {
                insertion: [
                    underlay.insertion_point.x,
                    underlay.insertion_point.y,
                    underlay.insertion_point.z,
                ],
                scale: [underlay.x_scale, underlay.y_scale, underlay.z_scale],
            },
        )),
        ar::EntityType::Unknown(unknown) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Unknown {
                entity_type: unknown.dxf_name.clone(),
            },
        )),
        ar::EntityType::Polyline2D(pline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline2D,
                vertices: pline
                    .vertices
                    .iter()
                    .map(|vertex| nm::PolylineVertex {
                        position: [vertex.location.x, vertex.location.y, vertex.location.z],
                        bulge: vertex.bulge,
                        start_width: vertex.start_width,
                        end_width: vertex.end_width,
                    })
                    .collect(),
                closed: pline.is_closed(),
            },
        )),
        ar::EntityType::Polyline3D(pline) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline3D,
                vertices: pline
                    .vertices
                    .iter()
                    .map(|vertex| nm::PolylineVertex {
                        position: [vertex.position.x, vertex.position.y, vertex.position.z],
                        bulge: 0.0,
                        start_width: 0.0,
                        end_width: 0.0,
                    })
                    .collect(),
                closed: pline.is_closed(),
            },
        )),
        ar::EntityType::PolygonMesh(mesh) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolygonMesh,
                vertices: mesh
                    .vertices
                    .iter()
                    .map(|vertex| nm::PolylineVertex {
                        position: [vertex.location.x, vertex.location.y, vertex.location.z],
                        bulge: 0.0,
                        start_width: 0.0,
                        end_width: 0.0,
                    })
                    .collect(),
                closed: mesh.is_closed_m(),
            },
        )),
        ar::EntityType::PolyfaceMesh(mesh) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolyfaceMesh,
                vertices: mesh
                    .vertices
                    .iter()
                    .map(|vertex| nm::PolylineVertex {
                        position: [vertex.location.x, vertex.location.y, vertex.location.z],
                        bulge: vertex.bulge,
                        start_width: vertex.start_width,
                        end_width: vertex.end_width,
                    })
                    .collect(),
                closed: mesh.flags.contains(ar::PolyfaceMeshFlags::CLOSED),
            },
        )),
        ar::EntityType::Hatch(hatch) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Hatch {
                pattern_name: hatch.pattern.name.clone(),
                solid_fill: hatch.is_solid,
                boundary_paths: hatch.paths.iter().map(acad_hatch_path_to_native).collect(),
            },
        )),
        ar::EntityType::Dimension(dimension) => acad_dimension_to_native(entity, dimension),
        ar::EntityType::MultiLeader(multileader) => acad_multileader_to_native(entity, multileader),
        ar::EntityType::Insert(insert) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Insert {
                block_name: insert.block_name.clone(),
                insertion: [
                    insert.insert_point.x,
                    insert.insert_point.y,
                    insert.insert_point.z,
                ],
                scale: [insert.x_scale(), insert.y_scale(), insert.z_scale()],
                rotation: insert.rotation.to_degrees(),
                has_attribs: !insert.attributes.is_empty(),
                attribs: insert
                    .attributes
                    .iter()
                    .map(|attrib| {
                        let mut entity = nm::Entity::new(nm::EntityData::Attrib {
                            tag: attrib.tag.clone(),
                            value: attrib.value.clone(),
                            insertion: [
                                attrib.insertion_point.x,
                                attrib.insertion_point.y,
                                attrib.insertion_point.z,
                            ],
                            height: attrib.height,
                        });
                        entity.layer_name = attrib.common.layer.clone();
                        entity
                    })
                    .collect(),
            },
        )),
        ar::EntityType::Viewport(viewport) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Viewport {
                center: [viewport.center.x, viewport.center.y, viewport.center.z],
                width: viewport.width,
                height: viewport.height,
            },
        )),
        ar::EntityType::Table(table) => Some(native_common_from_acadrust(
            entity,
            nm::EntityData::Table {
                num_rows: table.row_count() as i32,
                num_cols: table.column_count() as i32,
                insertion: [
                    table.insertion_point.x,
                    table.insertion_point.y,
                    table.insertion_point.z,
                ],
                horizontal_direction: [
                    table.horizontal_direction.x,
                    table.horizontal_direction.y,
                    table.horizontal_direction.z,
                ],
                version: table.data_version,
                value_flag: table.value_flags,
            },
        )),
        ar::EntityType::Mesh(mesh) => {
            let mut face_indices: Vec<i32> = Vec::new();
            for face in &mesh.faces {
                face_indices.push(face.vertices.len() as i32);
                for &v in &face.vertices {
                    face_indices.push(v as i32);
                }
            }
            Some(native_common_from_acadrust(
                entity,
                nm::EntityData::Mesh {
                    vertex_count: mesh.vertex_count() as i32,
                    face_count: mesh.face_count() as i32,
                    vertices: mesh.vertices.iter().map(|v| [v.x, v.y, v.z]).collect(),
                    face_indices,
                },
            ))
        }
        _ => None,
    }
}

fn native_common_from_acadrust(entity: &ar::EntityType, data: nm::EntityData) -> nm::Entity {
    let common = entity.common();
    let (color_index, true_color) = color_to_native(&common.color);
    let transparency = common.transparency.alpha();
    let mut native = nm::Entity::new(data);
    native.handle = nm::Handle::new(common.handle.value());
    native.owner_handle = nm::Handle::new(common.owner_handle.value());
    native.layer_name = common.layer.clone();
    native.linetype_name = common.linetype.clone();
    native.linetype_scale = common.linetype_scale;
    native.color_index = color_index;
    native.true_color = true_color;
    native.lineweight = lineweight_to_native(&common.line_weight);
    native.invisible = common.invisible;
    native.transparency = i32::from(transparency);
    native.xdata = xdata_from_acadrust(&common.extended_data);
    native
}

fn apply_common(common: &mut ar::EntityCommon, entity: &nm::Entity) {
    common.handle = Handle::new(entity.handle.value());
    common.owner_handle = Handle::new(entity.owner_handle.value());
    common.layer = entity.layer_name.clone();
    common.linetype = entity.linetype_name.clone();
    common.linetype_scale = entity.linetype_scale;
    common.color = if entity.true_color != 0 {
        Color::from_rgb(
            ((entity.true_color >> 16) & 0xFF) as u8,
            ((entity.true_color >> 8) & 0xFF) as u8,
            (entity.true_color & 0xFF) as u8,
        )
    } else {
        native_color(entity.color_index)
    };
    common.line_weight = native_lineweight(entity.lineweight);
    common.invisible = entity.invisible;
    common.transparency = native_transparency(entity.transparency);
    common.extended_data = xdata_to_acadrust(&entity.xdata);
}

// ── XData bridge ────────────────────────────────────────────────────────
//
// `nm::Entity.xdata` 的存储格式：`Vec<(app_name, Vec<(group_code, value_str)>)>`。
// `ar::EntityCommon.extended_data` 是按应用名分组的 `ExtendedDataRecord` 列表，每个
// record 内的 value 是 `XDataValue` 枚举。下面两个函数做往返投影，
// 覆盖 DXF 1000-1071 的主流 group code。未识别的 group code 走 `String` 兜底。

fn xdata_to_acadrust(xdata: &[(String, Vec<(i16, String)>)]) -> acadrust::xdata::ExtendedData {
    use acadrust::xdata::{ExtendedDataRecord, XDataValue};
    let mut out = acadrust::xdata::ExtendedData::new();
    for (app, entries) in xdata {
        let mut rec = ExtendedDataRecord::new(app.as_str());
        for (code, value) in entries {
            let v = match *code {
                1000 => XDataValue::String(value.clone()),
                1002 => XDataValue::ControlString(value.clone()),
                1003 => XDataValue::LayerName(value.clone()),
                1004 => {
                    let bytes = hex_to_bytes(value).unwrap_or_default();
                    XDataValue::BinaryData(bytes)
                }
                1005 => {
                    let h = u64::from_str_radix(value.trim_start_matches("0x"), 16).unwrap_or(0);
                    XDataValue::Handle(Handle::new(h))
                }
                1010 | 1011 | 1012 | 1013 => {
                    let pt = parse_point3(value);
                    match *code {
                        1010 => XDataValue::Point3D(pt),
                        1011 => XDataValue::Position3D(pt),
                        1012 => XDataValue::Displacement3D(pt),
                        _ => XDataValue::Direction3D(pt),
                    }
                }
                1040 => XDataValue::Real(value.parse().unwrap_or(0.0)),
                1041 => XDataValue::Distance(value.parse().unwrap_or(0.0)),
                1042 => XDataValue::ScaleFactor(value.parse().unwrap_or(1.0)),
                1070 => XDataValue::Integer16(value.parse().unwrap_or(0)),
                1071 => XDataValue::Integer32(value.parse().unwrap_or(0)),
                _ => XDataValue::String(value.clone()),
            };
            rec.add_value(v);
        }
        out.add_record(rec);
    }
    out
}

fn xdata_from_acadrust(ext: &acadrust::xdata::ExtendedData) -> Vec<(String, Vec<(i16, String)>)> {
    use acadrust::xdata::XDataValue;
    ext.records()
        .iter()
        .map(|rec| {
            let entries: Vec<(i16, String)> = rec
                .values
                .iter()
                .map(|v| match v {
                    XDataValue::String(s) => (1000, s.clone()),
                    XDataValue::ControlString(s) => (1002, s.clone()),
                    XDataValue::LayerName(s) => (1003, s.clone()),
                    XDataValue::BinaryData(b) => (1004, bytes_to_hex(b)),
                    XDataValue::Handle(h) => (1005, format!("{:X}", h.value())),
                    XDataValue::Point3D(p) => (1010, format_point3(p)),
                    XDataValue::Position3D(p) => (1011, format_point3(p)),
                    XDataValue::Displacement3D(p) => (1012, format_point3(p)),
                    XDataValue::Direction3D(p) => (1013, format_point3(p)),
                    XDataValue::Real(r) => (1040, r.to_string()),
                    XDataValue::Distance(r) => (1041, r.to_string()),
                    XDataValue::ScaleFactor(r) => (1042, r.to_string()),
                    XDataValue::Integer16(i) => (1070, i.to_string()),
                    XDataValue::Integer32(i) => (1071, i.to_string()),
                })
                .collect();
            (rec.application_name.clone(), entries)
        })
        .collect()
}

fn format_point3(p: &Vector3) -> String {
    format!("{},{},{}", p.x, p.y, p.z)
}

fn parse_point3(s: &str) -> Vector3 {
    let parts: Vec<f64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    Vector3::new(
        parts.first().copied().unwrap_or(0.0),
        parts.get(1).copied().unwrap_or(0.0),
        parts.get(2).copied().unwrap_or(0.0),
    )
}

fn bytes_to_hex(b: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(b.len() * 2);
    for byte in b {
        let _ = write!(out, "{:02X}", byte);
    }
    out
}

fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn native_dimension_to_acadrust(entity: &nm::Entity) -> Option<ar::EntityType> {
    let nm::EntityData::Dimension {
        dim_type,
        block_name,
        style_name,
        definition_point,
        text_midpoint,
        text_override,
        attachment_point: _,
        measurement,
        text_rotation,
        horizontal_direction,
        flip_arrow1: _,
        flip_arrow2: _,
        first_point,
        second_point,
        angle_vertex,
        dimension_arc,
        leader_length,
        rotation,
        ext_line_rotation,
    } = &entity.data
    else {
        return None;
    };

    let base_type = dim_type & 0x0F;
    let mut dimension = match base_type {
        0 => {
            let mut dim = ar::DimensionLinear::new(v3(first_point), v3(second_point));
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            dim.rotation = rotation.to_radians();
            dim.ext_line_rotation = ext_line_rotation.to_radians();
            ar::Dimension::Linear(dim)
        }
        1 => {
            let mut dim = ar::DimensionAligned::new(v3(first_point), v3(second_point));
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            dim.ext_line_rotation = ext_line_rotation.to_radians();
            ar::Dimension::Aligned(dim)
        }
        2 => {
            let mut dim =
                ar::DimensionAngular2Ln::new(v3(angle_vertex), v3(first_point), v3(second_point));
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            dim.dimension_arc = v3(dimension_arc);
            ar::Dimension::Angular2Ln(dim)
        }
        3 => {
            let mut dim = ar::DimensionDiameter::new(v3(angle_vertex), v3(definition_point));
            dim.base.definition_point = v3(definition_point);
            dim.leader_length = *leader_length;
            ar::Dimension::Diameter(dim)
        }
        4 => {
            let mut dim = ar::DimensionRadius::new(v3(angle_vertex), v3(definition_point));
            dim.base.definition_point = v3(definition_point);
            dim.leader_length = *leader_length;
            ar::Dimension::Radius(dim)
        }
        5 => {
            let mut dim =
                ar::DimensionAngular3Pt::new(v3(angle_vertex), v3(first_point), v3(second_point));
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            ar::Dimension::Angular3Pt(dim)
        }
        6 => {
            let mut dim =
                ar::DimensionOrdinate::new(v3(first_point), v3(second_point), (dim_type & 0x40) != 0);
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            ar::Dimension::Ordinate(dim)
        }
        _ => {
            let mut dim = ar::DimensionLinear::new(v3(first_point), v3(second_point));
            dim.definition_point = v3(definition_point);
            dim.base.definition_point = v3(definition_point);
            ar::Dimension::Linear(dim)
        }
    };

    let base = dimension.base_mut();
    base.style_name = style_name.clone();
    base.block_name = block_name.clone();
    base.text = text_override.clone();
    base.user_text = (!text_override.trim().is_empty()).then(|| text_override.clone());
    base.text_middle_point = v3(text_midpoint);
    base.insertion_point = v3(text_midpoint);
    base.text_rotation = text_rotation.to_radians();
    base.horizontal_direction = horizontal_direction.to_radians();
    base.actual_measurement = *measurement;
    apply_common(&mut base.common, entity);

    Some(ar::EntityType::Dimension(dimension))
}

fn acad_dimension_to_native(entity: &ar::EntityType, dimension: &ar::Dimension) -> Option<nm::Entity> {
    let base = dimension.base();
    let (
        dim_type,
        definition_point,
        first_point,
        second_point,
        angle_vertex,
        dimension_arc,
        leader_length,
        rotation,
        ext_line_rotation,
    ) = match dimension {
        ar::Dimension::Linear(dim) => (
            0,
            [dim.definition_point.x, dim.definition_point.y, dim.definition_point.z],
            [dim.first_point.x, dim.first_point.y, dim.first_point.z],
            [dim.second_point.x, dim.second_point.y, dim.second_point.z],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            0.0,
            dim.rotation.to_degrees(),
            dim.ext_line_rotation.to_degrees(),
        ),
        ar::Dimension::Aligned(dim) => (
            1,
            [dim.definition_point.x, dim.definition_point.y, dim.definition_point.z],
            [dim.first_point.x, dim.first_point.y, dim.first_point.z],
            [dim.second_point.x, dim.second_point.y, dim.second_point.z],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            0.0,
            0.0,
            dim.ext_line_rotation.to_degrees(),
        ),
        ar::Dimension::Angular2Ln(dim) => (
            2,
            [dim.definition_point.x, dim.definition_point.y, dim.definition_point.z],
            [dim.first_point.x, dim.first_point.y, dim.first_point.z],
            [dim.second_point.x, dim.second_point.y, dim.second_point.z],
            [dim.angle_vertex.x, dim.angle_vertex.y, dim.angle_vertex.z],
            [dim.dimension_arc.x, dim.dimension_arc.y, dim.dimension_arc.z],
            0.0,
            0.0,
            0.0,
        ),
        ar::Dimension::Diameter(dim) => (
            3,
            [
                dim.base.definition_point.x,
                dim.base.definition_point.y,
                dim.base.definition_point.z,
            ],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [dim.angle_vertex.x, dim.angle_vertex.y, dim.angle_vertex.z],
            [0.0, 0.0, 0.0],
            dim.leader_length,
            0.0,
            0.0,
        ),
        ar::Dimension::Radius(dim) => (
            4,
            [
                dim.base.definition_point.x,
                dim.base.definition_point.y,
                dim.base.definition_point.z,
            ],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [dim.angle_vertex.x, dim.angle_vertex.y, dim.angle_vertex.z],
            [0.0, 0.0, 0.0],
            dim.leader_length,
            0.0,
            0.0,
        ),
        ar::Dimension::Angular3Pt(dim) => (
            5,
            [dim.definition_point.x, dim.definition_point.y, dim.definition_point.z],
            [dim.first_point.x, dim.first_point.y, dim.first_point.z],
            [dim.second_point.x, dim.second_point.y, dim.second_point.z],
            [dim.angle_vertex.x, dim.angle_vertex.y, dim.angle_vertex.z],
            [0.0, 0.0, 0.0],
            0.0,
            0.0,
            0.0,
        ),
        ar::Dimension::Ordinate(dim) => (
            if dim.is_ordinate_type_x { 6 | 0x40 } else { 6 },
            [dim.definition_point.x, dim.definition_point.y, dim.definition_point.z],
            [
                dim.feature_location.x,
                dim.feature_location.y,
                dim.feature_location.z,
            ],
            [
                dim.leader_endpoint.x,
                dim.leader_endpoint.y,
                dim.leader_endpoint.z,
            ],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            0.0,
            0.0,
            0.0,
        ),
    };

    Some(native_common_from_acadrust(
        entity,
        nm::EntityData::Dimension {
            dim_type,
            block_name: base.block_name.clone(),
            style_name: base.style_name.clone(),
            definition_point,
            text_midpoint: [
                base.text_middle_point.x,
                base.text_middle_point.y,
                base.text_middle_point.z,
            ],
            text_override: base.user_text.clone().unwrap_or_else(|| base.text.clone()),
            attachment_point: 0,
            measurement: base.actual_measurement,
            text_rotation: base.text_rotation.to_degrees(),
            horizontal_direction: base.horizontal_direction.to_degrees(),
            flip_arrow1: false,
            flip_arrow2: false,
            first_point,
            second_point,
            angle_vertex,
            dimension_arc,
            leader_length,
            rotation,
            ext_line_rotation,
        },
    ))
}

fn native_multileader_to_acadrust(entity: &nm::Entity) -> Option<ar::EntityType> {
    let nm::EntityData::MultiLeader {
        content_type,
        text_label,
        style_name: _,
        arrowhead_size,
        landing_gap: _,
        dogleg_length,
        property_override_flags: _,
        path_type,
        line_color: _,
        leader_line_weight: _,
        enable_landing,
        enable_dogleg,
        enable_annotation_scale,
        scale_factor,
        text_attachment_direction: _,
        text_bottom_attachment_type: _,
        text_top_attachment_type: _,
        text_location,
        leader_vertices,
        leader_root_lengths,
    } = &entity.data
    else {
        return None;
    };

    let text_point = text_location
        .as_ref()
        .map(v3)
        .unwrap_or_else(Vector3::zero);
    let split_roots = split_native_mleader_roots(leader_vertices, leader_root_lengths);
    let mut ml = ar::MultiLeader::with_text(
        text_label,
        text_point,
        split_roots
            .first()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|point| v3(&point))
            .collect(),
    );
    if split_roots.len() > 1 {
        for root_points in split_roots.iter().skip(1) {
            let root = ml.context.add_leader_root();
            root.create_line(root_points.iter().map(v3).collect());
        }
    }
    ml.content_type = native_mleader_content_type(*content_type);
    ml.context.text_string = text_label.clone();
    ml.context.text_location = text_point;
    ml.arrowhead_size = *arrowhead_size;
    ml.dogleg_length = *dogleg_length;
    ml.enable_landing = *enable_landing;
    ml.enable_dogleg = *enable_dogleg;
    ml.enable_annotation_scale = *enable_annotation_scale;
    ml.scale_factor = *scale_factor;
    ml.path_type = native_mleader_path_type(*path_type);
    apply_common(&mut ml.common, entity);
    Some(ar::EntityType::MultiLeader(ml))
}

fn acad_multileader_to_native(
    entity: &ar::EntityType,
    multileader: &ar::MultiLeader,
) -> Option<nm::Entity> {
    let (leader_vertices, leader_root_lengths) = flatten_acad_mleader_roots(multileader);

    Some(native_common_from_acadrust(
        entity,
        nm::EntityData::MultiLeader {
            content_type: acad_mleader_content_type(multileader.content_type),
            text_label: multileader.context.text_string.clone(),
            style_name: "Standard".into(),
            arrowhead_size: multileader.arrowhead_size,
            landing_gap: 0.0,
            dogleg_length: multileader.dogleg_length,
            property_override_flags: 0,
            path_type: acad_mleader_path_type(multileader.path_type),
            line_color: 0,
            leader_line_weight: -1,
            enable_landing: multileader.enable_landing,
            enable_dogleg: multileader.enable_dogleg,
            enable_annotation_scale: multileader.enable_annotation_scale,
            scale_factor: multileader.scale_factor,
            text_attachment_direction: 0,
            text_bottom_attachment_type: 9,
            text_top_attachment_type: 9,
            text_location: Some([
                multileader.context.text_location.x,
                multileader.context.text_location.y,
                multileader.context.text_location.z,
            ]),
            leader_vertices,
            leader_root_lengths,
        },
    ))
}

fn split_native_mleader_roots(
    leader_vertices: &[[f64; 3]],
    leader_root_lengths: &[usize],
) -> Vec<Vec<[f64; 3]>> {
    if leader_vertices.is_empty() {
        return Vec::new();
    }
    if leader_root_lengths.is_empty() {
        return vec![leader_vertices.to_vec()];
    }
    let mut roots = Vec::new();
    let mut offset = 0usize;
    for &len in leader_root_lengths {
        if len == 0 {
            continue;
        }
        let end = (offset + len).min(leader_vertices.len());
        if offset >= end {
            break;
        }
        roots.push(leader_vertices[offset..end].to_vec());
        offset = end;
    }
    if offset < leader_vertices.len() {
        roots.push(leader_vertices[offset..].to_vec());
    }
    if roots.is_empty() {
        roots.push(leader_vertices.to_vec());
    }
    roots
}

fn flatten_acad_mleader_roots(multileader: &ar::MultiLeader) -> (Vec<[f64; 3]>, Vec<usize>) {
    let mut leader_vertices = Vec::new();
    let mut leader_root_lengths = Vec::new();
    for root in &multileader.context.leader_roots {
        let start = leader_vertices.len();
        for line in &root.lines {
            leader_vertices.extend(line.points.iter().map(|point| [point.x, point.y, point.z]));
        }
        let len = leader_vertices.len().saturating_sub(start);
        if len > 0 {
            leader_root_lengths.push(len);
        }
    }
    (leader_vertices, leader_root_lengths)
}

fn v3(arr: &[f64; 3]) -> Vector3 {
    Vector3::new(arr[0], arr[1], arr[2])
}

fn native_color(color_index: i16) -> Color {
    match color_index {
        256 => Color::ByLayer,
        -2 => Color::ByBlock,
        value if value > 0 => Color::Index(value as u8),
        _ => Color::ByLayer,
    }
}

fn native_text_halign(value: i16) -> ar::TextHorizontalAlignment {
    match value {
        1 => ar::TextHorizontalAlignment::Center,
        2 => ar::TextHorizontalAlignment::Right,
        3 => ar::TextHorizontalAlignment::Aligned,
        4 => ar::TextHorizontalAlignment::Middle,
        5 => ar::TextHorizontalAlignment::Fit,
        _ => ar::TextHorizontalAlignment::Left,
    }
}

fn native_text_valign(value: i16) -> ar::TextVerticalAlignment {
    match value {
        1 => ar::TextVerticalAlignment::Bottom,
        2 => ar::TextVerticalAlignment::Middle,
        3 => ar::TextVerticalAlignment::Top,
        _ => ar::TextVerticalAlignment::Baseline,
    }
}

fn acad_text_halign(value: ar::TextHorizontalAlignment) -> i16 {
    match value {
        ar::TextHorizontalAlignment::Center => 1,
        ar::TextHorizontalAlignment::Right => 2,
        ar::TextHorizontalAlignment::Aligned => 3,
        ar::TextHorizontalAlignment::Middle => 4,
        ar::TextHorizontalAlignment::Fit => 5,
        ar::TextHorizontalAlignment::Left => 0,
    }
}

fn acad_text_valign(value: ar::TextVerticalAlignment) -> i16 {
    match value {
        ar::TextVerticalAlignment::Bottom => 1,
        ar::TextVerticalAlignment::Middle => 2,
        ar::TextVerticalAlignment::Top => 3,
        ar::TextVerticalAlignment::Baseline => 0,
    }
}

fn native_mtext_attachment(value: i16) -> ar::AttachmentPoint {
    match value {
        2 => ar::AttachmentPoint::TopCenter,
        3 => ar::AttachmentPoint::TopRight,
        4 => ar::AttachmentPoint::MiddleLeft,
        5 => ar::AttachmentPoint::MiddleCenter,
        6 => ar::AttachmentPoint::MiddleRight,
        7 => ar::AttachmentPoint::BottomLeft,
        8 => ar::AttachmentPoint::BottomCenter,
        9 => ar::AttachmentPoint::BottomRight,
        _ => ar::AttachmentPoint::TopLeft,
    }
}

fn acad_mtext_attachment(value: ar::AttachmentPoint) -> i16 {
    match value {
        ar::AttachmentPoint::TopLeft => 1,
        ar::AttachmentPoint::TopCenter => 2,
        ar::AttachmentPoint::TopRight => 3,
        ar::AttachmentPoint::MiddleLeft => 4,
        ar::AttachmentPoint::MiddleCenter => 5,
        ar::AttachmentPoint::MiddleRight => 6,
        ar::AttachmentPoint::BottomLeft => 7,
        ar::AttachmentPoint::BottomCenter => 8,
        ar::AttachmentPoint::BottomRight => 9,
    }
}

fn native_mtext_direction(value: i16) -> ar::DrawingDirection {
    match value {
        3 => ar::DrawingDirection::TopToBottom,
        5 => ar::DrawingDirection::ByStyle,
        _ => ar::DrawingDirection::LeftToRight,
    }
}

fn acad_mtext_direction(value: ar::DrawingDirection) -> i16 {
    match value {
        ar::DrawingDirection::TopToBottom => 3,
        ar::DrawingDirection::ByStyle => 5,
        ar::DrawingDirection::LeftToRight => 1,
    }
}

fn native_mleader_content_type(value: i16) -> ar::LeaderContentType {
    match value {
        2 => ar::LeaderContentType::Block,
        3 => ar::LeaderContentType::Tolerance,
        1 => ar::LeaderContentType::MText,
        _ => ar::LeaderContentType::None,
    }
}

fn acad_mleader_content_type(value: ar::LeaderContentType) -> i16 {
    match value {
        ar::LeaderContentType::Block => 2,
        ar::LeaderContentType::Tolerance => 3,
        ar::LeaderContentType::MText => 1,
        ar::LeaderContentType::None => 0,
    }
}

fn native_mleader_path_type(value: i16) -> ar::MultiLeaderPathType {
    match value {
        2 => ar::MultiLeaderPathType::Spline,
        0 => ar::MultiLeaderPathType::Invisible,
        _ => ar::MultiLeaderPathType::StraightLineSegments,
    }
}

fn acad_mleader_path_type(value: ar::MultiLeaderPathType) -> i16 {
    match value {
        ar::MultiLeaderPathType::Spline => 2,
        ar::MultiLeaderPathType::Invisible => 0,
        ar::MultiLeaderPathType::StraightLineSegments => 1,
    }
}

fn color_to_native(color: &Color) -> (i16, i32) {
    match color {
        Color::ByLayer => (256, 0),
        Color::ByBlock => (-2, 0),
        Color::Index(i) => (*i as i16, 0),
        Color::Rgb { r, g, b } => (256, pack_true_color(*r, *g, *b)),
    }
}

fn pack_true_color(r: u8, g: u8, b: u8) -> i32 {
    ((r as i32) << 16) | ((g as i32) << 8) | (b as i32)
}

fn native_lineweight(value: i16) -> LineWeight {
    match value {
        -1 => LineWeight::ByLayer,
        -2 => LineWeight::ByBlock,
        -3 => LineWeight::Default,
        other => LineWeight::Value(other),
    }
}

fn lineweight_to_native(value: &LineWeight) -> i16 {
    match value {
        LineWeight::ByLayer => -1,
        LineWeight::ByBlock => -2,
        LineWeight::Default => -3,
        LineWeight::Value(v) => *v,
    }
}

fn native_transparency(value: i32) -> crate::types::Transparency {
    if value == 0 {
        crate::types::Transparency::OPAQUE
    } else if (value >> 24) == 0 {
        crate::types::Transparency::new(value.clamp(0, 255) as u8)
    } else {
        crate::types::Transparency::from_alpha_value(value as u32)
    }
}

fn native_hatch_path_to_acadrust(path: &nm::HatchBoundaryPath) -> ar::BoundaryPath {
    let flags = ar::BoundaryPathFlags::from_bits(path.flags as u32);
    let mut out = ar::BoundaryPath::with_flags(flags);
    for edge in &path.edges {
        out.add_edge(match edge {
            nm::HatchEdge::Line { start, end } => ar::BoundaryEdge::Line(ar::LineEdge {
                start: Vector2::new(start[0], start[1]),
                end: Vector2::new(end[0], end[1]),
            }),
            nm::HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                is_ccw,
            } => ar::BoundaryEdge::CircularArc(ar::CircularArcEdge {
                center: Vector2::new(center[0], center[1]),
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
                counter_clockwise: *is_ccw,
            }),
            nm::HatchEdge::EllipticArc {
                center,
                major_endpoint,
                minor_ratio,
                start_angle,
                end_angle,
                is_ccw,
            } => ar::BoundaryEdge::EllipticArc(ar::EllipticArcEdge {
                center: Vector2::new(center[0], center[1]),
                major_axis_endpoint: Vector2::new(major_endpoint[0], major_endpoint[1]),
                minor_axis_ratio: *minor_ratio,
                start_angle: *start_angle,
                end_angle: *end_angle,
                counter_clockwise: *is_ccw,
            }),
            nm::HatchEdge::Polyline { closed, vertices } => {
                ar::BoundaryEdge::Polyline(ar::PolylineEdge {
                    is_closed: *closed,
                    vertices: vertices
                        .iter()
                        .map(|vertex| Vector3::new(vertex[0], vertex[1], vertex[2]))
                        .collect(),
                })
            }
        });
    }
    out
}

fn acad_hatch_path_to_native(path: &ar::BoundaryPath) -> nm::HatchBoundaryPath {
    nm::HatchBoundaryPath {
        flags: path.flags.bits() as i32,
        edges: path
            .edges
            .iter()
            .filter_map(|edge| match edge {
                ar::BoundaryEdge::Line(line) => Some(nm::HatchEdge::Line {
                    start: [line.start.x, line.start.y],
                    end: [line.end.x, line.end.y],
                }),
                ar::BoundaryEdge::CircularArc(arc) => Some(nm::HatchEdge::CircularArc {
                    center: [arc.center.x, arc.center.y],
                    radius: arc.radius,
                    start_angle: arc.start_angle,
                    end_angle: arc.end_angle,
                    is_ccw: arc.counter_clockwise,
                }),
                ar::BoundaryEdge::EllipticArc(arc) => Some(nm::HatchEdge::EllipticArc {
                    center: [arc.center.x, arc.center.y],
                    major_endpoint: [arc.major_axis_endpoint.x, arc.major_axis_endpoint.y],
                    minor_ratio: arc.minor_axis_ratio,
                    start_angle: arc.start_angle,
                    end_angle: arc.end_angle,
                    is_ccw: arc.counter_clockwise,
                }),
                ar::BoundaryEdge::Polyline(poly) => Some(nm::HatchEdge::Polyline {
                    closed: poly.is_closed,
                    vertices: poly.vertices.iter().map(|v| [v.x, v.y, v.z]).collect(),
                }),
                ar::BoundaryEdge::Spline(_) => None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_to_acadrust_preserves_arc_and_text_rotation_units() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(nm::Entity::new(nm::EntityData::Arc {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
                start_angle: 15.0,
                end_angle: 120.0,
            }))
            .expect("arc should be added");
        native
            .add_entity(nm::Entity::new(nm::EntityData::Text {
                insertion: [1.0, 2.0, 0.0],
                height: 2.5,
                value: "Hello".into(),
                rotation: 90.0,
                style_name: "Standard".into(),
                width_factor: 0.9,
                oblique_angle: 10.0,
                horizontal_alignment: 2,
                vertical_alignment: 3,
                alignment_point: Some([2.0, 3.0, 0.0]),
            }))
            .expect("text should be added");

        let acad = native_doc_to_acadrust(&native);
        let mut saw_arc = false;
        let mut saw_text = false;

        for entity in acad.entities() {
            match entity {
                ar::EntityType::Arc(arc) => {
                    saw_arc = true;
                    assert_eq!(arc.start_angle, 15.0);
                    assert_eq!(arc.end_angle, 120.0);
                }
                ar::EntityType::Text(text) => {
                    saw_text = true;
                    assert!((text.rotation.to_degrees() - 90.0).abs() < 1e-9);
                    assert_eq!(text.style, "Standard");
                }
                _ => {}
            }
        }

        assert!(saw_arc, "arc should survive bridge");
        assert!(saw_text, "text should survive bridge");
    }

    #[test]
    fn acadrust_to_native_restores_common_fields_and_rotation_units() {
        let mut acad = acadrust::CadDocument::new();
        let mut text = ar::Text::new();
        text.insertion_point = Vector3::new(3.0, 4.0, 0.0);
        text.height = 1.5;
        text.value = "World".into();
        text.style = "Standard".into();
        text.width_factor = 0.8;
        text.oblique_angle = 12_f64.to_radians();
        text.horizontal_alignment = ar::TextHorizontalAlignment::Right;
        text.vertical_alignment = ar::TextVerticalAlignment::Top;
        text.alignment_point = Some(Vector3::new(6.0, 7.0, 0.0));
        text.rotation = 45_f64.to_radians();
        text.common.color = Color::Index(3);
        text.common.line_weight = LineWeight::Value(35);
        acad.add_entity(ar::EntityType::Text(text))
            .expect("text should be added");

        let native = acadrust_doc_to_native(&acad);
        let entity = native
            .entities
            .iter()
            .find(|entity| matches!(entity.data, nm::EntityData::Text { .. }))
            .expect("native text should exist");

        assert_eq!(entity.color_index, 3);
        assert_eq!(entity.lineweight, 35);
        match &entity.data {
            nm::EntityData::Text {
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
                assert_eq!(*insertion, [3.0, 4.0, 0.0]);
                assert_eq!(*height, 1.5);
                assert_eq!(value, "World");
                assert_eq!(style_name, "Standard");
                assert!((*rotation - 45.0).abs() < 1e-9);
                assert!((*width_factor - 0.8).abs() < 1e-9);
                assert!((*oblique_angle - 12.0).abs() < 1e-9);
                assert_eq!(*horizontal_alignment, 2);
                assert_eq!(*vertical_alignment, 3);
                assert_eq!(*alignment_point, Some([6.0, 7.0, 0.0]));
            }
            other => panic!("expected native text, got {other:?}"),
        }
    }

    #[test]
    fn spline_entity_bridge_roundtrips_basic_geometry() {
        let mut native = nm::Entity::new(nm::EntityData::Spline {
            degree: 3,
            closed: true,
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                [0.0, 0.0, 0.0],
                [1.0, 2.0, 0.0],
                [2.0, 2.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            weights: vec![1.0, 0.8, 0.8, 1.0],
            fit_points: vec![[0.5, 1.0, 0.0], [2.5, 1.0, 0.0]],
            start_tangent: [1.0, 0.0, 0.0],
            end_tangent: [1.0, 0.0, 0.0],
        });
        native.handle = nm::Handle::new(0x21);
        native.layer_name = "SPL".into();

        let acad = native_entity_to_acadrust(&native).expect("native spline should bridge to acad");
        let roundtrip = acadrust_entity_to_native(&acad).expect("acad spline should bridge to native");

        match roundtrip.data {
            nm::EntityData::Spline {
                degree,
                closed,
                knots,
                control_points,
                weights,
                fit_points,
                ..
            } => {
                assert_eq!(degree, 3);
                assert!(closed);
                assert_eq!(knots, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
                assert_eq!(control_points.len(), 4);
                assert_eq!(weights, vec![1.0, 0.8, 0.8, 1.0]);
                assert_eq!(fit_points.len(), 2);
            }
            other => panic!("expected native spline, got {other:?}"),
        }
    }

    #[test]
    fn dimension_entity_bridge_roundtrips_linear_geometry() {
        let mut native = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: "*D1".into(),
            style_name: "Standard".into(),
            definition_point: [4.0, 5.0, 0.0],
            text_midpoint: [2.0, 3.0, 0.0],
            text_override: "<>".into(),
            attachment_point: 0,
            measurement: 12.5,
            text_rotation: 15.0,
            horizontal_direction: 0.0,
            flip_arrow1: false,
            flip_arrow2: true,
            first_point: [0.0, 0.0, 0.0],
            second_point: [10.0, 0.0, 0.0],
            angle_vertex: [0.0, 0.0, 0.0],
            dimension_arc: [0.0, 0.0, 0.0],
            leader_length: 0.0,
            rotation: 25.0,
            ext_line_rotation: 35.0,
        });
        native.handle = nm::Handle::new(0x31);
        native.layer_name = "DIM".into();

        let acad =
            native_entity_to_acadrust(&native).expect("native dimension should bridge to acad");
        let roundtrip =
            acadrust_entity_to_native(&acad).expect("acad dimension should bridge to native");

        match roundtrip.data {
            nm::EntityData::Dimension {
                dim_type,
                style_name,
                definition_point,
                text_midpoint,
                first_point,
                second_point,
                measurement,
                rotation,
                ext_line_rotation,
                ..
            } => {
                assert_eq!(dim_type, 0);
                assert_eq!(style_name, "Standard");
                assert_eq!(definition_point, [4.0, 5.0, 0.0]);
                assert_eq!(text_midpoint, [2.0, 3.0, 0.0]);
                assert_eq!(first_point, [0.0, 0.0, 0.0]);
                assert_eq!(second_point, [10.0, 0.0, 0.0]);
                assert!((measurement - 12.5).abs() < 1e-9);
                assert!((rotation - 25.0).abs() < 1e-9);
                assert!((ext_line_rotation - 35.0).abs() < 1e-9);
            }
            other => panic!("expected native dimension, got {other:?}"),
        }
    }

    #[test]
    fn multileader_entity_bridge_roundtrips_text_location_and_vertices() {
        let mut native = nm::Entity::new(nm::EntityData::MultiLeader {
            content_type: 1,
            text_label: "TAG".into(),
            style_name: "Standard".into(),
            arrowhead_size: 2.5,
            landing_gap: 0.5,
            dogleg_length: 3.0,
            property_override_flags: 0,
            path_type: 1,
            line_color: 256,
            leader_line_weight: -1,
            enable_landing: true,
            enable_dogleg: true,
            enable_annotation_scale: false,
            scale_factor: 1.0,
            text_attachment_direction: 0,
            text_bottom_attachment_type: 9,
            text_top_attachment_type: 9,
            text_location: Some([6.0, 0.0, 4.0]),
            leader_vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 1.0], [6.0, 0.0, 4.0]],
            leader_root_lengths: vec![3],
        });
        native.handle = nm::Handle::new(0x41);
        native.layer_name = "MLEADER".into();

        let acad =
            native_entity_to_acadrust(&native).expect("native multileader should bridge to acad");
        let roundtrip =
            acadrust_entity_to_native(&acad).expect("acad multileader should bridge to native");

        match roundtrip.data {
            nm::EntityData::MultiLeader {
                content_type,
                text_label,
                style_name,
                text_location,
                leader_vertices,
                leader_root_lengths,
                ..
            } => {
                assert_eq!(content_type, 1);
                assert_eq!(text_label, "TAG");
                assert_eq!(style_name, "Standard");
                assert_eq!(text_location, Some([6.0, 0.0, 4.0]));
                assert_eq!(
                    leader_vertices,
                    vec![[0.0, 0.0, 0.0], [2.0, 0.0, 1.0], [6.0, 0.0, 4.0]]
                );
                assert_eq!(leader_root_lengths, vec![3]);
            }
            other => panic!("expected native multileader, got {other:?}"),
        }
    }

    #[test]
    fn ellipse_entity_bridge_roundtrips_geometry() {
        let mut native = nm::Entity::new(nm::EntityData::Ellipse {
            center: [1.0, 2.0, 3.0],
            major_axis: [4.0, 5.0, 0.0],
            ratio: 0.375,
            start_param: 0.25,
            end_param: 5.75,
        });
        native.handle = nm::Handle::new(0x51);
        native.layer_name = "ELLIPSE".into();

        let acad = native_entity_to_acadrust(&native).expect("native ellipse should bridge to acad");
        let roundtrip = acadrust_entity_to_native(&acad).expect("acad ellipse should bridge to native");

        match roundtrip.data {
            nm::EntityData::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                assert_eq!(center, [1.0, 2.0, 3.0]);
                assert_eq!(major_axis, [4.0, 5.0, 0.0]);
                assert!((ratio - 0.375).abs() < 1e-9);
                assert!((start_param - 0.25).abs() < 1e-9);
                assert!((end_param - 5.75).abs() < 1e-9);
            }
            other => panic!("expected native ellipse, got {other:?}"),
        }
    }

    #[test]
    fn direct_geometry_bridge_roundtrips_payload_fields() {
        let cases = vec![
            nm::Entity::new(nm::EntityData::Face3D {
                corners: [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                ],
                invisible_edges: 0,
            }),
            nm::Entity::new(nm::EntityData::Solid {
                corners: [
                    [2.0, 0.0, 0.0],
                    [3.0, 0.0, 0.0],
                    [3.0, 1.0, 0.0],
                    [2.0, 1.0, 0.0],
                ],
                normal: [0.0, 0.0, 1.0],
                thickness: 0.0,
            }),
            nm::Entity::new(nm::EntityData::Ray {
                origin: [4.0, 5.0, 6.0],
                direction: [0.0, 1.0, 0.0],
            }),
            nm::Entity::new(nm::EntityData::XLine {
                origin: [7.0, 8.0, 9.0],
                direction: [1.0, 0.0, 0.0],
            }),
            nm::Entity::new(nm::EntityData::Shape {
                insertion: [10.0, 11.0, 0.0],
                size: 2.5,
                shape_number: 7,
                name: String::new(),
                rotation: 0.0,
                relative_x_scale: 1.0,
                oblique_angle: 0.0,
                style_name: String::new(),
                normal: [0.0, 0.0, 1.0],
                thickness: 0.0,
            }),
        ];

        for native in cases {
            let expected = native.data.clone();
            let acad = native_entity_to_acadrust(&native)
                .unwrap_or_else(|| panic!("{} should bridge to acad", expected.type_name()));
            let roundtrip = acadrust_entity_to_native(&acad)
                .unwrap_or_else(|| panic!("{} should bridge to native", expected.type_name()));
            assert_eq!(
                roundtrip.data,
                expected,
                "{} payload should survive roundtrip",
                expected.type_name()
            );
        }
    }

    #[test]
    fn multileader_entity_bridge_preserves_multiple_root_lengths() {
        let mut ml = ar::MultiLeader::with_text(
            "TAG",
            Vector3::new(6.0, 0.0, 4.0),
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(6.0, 0.0, 4.0)],
        );
        {
            let root = ml.context.add_leader_root();
            root.create_line(vec![
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(6.0, 0.0, 4.0),
            ]);
        }

        let entity = ar::EntityType::MultiLeader(ml);
        let native = acadrust_entity_to_native(&entity).expect("multileader should bridge to native");
        let roundtrip =
            native_entity_to_acadrust(&native).expect("native multileader should bridge to acad");

        match native.data {
            nm::EntityData::MultiLeader { leader_root_lengths, .. } => {
                assert_eq!(leader_root_lengths, vec![2, 2]);
            }
            other => panic!("expected native multileader, got {other:?}"),
        }

        match roundtrip {
            ar::EntityType::MultiLeader(ml) => assert_eq!(ml.context.leader_roots.len(), 2),
            other => panic!("expected compat multileader, got {other:?}"),
        }
    }

    #[test]
    fn polyline_family_bridge_roundtrips_preserve_family_and_vertex_fields() {
        let cases = vec![
            (
                nm::EntityData::Polyline {
                    polyline_type: nm::PolylineType::Polyline2D,
                    vertices: vec![
                        nm::PolylineVertex {
                            position: [0.0, 0.0, 0.0],
                            bulge: 0.25,
                            start_width: 1.5,
                            end_width: 2.5,
                        },
                        nm::PolylineVertex {
                            position: [4.0, 1.0, 0.0],
                            bulge: -0.5,
                            start_width: 0.5,
                            end_width: 0.75,
                        },
                    ],
                    closed: true,
                },
                "Polyline2D",
            ),
            (
                nm::EntityData::Polyline {
                    polyline_type: nm::PolylineType::Polyline3D,
                    vertices: vec![
                        nm::PolylineVertex {
                            position: [1.0, 2.0, 3.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [4.0, 5.0, 6.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                    ],
                    closed: false,
                },
                "Polyline3D",
            ),
            (
                nm::EntityData::Polyline {
                    polyline_type: nm::PolylineType::PolygonMesh,
                    vertices: vec![
                        nm::PolylineVertex {
                            position: [0.0, 0.0, 0.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [1.0, 0.0, 1.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [1.0, 1.0, 2.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [0.0, 1.0, 3.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [2.0, 0.0, 4.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                        nm::PolylineVertex {
                            position: [2.0, 1.0, 5.0],
                            bulge: 0.0,
                            start_width: 0.0,
                            end_width: 0.0,
                        },
                    ],
                    closed: true,
                },
                "PolygonMesh",
            ),
            (
                nm::EntityData::Polyline {
                    polyline_type: nm::PolylineType::PolyfaceMesh,
                    vertices: vec![
                        nm::PolylineVertex {
                            position: [2.0, 0.0, 0.0],
                            bulge: 0.0,
                            start_width: 3.0,
                            end_width: 4.0,
                        },
                        nm::PolylineVertex {
                            position: [3.0, 1.0, 0.0],
                            bulge: 0.0,
                            start_width: 5.0,
                            end_width: 6.0,
                        },
                    ],
                    closed: true,
                },
                "PolyfaceMesh",
            ),
        ];

        for (data, family_name) in cases {
            let mut native = nm::Entity::new(data.clone());
            native.handle = nm::Handle::new(0x60);
            native.layer_name = family_name.into();

            let acad = native_entity_to_acadrust(&native)
                .unwrap_or_else(|| panic!("{family_name} should bridge to acad"));
            match (&acad, &data) {
                (
                    ar::EntityType::Polyline2D(pline),
                    nm::EntityData::Polyline {
                        vertices, closed, ..
                    },
                ) => {
                    assert_eq!(pline.vertices.len(), vertices.len());
                    assert_eq!(pline.is_closed(), *closed);
                    assert_eq!(pline.vertices[0].start_width, vertices[0].start_width);
                    assert_eq!(pline.vertices[0].end_width, vertices[0].end_width);
                    assert_eq!(pline.vertices[0].bulge, vertices[0].bulge);
                }
                (
                    ar::EntityType::Polyline3D(pline),
                    nm::EntityData::Polyline {
                        vertices, closed, ..
                    },
                ) => {
                    assert_eq!(pline.vertices.len(), vertices.len());
                    assert_eq!(pline.is_closed(), *closed);
                }
                (
                    ar::EntityType::PolygonMesh(mesh),
                    nm::EntityData::Polyline {
                        vertices, closed, ..
                    },
                ) => {
                    assert_eq!(mesh.vertices.len(), vertices.len());
                    assert_eq!(mesh.is_closed_m(), *closed);
                    assert_eq!(mesh.m_vertex_count, 2);
                    assert_eq!(mesh.n_vertex_count, 3);
                    assert_eq!(mesh.vertices[4].location, Vector3::new(2.0, 0.0, 4.0));
                }
                (
                    ar::EntityType::PolyfaceMesh(mesh),
                    nm::EntityData::Polyline {
                        vertices, closed, ..
                    },
                ) => {
                    assert_eq!(mesh.vertices.len(), vertices.len());
                    assert_eq!(mesh.flags.contains(ar::PolyfaceMeshFlags::CLOSED), *closed);
                    assert_eq!(mesh.vertices[0].start_width, vertices[0].start_width);
                    assert_eq!(mesh.vertices[0].end_width, vertices[0].end_width);
                    assert!(mesh.faces.is_empty(), "flat polyface bridge should not invent faces");
                }
                other => panic!("unexpected compat entity for {family_name}: {other:?}"),
            }

            let roundtrip = acadrust_entity_to_native(&acad)
                .unwrap_or_else(|| panic!("{family_name} should bridge back to native"));
            assert_eq!(roundtrip.data, data, "{family_name} payload should survive roundtrip");
        }
    }

    #[test]
    fn direct_geometry_bridge_preserves_scrutiny_payload_fidelity() {
        let mut face = ar::Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
        );
        let mut invisible_edges = ar::InvisibleEdgeFlags::new();
        invisible_edges.set_first_invisible(true);
        invisible_edges.set_third_invisible(true);
        face.invisible_edges = invisible_edges;
        let face = ar::EntityType::Face3D(face);
        let face_roundtrip =
            native_entity_to_acadrust(&acadrust_entity_to_native(&face).expect("face to native"))
                .expect("face back to acad");
        match face_roundtrip {
            ar::EntityType::Face3D(face) => {
                assert!(face.invisible_edges.is_first_invisible());
                assert!(!face.invisible_edges.is_second_invisible());
                assert!(face.invisible_edges.is_third_invisible());
                assert!(!face.invisible_edges.is_fourth_invisible());
            }
            other => panic!("expected compat face3d, got {other:?}"),
        }

        let mut solid = ar::Solid::new(
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(3.0, 0.0, 0.0),
            Vector3::new(3.0, 1.0, 0.0),
            Vector3::new(2.0, 1.0, 0.0),
        );
        solid.thickness = 2.5;
        solid.normal = Vector3::new(0.0, 0.6, 1.0);
        let solid = ar::EntityType::Solid(solid);
        let solid_roundtrip = native_entity_to_acadrust(
            &acadrust_entity_to_native(&solid).expect("solid to native"),
        )
        .expect("solid back to acad");
        match solid_roundtrip {
            ar::EntityType::Solid(solid) => {
                assert!((solid.thickness - 2.5).abs() < 1e-9);
                assert_eq!(solid.normal, Vector3::new(0.0, 0.6, 1.0));
            }
            other => panic!("expected compat solid, got {other:?}"),
        }

        let mut shape = ar::Shape::default();
        shape.insertion_point = Vector3::new(10.0, 11.0, 0.0);
        shape.size = 2.5;
        shape.shape_number = 7;
        shape.shape_name = "DIP8".into();
        shape.rotation = 30.0_f64.to_radians();
        shape.relative_x_scale = 1.75;
        shape.oblique_angle = 12.0_f64.to_radians();
        shape.style_name = "Symbols".into();
        shape.thickness = 1.25;
        shape.normal = Vector3::new(0.0, 1.0, 0.0);
        let shape = ar::EntityType::Shape(shape);
        let shape_roundtrip = native_entity_to_acadrust(
            &acadrust_entity_to_native(&shape).expect("shape to native"),
        )
        .expect("shape back to acad");
        match shape_roundtrip {
            ar::EntityType::Shape(shape) => {
                assert_eq!(shape.shape_name, "DIP8");
                assert!((shape.rotation.to_degrees() - 30.0).abs() < 1e-9);
                assert!((shape.relative_x_scale - 1.75).abs() < 1e-9);
                assert!((shape.oblique_angle.to_degrees() - 12.0).abs() < 1e-9);
                assert_eq!(shape.style_name, "Symbols");
                assert_eq!(shape.normal, Vector3::new(0.0, 1.0, 0.0));
                assert!((shape.thickness - 1.25).abs() < 1e-9);
            }
            other => panic!("expected compat shape, got {other:?}"),
        }
    }

    #[test]
    fn direct_geometry_bridge_rejects_out_of_range_face_edge_bits() {
        let mut face = ar::Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );
        face.invisible_edges = ar::InvisibleEdgeFlags::from_bits(0b1_0000);

        let native = acadrust_entity_to_native(&ar::EntityType::Face3D(face))
            .expect("face should bridge to native");
        match native.data {
            nm::EntityData::Face3D {
                invisible_edges, ..
            } => assert_eq!(invisible_edges, 0),
            other => panic!("expected native face3d, got {other:?}"),
        }
    }

    #[test]
    fn hatch_bridge_roundtrips_preserve_metadata_and_boundary_structure() {
        let mut native = nm::Entity::new(nm::EntityData::Hatch {
            pattern_name: "ANSI31".into(),
            solid_fill: false,
            boundary_paths: vec![
                nm::HatchBoundaryPath {
                    flags: 3,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![[0.0, 0.0, 0.25], [5.0, 0.0, -0.5], [5.0, 4.0, 0.0]],
                    }],
                },
                nm::HatchBoundaryPath {
                    flags: 1,
                    edges: vec![
                        nm::HatchEdge::Line {
                            start: [1.0, 1.0],
                            end: [4.0, 1.0],
                        },
                        nm::HatchEdge::CircularArc {
                            center: [4.0, 2.0],
                            radius: 1.0,
                            start_angle: 0.0,
                            end_angle: 90.0,
                            is_ccw: true,
                        },
                    ],
                },
            ],
        });
        native.handle = nm::Handle::new(0x70);

        let acad = native_entity_to_acadrust(&native).expect("hatch should bridge to acad");
        match &acad {
            ar::EntityType::Hatch(hatch) => {
                assert_eq!(hatch.pattern.name, "ANSI31");
                assert!(!hatch.is_solid);
                assert_eq!(hatch.paths.len(), 2);
                assert!(hatch.paths[0].is_polyline());
                assert_eq!(hatch.paths[0].edges.len(), 1);
                assert_eq!(hatch.paths[1].edges.len(), 2);
            }
            other => panic!("expected compat hatch, got {other:?}"),
        }

        let roundtrip = acadrust_entity_to_native(&acad).expect("hatch should bridge back to native");
        match &roundtrip.data {
            nm::EntityData::Hatch {
                pattern_name,
                solid_fill,
                boundary_paths,
            } => {
                assert_eq!(pattern_name, "ANSI31");
                assert!(!solid_fill);
                assert_eq!(boundary_paths.len(), 2);
                assert!(matches!(boundary_paths[0].edges[0], nm::HatchEdge::Polyline { .. }));
                assert_eq!(boundary_paths[1].edges.len(), 2);
            }
            other => panic!("expected native hatch, got {other:?}"),
        }
    }

    #[test]
    fn hatch_bridge_back_safely_accounts_for_spline_boundary_edges() {
        let mut spline = ar::SplineEdge {
            degree: 3,
            rational: true,
            periodic: false,
            knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            control_points: vec![
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(2.0, 3.0, 0.5),
                Vector3::new(4.0, 3.0, 1.5),
                Vector3::new(6.0, 0.0, 1.0),
            ],
            fit_points: vec![
                Vector2::new(0.0, 0.0),
                Vector2::new(3.0, 2.0),
                Vector2::new(6.0, 0.0),
            ],
            start_tangent: Vector2::new(1.0, 0.0),
            end_tangent: Vector2::new(0.0, -1.0),
        };
        let mut path = ar::BoundaryPath::with_flags(ar::BoundaryPathFlags::from_bits(5));
        path.add_edge(ar::BoundaryEdge::Spline(spline.clone()));
        let mut hatch = ar::Hatch::new();
        hatch.pattern.name = "ANSI31".into();
        hatch.paths.push(path);

        let native =
            acadrust_entity_to_native(&ar::EntityType::Hatch(hatch)).expect("hatch should bridge");

        match native.data {
            nm::EntityData::Hatch { boundary_paths, .. } => {
                assert_eq!(boundary_paths.len(), 1);
                assert_eq!(boundary_paths[0].flags, 5);
                assert!(
                    boundary_paths[0].edges.is_empty(),
                    "unsupported compat spline edges should be skipped instead of panicking"
                );
            }
            other => panic!("expected native hatch, got {other:?}"),
        }

        spline.fit_points.push(Vector2::new(7.0, -1.0));
        assert_eq!(spline.fit_points.len(), 4);
    }

    #[test]
    fn document_bridge_exposes_polyline_families_and_hatch_on_compat_side() {
        let mut native = nm::CadDocument::new();
        for data in [
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline2D,
                vertices: vec![nm::PolylineVertex {
                    position: [0.0, 0.0, 0.0],
                    bulge: 0.0,
                    start_width: 1.0,
                    end_width: 1.5,
                }],
                closed: true,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline3D,
                vertices: vec![nm::PolylineVertex {
                    position: [1.0, 2.0, 3.0],
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                }],
                closed: false,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolygonMesh,
                vertices: vec![nm::PolylineVertex {
                    position: [2.0, 3.0, 4.0],
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                }],
                closed: true,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolyfaceMesh,
                vertices: vec![nm::PolylineVertex {
                    position: [3.0, 4.0, 5.0],
                    bulge: 0.0,
                    start_width: 2.0,
                    end_width: 2.5,
                }],
                closed: true,
            },
            nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
                    }],
                }],
            },
        ] {
            native.add_entity(nm::Entity::new(data)).expect("entity should add");
        }

        let compat = native_doc_to_acadrust(&native);
        let mut saw_polyline2d = 0;
        let mut saw_polyline3d = 0;
        let mut saw_polygon_mesh = 0;
        let mut saw_polyface_mesh = 0;
        let mut saw_hatch = 0;

        for entity in compat.entities() {
            match entity {
                ar::EntityType::Polyline2D(_) => saw_polyline2d += 1,
                ar::EntityType::Polyline3D(_) => saw_polyline3d += 1,
                ar::EntityType::PolygonMesh(_) => saw_polygon_mesh += 1,
                ar::EntityType::PolyfaceMesh(_) => saw_polyface_mesh += 1,
                ar::EntityType::Hatch(_) => saw_hatch += 1,
                _ => {}
            }
        }

        assert_eq!(saw_polyline2d, 1);
        assert_eq!(saw_polyline3d, 1);
        assert_eq!(saw_polygon_mesh, 1);
        assert_eq!(saw_polyface_mesh, 1);
        assert_eq!(saw_hatch, 1);
    }

    #[test]
    fn annotation_bridge_roundtrips_leader_mline_and_tolerance_payloads() {
        let mut leader = nm::Entity::new(nm::EntityData::Leader {
            vertices: vec![[0.0, 0.0, 0.0], [1.5, 2.5, 0.0], [4.0, 2.5, 0.0]],
            has_arrowhead: false,
        });
        leader.handle = nm::Handle::new(0x80);
        leader.layer_name = "ANNO".into();

        let mut mline = nm::Entity::new(nm::EntityData::MLine {
            vertices: vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 2.0, 0.0]],
            style_name: "BATT".into(),
            scale: 2.25,
            closed: false,
        });
        mline.handle = nm::Handle::new(0x81);

        let mut tolerance = nm::Entity::new(nm::EntityData::Tolerance {
            text: "{\\Fgdt;p}%%v0.25%%vA^JB".into(),
            insertion: [8.0, 1.5, 0.0],
        });
        tolerance.handle = nm::Handle::new(0x82);

        for native in [leader, mline, tolerance] {
            let expected = native.data.clone();
            let acad = native_entity_to_acadrust(&native)
                .unwrap_or_else(|| panic!("{} should bridge to acad", expected.type_name()));
            let roundtrip = acadrust_entity_to_native(&acad)
                .unwrap_or_else(|| panic!("{} should bridge to native", expected.type_name()));
            assert_eq!(roundtrip.data, expected, "{} payload should survive roundtrip", expected.type_name());
        }
    }

    #[test]
    fn payload_bridge_roundtrips_text_opaque_and_acis_entities() {
        let cases = vec![
            nm::Entity::new(nm::EntityData::AttDef {
                tag: "PARTNO".into(),
                prompt: "Enter part number".into(),
                default_value: "PN-42".into(),
                insertion: [1.0, 2.0, 0.0],
                height: 2.5,
            }),
            nm::Entity::new(nm::EntityData::Attrib {
                tag: "SERIAL".into(),
                value: "A-001".into(),
                insertion: [2.0, 3.0, 0.0],
                height: 1.75,
            }),
            nm::Entity::new(nm::EntityData::PdfUnderlay {
                insertion: [4.0, 5.0, 0.0],
                scale: [1.5, 0.75, 1.0],
            }),
            nm::Entity::new(nm::EntityData::Unknown {
                entity_type: "ACAD_PROXY_ENTITY".into(),
            }),
            nm::Entity::new(nm::EntityData::Solid3D {
                acis_data: "body\nline-two\n".into(),
            }),
            nm::Entity::new(nm::EntityData::Region {
                acis_data: "region-body\nedge-two\n".into(),
            }),
        ];

        for native in cases {
            let expected = native.data.clone();
            let acad = native_entity_to_acadrust(&native)
                .unwrap_or_else(|| panic!("{} should bridge to acad", expected.type_name()));
            let roundtrip = acadrust_entity_to_native(&acad)
                .unwrap_or_else(|| panic!("{} should bridge to native", expected.type_name()));
            assert_eq!(roundtrip.data, expected, "{} payload should survive roundtrip", expected.type_name());
        }
    }

    fn representative_entity_with_common_fields(
        data: nm::EntityData,
        handle: u64,
        owner: u64,
    ) -> nm::Entity {
        let mut entity = nm::Entity::new(data);
        entity.handle = nm::Handle::new(handle);
        entity.owner_handle = nm::Handle::new(owner);
        entity.layer_name = "BridgeLayer".into();
        entity.linetype_name = "DASHED".into();
        entity.color_index = 5;
        entity.true_color = 0x12_34_56;
        entity.lineweight = 35;
        entity.invisible = true;
        entity.transparency = 77;
        entity.thickness = 2.25;
        entity.extrusion = [0.0, 0.5, 1.0];
        entity
    }

    fn assert_common_fields_preserved(entity: &nm::Entity, handle: u64, owner: u64) {
        assert_eq!(entity.handle, nm::Handle::new(handle));
        assert_eq!(entity.owner_handle, nm::Handle::new(owner));
        assert_eq!(entity.layer_name, "BridgeLayer");
        assert_eq!(entity.linetype_name, "DASHED");
        assert_eq!(entity.color_index, 256);
        assert_eq!(entity.true_color, 0x12_34_56);
        assert_eq!(entity.lineweight, 35);
        assert!(entity.invisible);
        assert_eq!(entity.transparency, 77);
    }

    #[test]
    fn bridge_roundtrips_preserve_common_fields_for_representative_families() {
        let cases = vec![
            representative_entity_with_common_fields(
                nm::EntityData::Ellipse {
                    center: [1.0, 2.0, 0.0],
                    major_axis: [4.0, 0.0, 0.0],
                    ratio: 0.5,
                    start_param: 0.0,
                    end_param: 3.14,
                },
                0x91,
                0x191,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Polyline {
                    polyline_type: nm::PolylineType::Polyline2D,
                    vertices: vec![nm::PolylineVertex {
                        position: [0.0, 0.0, 0.0],
                        bulge: 0.25,
                        start_width: 1.0,
                        end_width: 2.0,
                    }],
                    closed: true,
                },
                0x92,
                0x192,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Hatch {
                    pattern_name: "ANSI31".into(),
                    solid_fill: false,
                    boundary_paths: vec![nm::HatchBoundaryPath {
                        flags: 2,
                        edges: vec![nm::HatchEdge::Polyline {
                            closed: true,
                            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.5], [1.0, 1.0, 0.0]],
                        }],
                    }],
                },
                0x93,
                0x193,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Leader {
                    vertices: vec![[0.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
                    has_arrowhead: true,
                },
                0x94,
                0x194,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::MLine {
                    vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                    style_name: "BATT".into(),
                    scale: 1.5,
                    closed: false,
                },
                0x95,
                0x195,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Tolerance {
                    text: "%%v0.25".into(),
                    insertion: [3.0, 2.0, 0.0],
                },
                0x96,
                0x196,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Image {
                    insertion: [10.0, 20.0, 0.0],
                    u_vector: [0.5, 0.0, 0.0],
                    v_vector: [0.0, 0.25, 0.0],
                    image_size: [640.0, 480.0],
                },
                0x97,
                0x197,
            ),
            representative_entity_with_common_fields(
                nm::EntityData::Wipeout {
                    clip_vertices: vec![[0.0, 0.0], [4.0, 0.0], [3.0, 2.0], [0.0, 3.0]],
                    elevation: 0.0,
                },
                0x98,
                0x198,
            ),
        ];

        for native in cases {
            let handle = native.handle.value();
            let owner = native.owner_handle.value();
            let kind = native.data.type_name().to_string();
            let compat = native_entity_to_acadrust(&native)
                .unwrap_or_else(|| panic!("{kind} should bridge to acad"));
            let roundtrip = acadrust_entity_to_native(&compat)
                .unwrap_or_else(|| panic!("{kind} should bridge back to native"));

            assert_common_fields_preserved(&roundtrip, handle, owner);
        }
    }

    #[test]
    fn document_bridge_keeps_prioritized_entity_counts_visible_on_compat_side() {
        let mut native = nm::CadDocument::new();
        let prioritized = vec![
            nm::EntityData::Ellipse {
                center: [1.0, 2.0, 0.0],
                major_axis: [4.0, 0.0, 0.0],
                ratio: 0.25,
                start_param: 0.1,
                end_param: 2.9,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline2D,
                vertices: vec![nm::PolylineVertex {
                    position: [0.0, 0.0, 0.0],
                    bulge: 0.5,
                    start_width: 0.5,
                    end_width: 1.0,
                }],
                closed: true,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::Polyline3D,
                vertices: vec![nm::PolylineVertex {
                    position: [0.0, 0.0, 1.0],
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                }],
                closed: false,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolygonMesh,
                vertices: vec![nm::PolylineVertex {
                    position: [1.0, 0.0, 1.0],
                    bulge: 0.0,
                    start_width: 0.0,
                    end_width: 0.0,
                }],
                closed: true,
            },
            nm::EntityData::Polyline {
                polyline_type: nm::PolylineType::PolyfaceMesh,
                vertices: vec![nm::PolylineVertex {
                    position: [2.0, 0.0, 1.0],
                    bulge: 0.0,
                    start_width: 2.0,
                    end_width: 2.5,
                }],
                closed: true,
            },
            nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0]],
                    }],
                }],
            },
            nm::EntityData::Leader {
                vertices: vec![[0.0, 0.0, 0.0], [1.5, 2.5, 0.0], [4.0, 2.5, 0.0]],
                has_arrowhead: false,
            },
            nm::EntityData::MLine {
                vertices: vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 2.0, 0.0]],
                style_name: "BATT".into(),
                scale: 2.25,
                closed: false,
            },
            nm::EntityData::Image {
                insertion: [10.0, 20.0, 0.0],
                u_vector: [0.5, 0.0, 0.0],
                v_vector: [0.0, 0.25, 0.0],
                image_size: [640.0, 480.0],
            },
            nm::EntityData::Wipeout {
                clip_vertices: vec![[0.0, 0.0], [4.0, 0.0], [3.0, 2.0], [0.0, 3.0]],
                elevation: 0.0,
            },
            nm::EntityData::Tolerance {
                text: "{\\Fgdt;p}%%v0.25%%vA^JB".into(),
                insertion: [8.0, 1.5, 0.0],
            },
        ];

        for data in prioritized {
            native.add_entity(nm::Entity::new(data)).expect("entity should add");
        }

        let compat = native_doc_to_acadrust(&native);
        let mut saw_ellipse = 0;
        let mut saw_polyline2d = 0;
        let mut saw_polyline3d = 0;
        let mut saw_polygon_mesh = 0;
        let mut saw_polyface_mesh = 0;
        let mut saw_hatch = 0;
        let mut saw_leader = 0;
        let mut saw_mline = 0;
        let mut saw_image = 0;
        let mut saw_wipeout = 0;
        let mut saw_tolerance = 0;

        for entity in compat.entities() {
            match entity {
                ar::EntityType::Ellipse(_) => saw_ellipse += 1,
                ar::EntityType::Polyline2D(_) => saw_polyline2d += 1,
                ar::EntityType::Polyline3D(_) => saw_polyline3d += 1,
                ar::EntityType::PolygonMesh(_) => saw_polygon_mesh += 1,
                ar::EntityType::PolyfaceMesh(_) => saw_polyface_mesh += 1,
                ar::EntityType::Hatch(_) => saw_hatch += 1,
                ar::EntityType::Leader(_) => saw_leader += 1,
                ar::EntityType::MLine(_) => saw_mline += 1,
                ar::EntityType::RasterImage(_) => saw_image += 1,
                ar::EntityType::Wipeout(_) => saw_wipeout += 1,
                ar::EntityType::Tolerance(_) => saw_tolerance += 1,
                _ => {}
            }
        }

        assert_eq!(saw_ellipse, 1);
        assert_eq!(saw_polyline2d, 1);
        assert_eq!(saw_polyline3d, 1);
        assert_eq!(saw_polygon_mesh, 1);
        assert_eq!(saw_polyface_mesh, 1);
        assert_eq!(saw_hatch, 1);
        assert_eq!(saw_leader, 1);
        assert_eq!(saw_mline, 1);
        assert_eq!(saw_image, 1);
        assert_eq!(saw_wipeout, 1);
        assert_eq!(saw_tolerance, 1);
    }

    #[test]
    fn image_and_wipeout_bridge_survive_document_roundtrip_with_geometry() {
        let mut native = nm::CadDocument::new();
        let image_data = nm::EntityData::Image {
            insertion: [10.0, 20.0, 0.0],
            u_vector: [0.5, 0.0, 0.0],
            v_vector: [0.0, 0.25, 0.0],
            image_size: [640.0, 480.0],
        };
        let wipeout_data = nm::EntityData::Wipeout {
            clip_vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.75, 2.0 / 3.0], [0.0, 1.0]],
            elevation: 0.0,
        };
        native
            .add_entity(nm::Entity::new(image_data.clone()))
            .expect("image should add");
        native
            .add_entity(nm::Entity::new(wipeout_data.clone()))
            .expect("wipeout should add");

        let compat = native_doc_to_acadrust(&native);
        let mut saw_image = false;
        let mut saw_wipeout = false;
        let mut image_roundtrip = None;
        let mut wipeout_roundtrip = None;

        for entity in compat.entities() {
            match entity {
                ar::EntityType::RasterImage(_) => {
                    saw_image = true;
                    image_roundtrip = Some(
                        acadrust_entity_to_native(entity).expect("compat image should bridge back"),
                    );
                }
                ar::EntityType::Wipeout(_) => {
                    saw_wipeout = true;
                    wipeout_roundtrip = Some(
                        acadrust_entity_to_native(entity)
                            .expect("compat wipeout should bridge back"),
                    );
                }
                _ => {}
            }
        }

        assert!(saw_image, "image should be exposed on compat side");
        assert!(saw_wipeout, "wipeout should be exposed on compat side");
        assert_eq!(image_roundtrip.expect("image roundtrip").data, image_data);
        assert_eq!(wipeout_roundtrip.expect("wipeout roundtrip").data, wipeout_data);
        let compat_wipeout = compat
            .entities()
            .find_map(|entity| match entity {
                ar::EntityType::Wipeout(wipeout) => Some(wipeout),
                _ => None,
            })
            .expect("compat wipeout should exist");
        assert_eq!(
            compat_wipeout.clip_boundary_vertices,
            vec![
                Vector2::new(0.0, 0.0),
                Vector2::new(1.0, 0.0),
                Vector2::new(0.75, 2.0 / 3.0),
                Vector2::new(0.0, 1.0),
            ]
        );
    }

    #[test]
    fn wipeout_bridge_preserves_local_clip_vertices_on_roundtrip() {
        let local_clip_vertices = vec![[0.0, 0.0], [1.0, 0.0], [0.75, 2.0 / 3.0], [0.0, 1.0]];
        let native_entity = representative_entity_with_common_fields(
            nm::EntityData::Wipeout {
                clip_vertices: local_clip_vertices.clone(),
                elevation: 0.0,
            },
            0x99,
            0x199,
        );

        let compat = native_entity_to_acadrust(&native_entity).expect("wipeout should bridge out");
        let compat_wipeout = match &compat {
            ar::EntityType::Wipeout(wipeout) => wipeout,
            other => panic!("expected wipeout, got {other:?}"),
        };

        let expected_local_vertices = vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(1.0, 0.0),
            Vector2::new(0.75, 2.0 / 3.0),
            Vector2::new(0.0, 1.0),
        ];
        assert_eq!(
            compat_wipeout.clip_boundary_vertices,
            expected_local_vertices,
            "compat wipeout should retain local normalized clip vertices",
        );

        let world_vertices: Vec<[f64; 2]> = compat_wipeout
            .world_boundary_vertices()
            .iter()
            .map(|vertex| [vertex.x, vertex.y])
            .collect();
        assert_eq!(
            world_vertices,
            vec![[0.0, 0.0], [1.0, 0.0], [0.75, 2.0 / 3.0], [0.0, 1.0]],
            "world vertices should reflect the same visible wipeout polygon",
        );

        let roundtrip = acadrust_entity_to_native(&compat).expect("wipeout should bridge back");
        assert_eq!(
            roundtrip.data,
            nm::EntityData::Wipeout {
                clip_vertices: local_clip_vertices,
                elevation: 0.0,
            }
        );
    }
}
