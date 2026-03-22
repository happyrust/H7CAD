use acadrust::entities::{
    Dimension, DimensionAligned, DimensionAngular2Ln, DimensionAngular3Pt, DimensionBase,
    DimensionDiameter, DimensionLinear, DimensionOrdinate, DimensionRadius,
};
use acadrust::Entity;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, ro_prop as ro};
use crate::entities::traits::{PropertyEditable, Transformable};
use crate::scene::object::PropSection;

fn base_props(base: &DimensionBase) -> Vec<crate::scene::object::Property> {
    vec![
        crate::scene::object::Property {
            label: "Text".into(),
            field: "text",
            value: crate::scene::object::PropValue::EditText(base.text.clone()),
        },
        crate::scene::object::Property {
            label: "User Text".into(),
            field: "user_text",
            value: crate::scene::object::PropValue::EditText(
                base.user_text.clone().unwrap_or_default(),
            ),
        },
        crate::scene::object::Property {
            label: "Style".into(),
            field: "style_name",
            value: crate::scene::object::PropValue::EditText(base.style_name.clone()),
        },
        edit("Text X", "text_x", base.text_middle_point.x),
        edit("Text Y", "text_y", base.text_middle_point.y),
        edit("Text Z", "text_z", base.text_middle_point.z),
        edit("Text Rotation", "text_rotation", base.text_rotation),
        edit("Horizontal Dir", "horizontal_direction", base.horizontal_direction),
        edit("Line Spacing", "line_spacing_factor", base.line_spacing_factor),
        ro("Measurement", "measurement", format!("{:.4}", base.actual_measurement)),
    ]
}

fn properties(dim: &Dimension) -> PropSection {
    let mut props = base_props(dim.base());
    match dim {
        Dimension::Aligned(d) => {
            props.extend(linear_like_props(d.first_point, d.second_point, d.definition_point));
            props.push(edit("Ext Rotation", "ext_line_rotation", d.ext_line_rotation));
        }
        Dimension::Linear(d) => {
            props.extend(linear_like_props(d.first_point, d.second_point, d.definition_point));
            props.push(edit("Rotation", "rotation", d.rotation));
            props.push(edit("Ext Rotation", "ext_line_rotation", d.ext_line_rotation));
        }
        Dimension::Radius(d) => {
            props.extend(radius_like_props(d.angle_vertex, d.definition_point));
            props.push(edit("Leader Length", "leader_length", d.leader_length));
        }
        Dimension::Diameter(d) => {
            props.extend(radius_like_props(d.angle_vertex, d.definition_point));
            props.push(edit("Leader Length", "leader_length", d.leader_length));
        }
        Dimension::Angular2Ln(d) => {
            props.extend(angular_props(
                d.angle_vertex,
                d.first_point,
                d.second_point,
                d.definition_point,
            ));
            props.push(edit("Arc X", "dimension_arc_x", d.dimension_arc.x));
            props.push(edit("Arc Y", "dimension_arc_y", d.dimension_arc.y));
            props.push(edit("Arc Z", "dimension_arc_z", d.dimension_arc.z));
        }
        Dimension::Angular3Pt(d) => {
            props.extend(angular_props(
                d.angle_vertex,
                d.first_point,
                d.second_point,
                d.definition_point,
            ));
        }
        Dimension::Ordinate(d) => {
            props.push(edit("Origin X", "definition_x", d.definition_point.x));
            props.push(edit("Origin Y", "definition_y", d.definition_point.y));
            props.push(edit("Origin Z", "definition_z", d.definition_point.z));
            props.push(edit("Feature X", "feature_x", d.feature_location.x));
            props.push(edit("Feature Y", "feature_y", d.feature_location.y));
            props.push(edit("Feature Z", "feature_z", d.feature_location.z));
            props.push(edit("Leader X", "leader_x", d.leader_endpoint.x));
            props.push(edit("Leader Y", "leader_y", d.leader_endpoint.y));
            props.push(edit("Leader Z", "leader_z", d.leader_endpoint.z));
            props.push(ro(
                "Ordinate Type",
                "ordinate_type",
                if d.is_ordinate_type_x { "X" } else { "Y" },
            ));
        }
    }
    PropSection {
        title: "Geometry".into(),
        props,
    }
}

