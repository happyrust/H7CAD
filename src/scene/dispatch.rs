// Dispatch entry points for entity editing.

use crate::types::{Color as AcadColor, LineWeight, Transparency};
use acadrust::{EntityType, Handle};
use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::{arc, circle, line, lwpolyline, mtext, point, text};
use crate::entities::traits::EntityTypeOps;
use crate::io::native_bridge;
use crate::scene::object::{GripDef, GripShape, PropSection};
use crate::scene::properties;

pub fn grips(entity: &EntityType) -> Vec<GripDef> {
    EntityTypeOps::grips(entity)
}

pub fn properties_sectioned(
    handle: Handle,
    entity: &EntityType,
    text_style_names: &[String],
) -> Vec<PropSection> {
    let general = properties::general_section(entity);
    let geometry = entity
        .geometry_properties(text_style_names)
        .unwrap_or_else(|| properties::fallback_properties(handle, entity));
    vec![general, geometry]
}

pub fn apply_common_prop(entity: &mut EntityType, field: &str, value: &str) {
    let e = entity.as_entity_mut();
    match field {
        "layer" => e.set_layer(value.to_string()),
        "linetype" => {
            entity.common_mut().linetype = if value == "ByLayer" {
                String::new()
            } else {
                value.to_string()
            };
        }
        "linetype_scale" => {
            if let Ok(v) = value.trim().parse::<f64>() {
                if v > 0.0 {
                    entity.common_mut().linetype_scale = v;
                }
            }
        }
        "transparency" => {
            if let Ok(pct) = value.trim().parse::<f64>() {
                let alpha = (pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8;
                entity
                    .as_entity_mut()
                    .set_transparency(Transparency::new(alpha));
            }
        }
        _ => {}
    }
}

pub fn apply_color(entity: &mut EntityType, color: AcadColor) {
    entity.as_entity_mut().set_color(color);
}

pub fn apply_line_weight(entity: &mut EntityType, lw: LineWeight) {
    entity.as_entity_mut().set_line_weight(lw);
}

pub fn apply_geom_prop(entity: &mut EntityType, field: &str, value: &str) {
    EntityTypeOps::apply_geom_prop(entity, field, value);
}

pub fn apply_grip(entity: &mut EntityType, grip_id: usize, apply: crate::scene::object::GripApply) {
    EntityTypeOps::apply_grip(entity, grip_id, apply);
}

pub fn apply_transform(entity: &mut EntityType, t: &EntityTransform) {
    EntityTypeOps::apply_transform(entity, t);
}

fn bridged_native_entity(entity: &nm::Entity) -> Option<EntityType> {
    native_bridge::native_entity_to_acadrust(entity)
}

fn preserve_native_nonbridge_common(updated: &mut nm::Entity, original: &nm::Entity) {
    updated.handle = original.handle;
    updated.owner_handle = original.owner_handle;
    updated.transparency = original.transparency;
    updated.thickness = original.thickness;
    updated.extrusion = original.extrusion;
    updated.xdata = original.xdata.clone();
}

fn edit_native_via_compat(
    entity: &mut nm::Entity,
    edit: impl FnOnce(&mut EntityType),
) -> bool {
    let Some(mut compat) = bridged_native_entity(entity) else {
        return false;
    };
    edit(&mut compat);
    let Some(mut updated) = native_bridge::acadrust_entity_to_native(&compat) else {
        return false;
    };
    preserve_native_nonbridge_common(&mut updated, entity);
    *entity = updated;
    true
}

fn dimension_point(point: [f64; 3]) -> Vec3 {
    Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32)
}

fn square_grip_local(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Square,
    }
}

fn diamond_grip_local(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: true,
        shape: GripShape::Diamond,
    }
}

fn apply_to_dimension_point(point: &mut [f64; 3], apply: &crate::scene::object::GripApply) {
    match apply {
        crate::scene::object::GripApply::Absolute(pos) => {
            point[0] = pos.x as f64;
            point[1] = pos.y as f64;
            point[2] = pos.z as f64;
        }
        crate::scene::object::GripApply::Translate(delta) => {
            point[0] += delta.x as f64;
            point[1] += delta.y as f64;
            point[2] += delta.z as f64;
        }
    }
}

