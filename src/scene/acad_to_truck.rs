// acadrust -> truck topology conversion layer.

use acadrust::{CadDocument, EntityType};
use h7cad_native_model as nm;
use glam::Vec3;
use truck_modeling::{Edge, Solid, Vertex, Wire};

use crate::entities::{arc, circle, line, lwpolyline, mtext, point, text};
use crate::entities::traits::EntityTypeOps;
use crate::scene::wire_model::{SnapHint, TangentGeom};

#[allow(dead_code)]
pub enum TruckObject {
    Point(Vertex),
    Curve(Edge),
    Contour(Wire),
    Text(Vec<Vec<[f32; 2]>>),
    /// Pre-computed NaN-separated 3-D point list (leader lines, arrowheads, etc.).
    Lines(Vec<[f32; 3]>),
    Volume(Solid),
}

pub struct TruckEntity {
    pub object: TruckObject,
    pub snap_pts: Vec<(Vec3, SnapHint)>,
    pub tangent_geoms: Vec<TangentGeom>,
    pub key_vertices: Vec<[f32; 3]>,
}

pub fn convert(entity: &EntityType, document: &CadDocument) -> Option<TruckEntity> {
    entity.to_truck_entity(document)
}

pub fn convert_native(entity: &nm::Entity, document: &nm::CadDocument) -> Option<TruckEntity> {
    match &entity.data {
        nm::EntityData::Point { position } => Some(point::to_truck(position)),
        nm::EntityData::Line { start, end } => Some(line::to_truck(start, end)),
        nm::EntityData::Circle { center, radius } => Some(circle::to_truck(center, *radius)),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Some(arc::to_truck(center, *radius, *start_angle, *end_angle)),
        nm::EntityData::LwPolyline { vertices, closed } => {
            Some(lwpolyline::to_truck(vertices, *closed, 0.0))
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
        } => Some(text::to_truck_native(
            insertion,
            *height,
            value,
            *rotation,
            style_name,
            *width_factor,
            *oblique_angle,
            *horizontal_alignment,
            *vertical_alignment,
            *alignment_point,
            document,
        )),
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
        } => Some(mtext::to_truck_native(
            insertion,
            *height,
            *width,
            *rectangle_height,
            value,
            *rotation,
            style_name,
            *attachment_point,
            *line_spacing_factor,
            *drawing_direction,
            document,
        )),
        nm::EntityData::MultiLeader {
            content_type,
            text_label,
            style_name,
            arrowhead_size,
            dogleg_length,
            path_type,
            enable_landing,
            enable_dogleg,
            text_location,
            leader_vertices,
            leader_root_lengths,
            ..
        } => {
            let mut points: Vec<[f32; 3]> = Vec::new();
            let mut tangents: Vec<TangentGeom> = Vec::new();
            let mut key_vertices: Vec<[f32; 3]> = Vec::new();
            let mut first_segment = true;
            let invisible = *path_type == 0;
            let draw_arrow = *arrowhead_size > 0.0;

            for root in split_native_mleader_roots(leader_vertices, leader_root_lengths) {
                if root.is_empty() {
                    continue;
                }
                let connection = text_location.unwrap_or(*root.last().unwrap());
                let cp = [connection[0] as f32, connection[1] as f32, connection[2] as f32];

                if !invisible {
                    if !first_segment {
                        points.push([f32::NAN; 3]);
                    }
                    first_segment = false;

                    for point in &root {
                        let p = [point[0] as f32, point[1] as f32, point[2] as f32];
                        points.push(p);
                        key_vertices.push(p);
                    }

                    for window in root.windows(2) {
                        tangents.push(TangentGeom::Line {
                            p1: [window[0][0] as f32, window[0][1] as f32, window[0][2] as f32],
                            p2: [window[1][0] as f32, window[1][1] as f32, window[1][2] as f32],
                        });
                    }

                    let last = *root.last().unwrap();
                    let last_p = [last[0] as f32, last[1] as f32, last[2] as f32];
                    let dist = ((last_p[0] - cp[0]).powi(2) + (last_p[1] - cp[1]).powi(2)).sqrt();
                    if dist > 1e-9 {
                        points.push(cp);
                        key_vertices.push(cp);
                        tangents.push(TangentGeom::Line { p1: last_p, p2: cp });
                    }
                }

                if draw_arrow {
                    let tip = root[0];
                    let next = if root.len() >= 2 { root[1] } else { connection };
                    let tip_p = [tip[0] as f32, tip[1] as f32, tip[2] as f32];
                    let dx = (next[0] - tip[0]) as f32;
                    let dy = (next[1] - tip[1]) as f32;
                    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                    let (dx, dy) = (dx / len, dy / len);
                    let angle = std::f32::consts::PI / 6.0;
                    let (s, c) = angle.sin_cos();
                    points.push([f32::NAN; 3]);
                    points.push([
                        tip_p[0] + (dx * c - dy * s) * *arrowhead_size as f32,
                        tip_p[1] + (dx * s + dy * c) * *arrowhead_size as f32,
                        tip_p[2],
                    ]);
                    points.push(tip_p);
                    points.push([
                        tip_p[0] + (dx * c + dy * s) * *arrowhead_size as f32,
                        tip_p[1] + (-dx * s + dy * c) * *arrowhead_size as f32,
                        tip_p[2],
                    ]);
                }

                if *enable_landing && *enable_dogleg && *dogleg_length > 0.0 {
                    let last = *root.last().unwrap();
                    let mut dir = [
                        connection[0] - last[0],
                        connection[1] - last[1],
                        connection[2] - last[2],
                    ];
                    let dlen = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
                    if dlen > 1e-9 {
                        dir[0] /= dlen;
                        dir[1] /= dlen;
                        points.push([f32::NAN; 3]);
                        points.push(cp);
                        points.push([
                            (connection[0] + dir[0] * *dogleg_length) as f32,
                            (connection[1] + dir[1] * *dogleg_length) as f32,
                            connection[2] as f32,
                        ]);
                    }
                }
            }

            if *content_type == 1 && !text_label.is_empty() {
                if let Some(loc) = text_location {
                    let text = mtext::to_truck_native(
                        loc,
                        document.header.textsize.max(0.01),
                        0.0,
                        None,
                        text_label,
                        0.0,
                        style_name,
                        1,
                        1.0,
                        1,
                        document,
                    );
                    match text.object {
                        TruckObject::Text(strokes) => {
                            for stroke in strokes {
                                if stroke.len() < 2 {
                                    continue;
                                }
                                points.push([f32::NAN; 3]);
                                for [x, y] in stroke {
                                    points.push([x, y, loc[2] as f32]);
                                }
                            }
                        }
                        TruckObject::Lines(text_points) => {
                            if !text_points.is_empty() {
                                points.push([f32::NAN; 3]);
                                points.extend(text_points);
                            }
                        }
                        _ => {}
                    }
                }
            }

            if points.is_empty() {
                None
            } else {
                Some(TruckEntity {
                    object: TruckObject::Lines(points),
                    snap_pts: text_location
                        .map(|loc| vec![(Vec3::new(loc[0] as f32, loc[1] as f32, loc[2] as f32), SnapHint::Insertion)])
                        .unwrap_or_default(),
                    tangent_geoms: tangents,
                    key_vertices,
                })
            }
        }
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use h7cad_native_model as nm;

    #[test]
    fn convert_native_line_returns_truck_geometry() {
        let mut doc = nm::CadDocument::new();
        let handle = doc
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("line should be added");

        let entity = doc.get_entity(handle).expect("line should exist");
        let truck = convert_native(entity, &doc).expect("native line should convert");
        assert_eq!(truck.key_vertices.len(), 2);
    }

    #[test]
    fn convert_native_multileader_returns_line_geometry() {
        let mut doc = nm::CadDocument::new();
        let handle = doc
            .add_entity(nm::Entity::new(nm::EntityData::MultiLeader {
                content_type: 1,
                text_label: "TAG".into(),
                style_name: "Standard".into(),
                arrowhead_size: 2.5,
                landing_gap: 0.0,
                dogleg_length: 2.5,
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
                leader_vertices: vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 1.0],
                    [6.0, 0.0, 4.0],
                    [10.0, 0.0, 0.0],
                    [6.0, 0.0, 4.0],
                ],
                leader_root_lengths: vec![3, 2],
            }))
            .expect("multileader should be added");

        let entity = doc.get_entity(handle).expect("multileader should exist");
        let truck = convert_native(entity, &doc).expect("native multileader should convert");
        match truck.object {
            TruckObject::Lines(points) => assert!(points.len() >= 5),
            _ => panic!("expected line geometry"),
        }
    }
}