fn linear_like_props(
    first: acadrust::types::Vector3,
    second: acadrust::types::Vector3,
    definition: acadrust::types::Vector3,
) -> Vec<crate::scene::object::Property> {
    vec![
        edit("First X", "first_x", first.x),
        edit("First Y", "first_y", first.y),
        edit("First Z", "first_z", first.z),
        edit("Second X", "second_x", second.x),
        edit("Second Y", "second_y", second.y),
        edit("Second Z", "second_z", second.z),
        edit("Definition X", "definition_x", definition.x),
        edit("Definition Y", "definition_y", definition.y),
        edit("Definition Z", "definition_z", definition.z),
    ]
}

fn radius_like_props(
    center: acadrust::types::Vector3,
    point: acadrust::types::Vector3,
) -> Vec<crate::scene::object::Property> {
    vec![
        edit("Center X", "center_x", center.x),
        edit("Center Y", "center_y", center.y),
        edit("Center Z", "center_z", center.z),
        edit("Point X", "point_x", point.x),
        edit("Point Y", "point_y", point.y),
        edit("Point Z", "point_z", point.z),
    ]
}

fn angular_props(
    vertex: acadrust::types::Vector3,
    first: acadrust::types::Vector3,
    second: acadrust::types::Vector3,
    definition: acadrust::types::Vector3,
) -> Vec<crate::scene::object::Property> {
    vec![
        edit("Vertex X", "vertex_x", vertex.x),
        edit("Vertex Y", "vertex_y", vertex.y),
        edit("Vertex Z", "vertex_z", vertex.z),
        edit("First X", "first_x", first.x),
        edit("First Y", "first_y", first.y),
        edit("First Z", "first_z", first.z),
        edit("Second X", "second_x", second.x),
        edit("Second Y", "second_y", second.y),
        edit("Second Z", "second_z", second.z),
        edit("Definition X", "definition_x", definition.x),
        edit("Definition Y", "definition_y", definition.y),
        edit("Definition Z", "definition_z", definition.z),
    ]
}

fn apply_base_prop(base: &mut DimensionBase, field: &str, value: &str) -> bool {
    match field {
        "text" => {
            base.text = value.to_string();
            true
        }
        "user_text" => {
            base.user_text = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
            true
        }
        "style_name" => {
            base.style_name = value.to_string();
            true
        }
        "text_x" => assign_f64(value, &mut base.text_middle_point.x),
        "text_y" => assign_f64(value, &mut base.text_middle_point.y),
        "text_z" => assign_f64(value, &mut base.text_middle_point.z),
        "text_rotation" => assign_f64(value, &mut base.text_rotation),
        "horizontal_direction" => assign_f64(value, &mut base.horizontal_direction),
        "line_spacing_factor" => assign_f64(value, &mut base.line_spacing_factor),
        _ => false,
    }
}

fn assign_f64(value: &str, target: &mut f64) -> bool {
    let Some(v) = parse_f64(value) else {
        return false;
    };
    *target = v;
    true
}

fn apply_geom_prop(dim: &mut Dimension, field: &str, value: &str) {
    if apply_base_prop(dim.base_mut(), field, value) {
        return;
    }
    match dim {
        Dimension::Aligned(d) => apply_linear_fields_aligned(d, field, value),
        Dimension::Linear(d) => apply_linear_fields_linear(d, field, value),
        Dimension::Radius(d) => apply_radius_fields(d, field, value),
        Dimension::Diameter(d) => apply_diameter_fields(d, field, value),
        Dimension::Angular2Ln(d) => apply_angular2_fields(d, field, value),
        Dimension::Angular3Pt(d) => apply_angular3_fields(d, field, value),
        Dimension::Ordinate(d) => apply_ordinate_fields(d, field, value),
    }
    dim.base_mut().actual_measurement = dim.measurement();
}

fn apply_linear_fields_aligned(d: &mut DimensionAligned, field: &str, value: &str) {
    apply_linear_common(
        &mut d.first_point,
        &mut d.second_point,
        &mut d.definition_point,
        field,
        value,
    );
    let _ = assign_f64(value, &mut d.ext_line_rotation);
}

fn apply_linear_fields_linear(d: &mut DimensionLinear, field: &str, value: &str) {
    apply_linear_common(
        &mut d.first_point,
        &mut d.second_point,
        &mut d.definition_point,
        field,
        value,
    );
    match field {
        "rotation" => {
            let _ = assign_f64(value, &mut d.rotation);
        }
        "ext_line_rotation" => {
            let _ = assign_f64(value, &mut d.ext_line_rotation);
        }
        _ => {}
    }
}