fn assign_dimension_component(point: &mut [f64; 3], axis: usize, value: &str) {
    if let Ok(parsed) = value.trim().parse::<f64>() {
        point[axis] = parsed;
    }
}

fn measure_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn measure_angle(vertex: [f64; 3], first: [f64; 3], second: [f64; 3]) -> f64 {
    let v1 = [first[0] - vertex[0], first[1] - vertex[1], first[2] - vertex[2]];
    let v2 = [second[0] - vertex[0], second[1] - vertex[1], second[2] - vertex[2]];
    let len1 = (v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]).sqrt();
    let len2 = (v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]).sqrt();
    if len1 <= 1e-9 || len2 <= 1e-9 {
        return 0.0;
    }
    let cos = ((v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]) / (len1 * len2))
        .clamp(-1.0, 1.0);
    cos.acos().to_degrees()
}

fn update_dimension_measurement(entity: &mut nm::Entity) {
    let nm::EntityData::Dimension {
        dim_type,
        measurement,
        definition_point,
        first_point,
        second_point,
        angle_vertex,
        ..
    } = &mut entity.data
    else {
        return;
    };

    *measurement = match *dim_type & 0x0F {
        0 | 1 => measure_distance(*first_point, *second_point),
        2 | 5 => measure_angle(*angle_vertex, *first_point, *second_point),
        3 => measure_distance(*angle_vertex, *definition_point) * 2.0,
        4 => measure_distance(*angle_vertex, *definition_point),
        6 => {
            if (*dim_type & 0x40) != 0 {
                (first_point[0] - definition_point[0]).abs()
            } else {
                (first_point[1] - definition_point[1]).abs()
            }
        }
        _ => *measurement,
    };
}

fn grips_dimension_native(entity: &nm::Entity) -> Vec<GripDef> {
    let nm::EntityData::Dimension {
        dim_type,
        text_midpoint,
        first_point,
        second_point,
        definition_point,
        angle_vertex,
        ..
    } = &entity.data
    else {
        return vec![];
    };
    let text = dimension_point(*text_midpoint);
    match *dim_type & 0x0F {
        0 | 1 => vec![
            square_grip_local(0, dimension_point(*first_point)),
            diamond_grip_local(1, dimension_point(*second_point)),
            diamond_grip_local(2, dimension_point(*definition_point)),
            diamond_grip_local(3, text),
        ],
        3 | 4 => vec![
            square_grip_local(0, dimension_point(*angle_vertex)),
            diamond_grip_local(1, dimension_point(*definition_point)),
            diamond_grip_local(2, text),
        ],
        2 | 5 => vec![
            square_grip_local(0, dimension_point(*angle_vertex)),
            diamond_grip_local(1, dimension_point(*first_point)),
            diamond_grip_local(2, dimension_point(*second_point)),
            diamond_grip_local(3, dimension_point(*definition_point)),
            diamond_grip_local(4, text),
        ],
        6 => vec![
            square_grip_local(0, dimension_point(*definition_point)),
            diamond_grip_local(1, dimension_point(*first_point)),
            diamond_grip_local(2, dimension_point(*second_point)),
            diamond_grip_local(3, text),
        ],
        _ => vec![],
    }
}