fn apply_linear_common(
    first: &mut acadrust::types::Vector3,
    second: &mut acadrust::types::Vector3,
    definition: &mut acadrust::types::Vector3,
    field: &str,
    value: &str,
) {
    match field {
        "first_x" => {
            let _ = assign_f64(value, &mut first.x);
        }
        "first_y" => {
            let _ = assign_f64(value, &mut first.y);
        }
        "first_z" => {
            let _ = assign_f64(value, &mut first.z);
        }
        "second_x" => {
            let _ = assign_f64(value, &mut second.x);
        }
        "second_y" => {
            let _ = assign_f64(value, &mut second.y);
        }
        "second_z" => {
            let _ = assign_f64(value, &mut second.z);
        }
        "definition_x" => {
            let _ = assign_f64(value, &mut definition.x);
        }
        "definition_y" => {
            let _ = assign_f64(value, &mut definition.y);
        }
        "definition_z" => {
            let _ = assign_f64(value, &mut definition.z);
        }
        _ => {}
    }
}

fn apply_radius_fields(d: &mut DimensionRadius, field: &str, value: &str) {
    apply_radius_common(&mut d.angle_vertex, &mut d.definition_point, field, value);
    if field == "leader_length" {
        let _ = assign_f64(value, &mut d.leader_length);
    }
}

fn apply_diameter_fields(d: &mut DimensionDiameter, field: &str, value: &str) {
    apply_radius_common(&mut d.angle_vertex, &mut d.definition_point, field, value);
    if field == "leader_length" {
        let _ = assign_f64(value, &mut d.leader_length);
    }
}

fn apply_radius_common(
    center: &mut acadrust::types::Vector3,
    point: &mut acadrust::types::Vector3,
    field: &str,
    value: &str,
) {
    match field {
        "center_x" => {
            let _ = assign_f64(value, &mut center.x);
        }
        "center_y" => {
            let _ = assign_f64(value, &mut center.y);
        }
        "center_z" => {
            let _ = assign_f64(value, &mut center.z);
        }
        "point_x" => {
            let _ = assign_f64(value, &mut point.x);
        }
        "point_y" => {
            let _ = assign_f64(value, &mut point.y);
        }
        "point_z" => {
            let _ = assign_f64(value, &mut point.z);
        }
        _ => {}
    }
}

fn apply_angular2_fields(d: &mut DimensionAngular2Ln, field: &str, value: &str) {
    apply_angular_common(
        &mut d.angle_vertex,
        &mut d.first_point,
        &mut d.second_point,
        &mut d.definition_point,
        field,
        value,
    );
    match field {
        "dimension_arc_x" => {
            let _ = assign_f64(value, &mut d.dimension_arc.x);
        }
        "dimension_arc_y" => {
            let _ = assign_f64(value, &mut d.dimension_arc.y);
        }
        "dimension_arc_z" => {
            let _ = assign_f64(value, &mut d.dimension_arc.z);
        }
        _ => {}
    }
}

fn apply_angular3_fields(d: &mut DimensionAngular3Pt, field: &str, value: &str) {
    apply_angular_common(
        &mut d.angle_vertex,
        &mut d.first_point,
        &mut d.second_point,
        &mut d.definition_point,
        field,
        value,
    );
}

fn apply_angular_common(
    vertex: &mut acadrust::types::Vector3,
    first: &mut acadrust::types::Vector3,
    second: &mut acadrust::types::Vector3,
    definition: &mut acadrust::types::Vector3,
    field: &str,
    value: &str,
) {
    match field {
        "vertex_x" => {
            let _ = assign_f64(value, &mut vertex.x);
        }
        "vertex_y" => {
            let _ = assign_f64(value, &mut vertex.y);
        }
        "vertex_z" => {
            let _ = assign_f64(value, &mut vertex.z);
        }
        "first_x" => {
            let _ = assign_f64(value, &mut first.x);
        }
        "first_y" => {
            let _ = assign_f64(value, &mut first.y);
        }
        "first_z" => {
            let _ = assign_f64(value, &mut first.z);
        }
        "second_x" => {
            let _ = assign_f64(value, &mut second.x);
        }
        "second_y" => {
            let _ = assign_f64(value, &mut second.y);
        }
        "second_z" => {
            let _ = assign_f64(value, &mut second.z);
        }
        "definition_x" => {
            let _ = assign_f64(value, &mut definition.x);
        }
        "definition_y" => {
            let _ = assign_f64(value, &mut definition.y);
        }
        "definition_z" => {
            let _ = assign_f64(value, &mut definition.z);
        }
        _ => {}
    }
}

fn apply_ordinate_fields(d: &mut DimensionOrdinate, field: &str, value: &str) {
    match field {
        "definition_x" => {
            let _ = assign_f64(value, &mut d.definition_point.x);
        }
        "definition_y" => {
            let _ = assign_f64(value, &mut d.definition_point.y);
        }
        "definition_z" => {
            let _ = assign_f64(value, &mut d.definition_point.z);
        }
        "feature_x" => {
            let _ = assign_f64(value, &mut d.feature_location.x);
        }
        "feature_y" => {
            let _ = assign_f64(value, &mut d.feature_location.y);
        }
        "feature_z" => {
            let _ = assign_f64(value, &mut d.feature_location.z);
        }
        "leader_x" => {
            let _ = assign_f64(value, &mut d.leader_endpoint.x);
        }
        "leader_y" => {
            let _ = assign_f64(value, &mut d.leader_endpoint.y);
        }
        "leader_z" => {
            let _ = assign_f64(value, &mut d.leader_endpoint.z);
        }
        _ => {}
    }
}

fn apply_transform(dim: &mut Dimension, t: &EntityTransform) {
    match t {
        EntityTransform::Translate(d) => dim.translate(acadrust::types::Vector3::new(
            d.x as f64,
            d.y as f64,
            d.z as f64,
        )),
        EntityTransform::Rotate { center, angle_rad } => {
            transform_dimension_points(dim, |pt| rotate_point(pt, *center, *angle_rad))
        }
        EntityTransform::Scale { center, factor } => {
            transform_dimension_points(dim, |pt| scale_point(pt, *center, *factor))
        }
        EntityTransform::Mirror { p1, p2 } => {
            transform_dimension_points(dim, |pt| mirror_point(pt, *p1, *p2))
        }
    }
    dim.base_mut().actual_measurement = dim.measurement();
}

fn transform_dimension_points<F>(dim: &mut Dimension, mut f: F)
where
    F: FnMut(&mut acadrust::types::Vector3),
{
    f(&mut dim.base_mut().text_middle_point);
    f(&mut dim.base_mut().insertion_point);
    match dim {
        Dimension::Aligned(d) => {
            f(&mut d.first_point);
            f(&mut d.second_point);
            f(&mut d.definition_point);
        }
        Dimension::Linear(d) => {
            f(&mut d.first_point);
            f(&mut d.second_point);
            f(&mut d.definition_point);
        }
        Dimension::Radius(d) => {
            f(&mut d.angle_vertex);
            f(&mut d.definition_point);
        }
        Dimension::Diameter(d) => {
            f(&mut d.angle_vertex);
            f(&mut d.definition_point);
        }
        Dimension::Angular2Ln(d) => {
            f(&mut d.dimension_arc);
            f(&mut d.first_point);
            f(&mut d.second_point);
            f(&mut d.angle_vertex);
            f(&mut d.definition_point);
        }
        Dimension::Angular3Pt(d) => {
            f(&mut d.first_point);
            f(&mut d.second_point);
            f(&mut d.angle_vertex);
            f(&mut d.definition_point);
        }
        Dimension::Ordinate(d) => {
            f(&mut d.definition_point);
            f(&mut d.feature_location);
            f(&mut d.leader_endpoint);
        }
    }
}

fn rotate_point(p: &mut acadrust::types::Vector3, center: Vec3, angle_rad: f32) {
    let dx = p.x as f32 - center.x;
    let dy = p.y as f32 - center.y;
    let (s, c) = angle_rad.sin_cos();
    p.x = (center.x + dx * c - dy * s) as f64;
    p.y = (center.y + dx * s + dy * c) as f64;
}

fn scale_point(p: &mut acadrust::types::Vector3, center: Vec3, factor: f32) {
    p.x = (center.x + (p.x as f32 - center.x) * factor) as f64;
    p.y = (center.y + (p.y as f32 - center.y) * factor) as f64;
    p.z = (center.z + (p.z as f32 - center.z) * factor) as f64;
}

fn mirror_point(p: &mut acadrust::types::Vector3, p1: Vec3, p2: Vec3) {
    crate::scene::transform::reflect_xy_point(&mut p.x, &mut p.y, p1, p2);
}

impl PropertyEditable for Dimension {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Dimension {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