fn apply_dimension_geom_prop_native(entity: &mut nm::Entity, field: &str, value: &str) {
    let nm::EntityData::Dimension {
        dim_type,
        style_name,
        text_midpoint,
        text_override,
        text_rotation,
        horizontal_direction,
        first_point,
        second_point,
        definition_point,
        angle_vertex,
        dimension_arc,
        leader_length,
        rotation,
        ext_line_rotation,
        ..
    } = &mut entity.data
    else {
        return;
    };

    match field {
        "text" | "user_text" => {
            *text_override = value.to_string();
        }
        "style_name" => *style_name = value.to_string(),
        "text_x" => assign_dimension_component(text_midpoint, 0, value),
        "text_y" => assign_dimension_component(text_midpoint, 1, value),
        "text_z" => assign_dimension_component(text_midpoint, 2, value),
        "text_rotation" => {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                *text_rotation = parsed;
            }
        }
        "horizontal_direction" => {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                *horizontal_direction = parsed;
            }
        }
        "line_spacing_factor" => {}
        "first_x" => assign_dimension_component(first_point, 0, value),
        "first_y" => assign_dimension_component(first_point, 1, value),
        "first_z" => assign_dimension_component(first_point, 2, value),
        "second_x" => assign_dimension_component(second_point, 0, value),
        "second_y" => assign_dimension_component(second_point, 1, value),
        "second_z" => assign_dimension_component(second_point, 2, value),
        "definition_x" => assign_dimension_component(definition_point, 0, value),
        "definition_y" => assign_dimension_component(definition_point, 1, value),
        "definition_z" => assign_dimension_component(definition_point, 2, value),
        "center_x" | "vertex_x" => assign_dimension_component(angle_vertex, 0, value),
        "center_y" | "vertex_y" => assign_dimension_component(angle_vertex, 1, value),
        "center_z" | "vertex_z" => assign_dimension_component(angle_vertex, 2, value),
        "point_x" => assign_dimension_component(definition_point, 0, value),
        "point_y" => assign_dimension_component(definition_point, 1, value),
        "point_z" => assign_dimension_component(definition_point, 2, value),
        "dimension_arc_x" => assign_dimension_component(dimension_arc, 0, value),
        "dimension_arc_y" => assign_dimension_component(dimension_arc, 1, value),
        "dimension_arc_z" => assign_dimension_component(dimension_arc, 2, value),
        "leader_length" => {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                *leader_length = parsed;
            }
        }
        "rotation" => {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                *rotation = parsed;
            }
        }
        "ext_line_rotation" => {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                *ext_line_rotation = parsed;
            }
        }
        "feature_x" => assign_dimension_component(first_point, 0, value),
        "feature_y" => assign_dimension_component(first_point, 1, value),
        "feature_z" => assign_dimension_component(first_point, 2, value),
        "leader_x" => assign_dimension_component(second_point, 0, value),
        "leader_y" => assign_dimension_component(second_point, 1, value),
        "leader_z" => assign_dimension_component(second_point, 2, value),
        _ => {}
    }

    if matches!(*dim_type & 0x0F, 0..=6) {
        update_dimension_measurement(entity);
    }
}

fn apply_dimension_grip_native(
    entity: &mut nm::Entity,
    grip_id: usize,
    apply: crate::scene::object::GripApply,
) {
    let nm::EntityData::Dimension {
        dim_type,
        text_midpoint,
        first_point,
        second_point,
        definition_point,
        angle_vertex,
        ..
    } = &mut entity.data
    else {
        return;
    };

    let text_grip = match *dim_type & 0x0F {
        0 | 1 => 3,
        3 | 4 => 2,
        2 | 5 => 4,
        6 => 3,
        _ => return,
    };
    if grip_id == text_grip {
        apply_to_dimension_point(text_midpoint, &apply);
        return;
    }

    match *dim_type & 0x0F {
        0 | 1 => match grip_id {
            0 => apply_to_dimension_point(first_point, &apply),
            1 => apply_to_dimension_point(second_point, &apply),
            2 => apply_to_dimension_point(definition_point, &apply),
            _ => {}
        },
        3 | 4 => match grip_id {
            0 => apply_to_dimension_point(angle_vertex, &apply),
            1 => apply_to_dimension_point(definition_point, &apply),
            _ => {}
        },
        2 | 5 => match grip_id {
            0 => apply_to_dimension_point(angle_vertex, &apply),
            1 => apply_to_dimension_point(first_point, &apply),
            2 => apply_to_dimension_point(second_point, &apply),
            3 => apply_to_dimension_point(definition_point, &apply),
            _ => {}
        },
        6 => match grip_id {
            0 => apply_to_dimension_point(definition_point, &apply),
            1 => apply_to_dimension_point(first_point, &apply),
            2 => apply_to_dimension_point(second_point, &apply),
            _ => {}
        },
        _ => {}
    }
    update_dimension_measurement(entity);
}

fn transform_dimension_point(point: &mut [f64; 3], transform: &EntityTransform) {
    match transform {
        EntityTransform::Translate(delta) => {
            point[0] += delta.x as f64;
            point[1] += delta.y as f64;
            point[2] += delta.z as f64;
        }
        EntityTransform::Rotate { center, angle_rad } => {
            let dx = point[0] as f32 - center.x;
            let dy = point[1] as f32 - center.y;
            let (s, c) = angle_rad.sin_cos();
            point[0] = (center.x + dx * c - dy * s) as f64;
            point[1] = (center.y + dx * s + dy * c) as f64;
        }
        EntityTransform::Scale { center, factor } => {
            point[0] = (center.x + (point[0] as f32 - center.x) * factor) as f64;
            point[1] = (center.y + (point[1] as f32 - center.y) * factor) as f64;
            point[2] = (center.z + (point[2] as f32 - center.z) * factor) as f64;
        }
        EntityTransform::Mirror { p1, p2 } => {
            let (mut x, mut y) = (point[0], point[1]);
            crate::scene::transform::reflect_xy_point(&mut x, &mut y, *p1, *p2);
            point[0] = x;
            point[1] = y;
        }
    }
}

fn apply_dimension_transform_native(entity: &mut nm::Entity, transform: &EntityTransform) {
    let nm::EntityData::Dimension {
        text_midpoint,
        first_point,
        second_point,
        definition_point,
        angle_vertex,
        dimension_arc,
        ..
    } = &mut entity.data
    else {
        return;
    };

    transform_dimension_point(text_midpoint, transform);
    transform_dimension_point(first_point, transform);
    transform_dimension_point(second_point, transform);
    transform_dimension_point(definition_point, transform);
    transform_dimension_point(angle_vertex, transform);
    transform_dimension_point(dimension_arc, transform);
    update_dimension_measurement(entity);
}

pub fn grips_native(entity: &nm::Entity) -> Vec<GripDef> {
    match &entity.data {
        nm::EntityData::Point { position } => point::grips(position),
        nm::EntityData::Line { start, end } => line::grips(start, end),
        nm::EntityData::Circle { center, radius } => circle::grips(center, *radius),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::grips(center, *radius, *start_angle, *end_angle),
        nm::EntityData::LwPolyline { vertices, .. } => lwpolyline::grips(vertices, 0.0),
        nm::EntityData::Text { insertion, .. } => text::grips_native(insertion),
        nm::EntityData::MText {
            insertion,
            width,
            rotation,
            ..
        } => mtext::grips_native(insertion, *width, *rotation),
        nm::EntityData::Dimension { .. } => grips_dimension_native(entity),
        _ => bridged_native_entity(entity).map(|compat| grips(&compat)).unwrap_or_default(),
    }
}

pub fn properties_sectioned_native(
    handle: nm::Handle,
    entity: &nm::Entity,
    text_style_names: &[String],
) -> Vec<PropSection> {
    let general = properties::general_section_native(entity);
    let geometry = geometry_properties_native(entity, text_style_names)
        .unwrap_or_else(|| properties::fallback_properties_native(handle, entity));
    vec![general, geometry]
}

pub fn apply_common_prop_native(entity: &mut nm::Entity, field: &str, value: &str) {
    match field {
        "layer" => entity.layer_name = value.to_string(),
        "linetype" => {
            entity.linetype_name = if value == "ByLayer" {
                String::new()
            } else {
                value.to_string()
            };
        }
        "transparency" => {
            if let Ok(pct) = value.trim().parse::<f64>() {
                entity.transparency = (pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as i32;
            }
        }
        _ => {}
    }
}

pub fn toggle_invisible_native(entity: &mut nm::Entity) {
    entity.invisible = !entity.invisible;
}

pub fn apply_color_native(entity: &mut nm::Entity, color: AcadColor) {
    match color {
        AcadColor::ByLayer => {
            entity.color_index = 256;
            entity.true_color = 0;
        }
        AcadColor::ByBlock => {
            entity.color_index = -2;
            entity.true_color = 0;
        }
        AcadColor::Index(i) => {
            entity.color_index = i as i16;
            entity.true_color = 0;
        }
        AcadColor::Rgb { r, g, b } => {
            entity.color_index = 256;
            entity.true_color = ((r as i32) << 16) | ((g as i32) << 8) | (b as i32);
        }
    }
}

pub fn apply_line_weight_native(entity: &mut nm::Entity, lw: LineWeight) {
    entity.lineweight = match lw {
        LineWeight::ByLayer => -1,
        LineWeight::ByBlock => -2,
        LineWeight::Default => -3,
        LineWeight::Value(v) => v,
    };
}

pub fn apply_geom_prop_native(entity: &mut nm::Entity, field: &str, value: &str) {
    match &mut entity.data {
        nm::EntityData::Point { position } => point::apply_geom_prop(position, field, value),
        nm::EntityData::Line { start, end } => line::apply_geom_prop(start, end, field, value),
        nm::EntityData::Circle { center, radius } => {
            circle::apply_geom_prop(center, radius, field, value)
        }
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::apply_geom_prop(center, radius, start_angle, end_angle, field, value),
        nm::EntityData::LwPolyline { .. } => {}
        nm::EntityData::Text {
            insertion,
            height,
            value: text_value,
            rotation,
            style_name,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            ..
        } => text::apply_geom_prop_native(
            insertion,
            height,
            text_value,
            rotation,
            style_name,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            field,
            value,
        ),
        nm::EntityData::MText {
            insertion,
            height,
            width,
            rectangle_height,
            value: text_value,
            rotation,
            style_name,
            attachment_point,
            line_spacing_factor,
            ..
        } => mtext::apply_geom_prop_native(
            insertion,
            height,
            width,
            rectangle_height,
            text_value,
            rotation,
            style_name,
            attachment_point,
            line_spacing_factor,
            field,
            value,
        ),
        nm::EntityData::Dimension { .. } => apply_dimension_geom_prop_native(entity, field, value),
        _ => {
            let _ = edit_native_via_compat(entity, |compat| apply_geom_prop(compat, field, value));
        }
    }
}

pub fn apply_grip_native(entity: &mut nm::Entity, grip_id: usize, apply: crate::scene::object::GripApply) {
    match &mut entity.data {
        nm::EntityData::Point { position } => point::apply_grip(position, grip_id, apply),
        nm::EntityData::Line { start, end } => line::apply_grip(start, end, grip_id, apply),
        nm::EntityData::Circle { center, radius } => {
            circle::apply_grip(center, radius, grip_id, apply)
        }
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::apply_grip(center, radius, start_angle, end_angle, grip_id, apply),
        nm::EntityData::LwPolyline { vertices, .. } => lwpolyline::apply_grip(vertices, grip_id, apply),
        nm::EntityData::Text { insertion, .. } => text::apply_grip_native(insertion, grip_id, apply),
        nm::EntityData::MText {
            insertion,
            width,
            rotation,
            ..
        } => mtext::apply_grip_native(insertion, width, *rotation, grip_id, apply),
        nm::EntityData::Dimension { .. } => apply_dimension_grip_native(entity, grip_id, apply),
        _ => {
            let _ = edit_native_via_compat(entity, |compat| apply_grip(compat, grip_id, apply));
        }
    }
}

pub fn apply_transform_native(entity: &mut nm::Entity, t: &EntityTransform) {
    match &mut entity.data {
        nm::EntityData::Point { position } => point::apply_transform(position, t),
        nm::EntityData::Line { start, end } => line::apply_transform(start, end, t),
        nm::EntityData::Circle { center, radius } => circle::apply_transform(center, radius, t),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::apply_transform(center, radius, start_angle, end_angle, t),
        nm::EntityData::LwPolyline { vertices, .. } => lwpolyline::apply_transform(vertices, t),
        nm::EntityData::Text {
            insertion,
            rotation,
            ..
        } => text::apply_transform_native(insertion, rotation, t),
        nm::EntityData::MText {
            insertion,
            rotation,
            ..
        } => mtext::apply_transform_native(insertion, rotation, t),
        nm::EntityData::Dimension { .. } => apply_dimension_transform_native(entity, t),
        _ => {
            let _ = edit_native_via_compat(entity, |compat| apply_transform(compat, t));
        }
    }
}

fn geometry_properties_native(entity: &nm::Entity, text_style_names: &[String]) -> Option<PropSection> {
    match &entity.data {
        nm::EntityData::Point { position } => Some(point::properties(position)),
        nm::EntityData::Line { start, end } => Some(line::properties(start, end)),
        nm::EntityData::Circle { center, radius } => Some(circle::properties(center, *radius)),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Some(arc::properties(center, *radius, *start_angle, *end_angle)),
        nm::EntityData::LwPolyline { vertices, closed } => {
            Some(lwpolyline::properties(vertices, *closed, 0.0))
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
            ..
        } => Some(text::properties_native(
            insertion,
            *height,
            value,
            *rotation,
            style_name,
            *width_factor,
            *oblique_angle,
            *horizontal_alignment,
            *vertical_alignment,
            text_style_names,
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
        } => Some(mtext::properties_native(
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
            text_style_names,
        )),
        _ => bridged_native_entity(entity)
            .and_then(|compat| compat.geometry_properties(text_style_names)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use h7cad_native_model as nm;

    #[test]
    fn properties_sectioned_native_exposes_general_and_geometry() {
        let entity = nm::Entity::new(nm::EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [5.0, 0.0, 0.0],
        });
        let sections = properties_sectioned_native(nm::Handle::new(0x11), &entity, &[]);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "General");
        assert_eq!(sections[1].title, "Geometry");
    }

    #[test]
    fn apply_geom_prop_native_updates_text_content_and_rotation() {
        let mut entity = nm::Entity::new(nm::EntityData::Text {
            insertion: [1.0, 2.0, 0.0],
            height: 2.5,
            value: "old".into(),
            rotation: 0.0,
            style_name: "Standard".into(),
            width_factor: 1.0,
            oblique_angle: 0.0,
            horizontal_alignment: 0,
            vertical_alignment: 0,
            alignment_point: None,
        });

        apply_geom_prop_native(&mut entity, "content", "new");
        apply_geom_prop_native(&mut entity, "rotation", "90");

        match &entity.data {
            nm::EntityData::Text {
                value, rotation, ..
            } => {
                assert_eq!(value, "new");
                assert!((*rotation - 90.0).abs() < 1e-9);
            }
            other => panic!("expected text entity, got {other:?}"),
        }
    }

    #[test]
    fn grips_native_bridges_dimension_and_multileader() {
        let dimension = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: String::new(),
            style_name: "Standard".into(),
            definition_point: [4.0, 5.0, 0.0],
            text_midpoint: [2.0, 3.0, 0.0],
            text_override: "<>".into(),
            attachment_point: 0,
            measurement: 12.5,
            text_rotation: 15.0,
            horizontal_direction: 0.0,
            flip_arrow1: false,
            flip_arrow2: false,
            first_point: [0.0, 0.0, 0.0],
            second_point: [10.0, 0.0, 0.0],
            angle_vertex: [0.0, 0.0, 0.0],
            dimension_arc: [0.0, 0.0, 0.0],
            leader_length: 0.0,
            rotation: 25.0,
            ext_line_rotation: 35.0,
        });
        let multileader = nm::Entity::new(nm::EntityData::MultiLeader {
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
            leader_vertices: vec![[0.0, 0.0, 0.0], [6.0, 0.0, 4.0]],
            leader_root_lengths: vec![2],
        });

        assert!(!grips_native(&dimension).is_empty());
        assert_eq!(grips_native(&multileader).len(), 3);
    }

    #[test]
    fn apply_geom_prop_and_grip_native_bridge_dimension_and_multileader() {
        let mut dimension = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: String::new(),
            style_name: "Standard".into(),
            definition_point: [4.0, 5.0, 0.0],
            text_midpoint: [2.0, 3.0, 0.0],
            text_override: "<>".into(),
            attachment_point: 0,
            measurement: 12.5,
            text_rotation: 15.0,
            horizontal_direction: 0.0,
            flip_arrow1: false,
            flip_arrow2: false,
            first_point: [0.0, 0.0, 0.0],
            second_point: [10.0, 0.0, 0.0],
            angle_vertex: [0.0, 0.0, 0.0],
            dimension_arc: [0.0, 0.0, 0.0],
            leader_length: 0.0,
            rotation: 25.0,
            ext_line_rotation: 35.0,
        });
        apply_geom_prop_native(&mut dimension, "text_x", "8.0");
        apply_grip_native(
            &mut dimension,
            3,
            crate::scene::object::GripApply::Absolute(glam::Vec3::new(9.0, 4.0, 0.0)),
        );
        match &dimension.data {
            nm::EntityData::Dimension { text_midpoint, .. } => {
                assert_eq!(*text_midpoint, [9.0, 4.0, 0.0]);
            }
            other => panic!("expected native dimension, got {other:?}"),
        }

        let mut multileader = nm::Entity::new(nm::EntityData::MultiLeader {
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
            leader_vertices: vec![[0.0, 0.0, 0.0], [6.0, 0.0, 4.0]],
            leader_root_lengths: vec![2],
        });
        apply_geom_prop_native(&mut multileader, "text_x", "7.0");
        apply_grip_native(
            &mut multileader,
            2,
            crate::scene::object::GripApply::Absolute(glam::Vec3::new(8.0, 1.0, 5.0)),
        );
        match &multileader.data {
            nm::EntityData::MultiLeader { text_location, .. } => {
                assert_eq!(*text_location, Some([8.0, 1.0, 5.0]));
            }
            other => panic!("expected native multileader, got {other:?}"),
        }
    }

    #[test]
    fn apply_geom_prop_native_preserves_dimension_flip_flags() {
        let mut entity = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: String::new(),
            style_name: "Standard".into(),
            definition_point: [4.0, 5.0, 0.0],
            text_midpoint: [2.0, 3.0, 0.0],
            text_override: "<>".into(),
            attachment_point: 0,
            measurement: 12.5,
            text_rotation: 15.0,
            horizontal_direction: 0.0,
            flip_arrow1: true,
            flip_arrow2: true,
            first_point: [0.0, 0.0, 0.0],
            second_point: [10.0, 0.0, 0.0],
            angle_vertex: [0.0, 0.0, 0.0],
            dimension_arc: [0.0, 0.0, 0.0],
            leader_length: 0.0,
            rotation: 25.0,
            ext_line_rotation: 35.0,
        });

        apply_geom_prop_native(&mut entity, "text_x", "8.0");

        match &entity.data {
            nm::EntityData::Dimension {
                text_midpoint,
                flip_arrow1,
                flip_arrow2,
                ..
            } => {
                assert_eq!(*text_midpoint, [8.0, 3.0, 0.0]);
                assert!(*flip_arrow1);
                assert!(*flip_arrow2);
            }
            other => panic!("expected dimension entity, got {other:?}"),
        }
    }

    #[test]
    fn apply_transform_native_preserves_dimension_flip_flags() {
        let mut entity = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: String::new(),
            style_name: "Standard".into(),
            definition_point: [4.0, 5.0, 0.0],
            text_midpoint: [2.0, 3.0, 0.0],
            text_override: "<>".into(),
            attachment_point: 0,
            measurement: 12.5,
            text_rotation: 15.0,
            horizontal_direction: 0.0,
            flip_arrow1: true,
            flip_arrow2: true,
            first_point: [0.0, 0.0, 0.0],
            second_point: [10.0, 0.0, 0.0],
            angle_vertex: [0.0, 0.0, 0.0],
            dimension_arc: [0.0, 0.0, 0.0],
            leader_length: 0.0,
            rotation: 25.0,
            ext_line_rotation: 35.0,
        });

        apply_transform_native(
            &mut entity,
            &EntityTransform::Translate(glam::Vec3::new(2.0, 1.0, 0.0)),
        );

        match &entity.data {
            nm::EntityData::Dimension {
                first_point,
                second_point,
                flip_arrow1,
                flip_arrow2,
                ..
            } => {
                assert_eq!(*first_point, [2.0, 1.0, 0.0]);
                assert_eq!(*second_point, [12.0, 1.0, 0.0]);
                assert!(*flip_arrow1);
                assert!(*flip_arrow2);
            }
            other => panic!("expected dimension entity, got {other:?}"),
        }
    }
}
