use acadrust::entities::{Text, TextHorizontalAlignment as HA, TextVerticalAlignment as VA};
use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, square_grip};
use crate::entities::text_support::{resolve_text_style, resolve_text_style_native, text_local_bounds};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::cxf;
use crate::scene::object::{GripApply, GripDef, PropSection, PropValue, Property};
use crate::scene::wire_model::SnapHint;

fn text_halign_str(a: &acadrust::entities::TextHorizontalAlignment) -> &'static str {
    use acadrust::entities::TextHorizontalAlignment::*;
    match a {
        Left => "Left",
        Center => "Center",
        Right => "Right",
        Aligned => "Aligned",
        Middle => "Middle",
        Fit => "Fit",
    }
}

fn text_valign_str(a: &acadrust::entities::TextVerticalAlignment) -> &'static str {
    use acadrust::entities::TextVerticalAlignment::*;
    match a {
        Baseline => "Baseline",
        Bottom => "Bottom",
        Middle => "Middle",
        Top => "Top",
    }
}

fn sync_text_alignment_point(t: &mut Text) {
    let needs_alignment_point = !matches!(
        (t.horizontal_alignment, t.vertical_alignment),
        (HA::Left, VA::Baseline)
    );
    if needs_alignment_point {
        if t.alignment_point.is_none() {
            t.alignment_point = Some(t.insertion_point);
        }
    } else {
        t.alignment_point = None;
    }
}

fn to_truck(t: &Text, document: &acadrust::CadDocument) -> TruckEntity {
    let snap_pt = Vec3::new(
        t.insertion_point.x as f32,
        t.insertion_point.y as f32,
        t.insertion_point.z as f32,
    );
    let resolved_style = resolve_text_style(&t.style, document);
    let font_name = resolved_style.font_name;
    let width_factor = (if t.width_factor > 0.0 {
        t.width_factor as f32
    } else {
        1.0
    } * resolved_style.width_factor.max(0.01))
    .clamp(0.01, 100.0);
    let oblique_angle = t.oblique_angle as f32 + resolved_style.oblique_angle;
    let anchor = match (
        &t.horizontal_alignment,
        &t.vertical_alignment,
        &t.alignment_point,
    ) {
        (HA::Aligned | HA::Middle | HA::Fit, _, Some(a)) => [a.x as f32, a.y as f32],
        (HA::Center | HA::Right, _, Some(a)) => [a.x as f32, a.y as f32],
        (_, VA::Bottom | VA::Middle | VA::Top, Some(a)) => [a.x as f32, a.y as f32],
        _ => [t.insertion_point.x as f32, t.insertion_point.y as f32],
    };
    let bounds = text_local_bounds(
        &font_name,
        &t.value,
        t.height as f32,
        width_factor,
        oblique_angle,
    );
    let (anchor_local_x, anchor_local_y) = if let Some(([min_x, min_y], [max_x, max_y])) = bounds {
        let ax = match t.horizontal_alignment {
            HA::Left => min_x,
            HA::Center | HA::Middle => (min_x + max_x) * 0.5,
            HA::Right | HA::Aligned | HA::Fit => max_x,
        };
        let ay = match t.vertical_alignment {
            VA::Baseline => 0.0,
            VA::Bottom => min_y,
            VA::Middle => (min_y + max_y) * 0.5,
            VA::Top => max_y,
        };
        (ax, ay)
    } else {
        (0.0, 0.0)
    };
    let (cos_r, sin_r) = ((t.rotation as f32).cos(), (t.rotation as f32).sin());
    let origin = [
        anchor[0] - (anchor_local_x * cos_r - anchor_local_y * sin_r),
        anchor[1] - (anchor_local_x * sin_r + anchor_local_y * cos_r),
    ];
    let strokes_2d = cxf::tessellate_text_ex(
        origin,
        t.height as f32,
        t.rotation as f32,
        width_factor,
        oblique_angle,
        &font_name,
        &t.value,
    );
    TruckEntity {
        object: TruckObject::Text(strokes_2d),
        snap_pts: vec![(snap_pt, SnapHint::Insertion)],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

fn native_text_halign_str(code: i16) -> &'static str {
    match code {
        1 => "Center",
        2 => "Right",
        3 => "Aligned",
        4 => "Middle",
        5 => "Fit",
        _ => "Left",
    }
}

fn native_text_valign_str(code: i16) -> &'static str {
    match code {
        1 => "Bottom",
        2 => "Middle",
        3 => "Top",
        _ => "Baseline",
    }
}

fn native_text_halign_from_str(value: &str) -> i16 {
    match value {
        "Center" => 1,
        "Right" => 2,
        "Aligned" => 3,
        "Middle" => 4,
        "Fit" => 5,
        _ => 0,
    }
}

fn native_text_valign_from_str(value: &str) -> i16 {
    match value {
        "Bottom" => 1,
        "Middle" => 2,
        "Top" => 3,
        _ => 0,
    }
}

pub fn to_truck_native(
    insertion: &[f64; 3],
    height: f64,
    value: &str,
    rotation_deg: f64,
    style_name: &str,
    width_factor_value: f64,
    oblique_angle_deg: f64,
    horizontal_alignment: i16,
    vertical_alignment: i16,
    alignment_point: Option<[f64; 3]>,
    document: &nm::CadDocument,
) -> TruckEntity {
    let snap_pt = Vec3::new(insertion[0] as f32, insertion[1] as f32, insertion[2] as f32);
    let resolved_style = resolve_text_style_native(style_name, document);
    let font_name = resolved_style.font_name;
    let width_factor = ((if width_factor_value > 0.0 {
        width_factor_value as f32
    } else {
        1.0
    }) * resolved_style.width_factor.max(0.01))
    .clamp(0.01, 100.0);
    let oblique_angle = oblique_angle_deg as f32 + resolved_style.oblique_angle;
    let rot = rotation_deg.to_radians() as f32;
    let anchor = alignment_point
        .map(|point| [point[0] as f32, point[1] as f32])
        .unwrap_or([insertion[0] as f32, insertion[1] as f32]);
    let bounds = text_local_bounds(
        &font_name,
        value,
        height as f32,
        width_factor,
        oblique_angle,
    );
    let (anchor_local_x, anchor_local_y) = if let Some(([min_x, min_y], [max_x, max_y])) = bounds {
        let ax = match horizontal_alignment {
            1 | 4 => (min_x + max_x) * 0.5,
            2 | 3 | 5 => max_x,
            _ => min_x,
        };
        let ay = match vertical_alignment {
            1 => min_y,
            2 => (min_y + max_y) * 0.5,
            3 => max_y,
            _ => 0.0,
        };
        (ax, ay)
    } else {
        (0.0, 0.0)
    };
    let (cos_r, sin_r) = (rot.cos(), rot.sin());
    let origin = [
        anchor[0] - (anchor_local_x * cos_r - anchor_local_y * sin_r),
        anchor[1] - (anchor_local_x * sin_r + anchor_local_y * cos_r),
    ];
    let strokes_2d = cxf::tessellate_text_ex(
        origin,
        height as f32,
        rot,
        width_factor,
        oblique_angle,
        &font_name,
        value,
    );
    TruckEntity {
        object: TruckObject::Text(strokes_2d),
        snap_pts: vec![(snap_pt, SnapHint::Insertion)],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

fn grips(t: &Text) -> Vec<GripDef> {
    let p = Vec3::new(
        t.insertion_point.x as f32,
        t.insertion_point.y as f32,
        t.insertion_point.z as f32,
    );
    vec![square_grip(0, p)]
}

pub fn grips_native(insertion: &[f64; 3]) -> Vec<GripDef> {
    vec![square_grip(0, Vec3::new(
        insertion[0] as f32,
        insertion[1] as f32,
        insertion[2] as f32,
    ))]
}

fn properties(t: &Text, text_style_names: &[String]) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Insert X", "ins_x", t.insertion_point.x),
            edit("Insert Y", "ins_y", t.insertion_point.y),
            edit("Insert Z", "ins_z", t.insertion_point.z),
            edit("Height", "height", t.height),
            edit("Rotation", "rotation", t.rotation.to_degrees()),
            edit("Width Factor", "width_factor", t.width_factor),
            edit(
                "Oblique Angle",
                "oblique_angle",
                t.oblique_angle.to_degrees(),
            ),
            Property {
                label: "H-Align".into(),
                field: "h_align",
                value: PropValue::Choice {
                    selected: text_halign_str(&t.horizontal_alignment).to_string(),
                    options: ["Left", "Center", "Right", "Aligned", "Middle", "Fit"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "V-Align".into(),
                field: "v_align",
                value: PropValue::Choice {
                    selected: text_valign_str(&t.vertical_alignment).to_string(),
                    options: ["Baseline", "Bottom", "Middle", "Top"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "Content".into(),
                field: "content",
                value: PropValue::EditText(t.value.clone()),
            },
            Property {
                label: "Style".into(),
                field: "style",
                value: PropValue::Choice {
                    selected: if t.style.trim().is_empty() {
                        "Standard".into()
                    } else {
                        t.style.clone()
                    },
                    options: text_style_names.to_vec(),
                },
            },
        ],
    }
}

pub fn properties_native(
    insertion: &[f64; 3],
    height: f64,
    value: &str,
    rotation_deg: f64,
    style_name: &str,
    width_factor: f64,
    oblique_angle: f64,
    horizontal_alignment: i16,
    vertical_alignment: i16,
    text_style_names: &[String],
) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Insert X", "ins_x", insertion[0]),
            edit("Insert Y", "ins_y", insertion[1]),
            edit("Insert Z", "ins_z", insertion[2]),
            edit("Height", "height", height),
            edit("Rotation", "rotation", rotation_deg),
            edit("Width Factor", "width_factor", width_factor),
            edit("Oblique Angle", "oblique_angle", oblique_angle),
            Property {
                label: "H-Align".into(),
                field: "h_align",
                value: PropValue::Choice {
                    selected: native_text_halign_str(horizontal_alignment).to_string(),
                    options: ["Left", "Center", "Right", "Aligned", "Middle", "Fit"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "V-Align".into(),
                field: "v_align",
                value: PropValue::Choice {
                    selected: native_text_valign_str(vertical_alignment).to_string(),
                    options: ["Baseline", "Bottom", "Middle", "Top"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "Content".into(),
                field: "content",
                value: PropValue::EditText(value.to_string()),
            },
            Property {
                label: "Style".into(),
                field: "style",
                value: PropValue::Choice {
                    selected: if style_name.trim().is_empty() {
                        "Standard".into()
                    } else {
                        style_name.to_string()
                    },
                    options: text_style_names.to_vec(),
                },
            },
        ],
    }
}

fn apply_geom_prop(t: &mut Text, field: &str, value: &str) {
    match field {
        "content" => {
            t.value = value.to_string();
            return;
        }
        "style" => {
            t.style = value.to_string();
            return;
        }
        "h_align" => {
            t.horizontal_alignment = match value {
                "Left" => HA::Left,
                "Center" => HA::Center,
                "Right" => HA::Right,
                "Aligned" => HA::Aligned,
                "Middle" => HA::Middle,
                "Fit" => HA::Fit,
                _ => return,
            };
            sync_text_alignment_point(t);
            return;
        }
        "v_align" => {
            t.vertical_alignment = match value {
                "Baseline" => VA::Baseline,
                "Bottom" => VA::Bottom,
                "Middle" => VA::Middle,
                "Top" => VA::Top,
                _ => return,
            };
            sync_text_alignment_point(t);
            return;
        }
        _ => {}
    }
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "ins_x" => t.insertion_point.x = v,
        "ins_y" => t.insertion_point.y = v,
        "ins_z" => t.insertion_point.z = v,
        "height" if v > 0.0 => t.height = v,
        "rotation" => t.rotation = v.to_radians(),
        "width_factor" if v > 0.0 => t.width_factor = v,
        "oblique_angle" => t.oblique_angle = v.to_radians(),
        _ => {}
    }
}

pub fn apply_geom_prop_native(
    insertion: &mut [f64; 3],
    height: &mut f64,
    value_text: &mut String,
    rotation_deg: &mut f64,
    style_name: &mut String,
    width_factor: &mut f64,
    oblique_angle: &mut f64,
    horizontal_alignment: &mut i16,
    vertical_alignment: &mut i16,
    field: &str,
    value: &str,
) {
    match field {
        "content" => {
            *value_text = value.to_string();
            return;
        }
        "style" => {
            *style_name = value.to_string();
            return;
        }
        "h_align" => {
            *horizontal_alignment = native_text_halign_from_str(value);
            return;
        }
        "v_align" => {
            *vertical_alignment = native_text_valign_from_str(value);
            return;
        }
        _ => {}
    }
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "ins_x" => insertion[0] = v,
        "ins_y" => insertion[1] = v,
        "ins_z" => insertion[2] = v,
        "height" if v > 0.0 => *height = v,
        "rotation" => *rotation_deg = v,
        "width_factor" if v > 0.0 => *width_factor = v,
        "oblique_angle" => *oblique_angle = v,
        _ => {}
    }
}

fn apply_grip(t: &mut Text, _grip_id: usize, apply: GripApply) {
    match apply {
        GripApply::Absolute(p) => {
            t.insertion_point.x = p.x as f64;
            t.insertion_point.y = p.y as f64;
            t.insertion_point.z = p.z as f64;
        }
        GripApply::Translate(d) => {
            t.insertion_point.x += d.x as f64;
            t.insertion_point.y += d.y as f64;
            t.insertion_point.z += d.z as f64;
        }
    }
}

pub fn apply_grip_native(insertion: &mut [f64; 3], _grip_id: usize, apply: GripApply) {
    match apply {
        GripApply::Absolute(p) => {
            insertion[0] = p.x as f64;
            insertion[1] = p.y as f64;
            insertion[2] = p.z as f64;
        }
        GripApply::Translate(d) => {
            insertion[0] += d.x as f64;
            insertion[1] += d.y as f64;
            insertion[2] += d.z as f64;
        }
    }
}

fn apply_transform(t: &mut Text, tr: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(t, tr, |entity, p1, p2| {
        crate::scene::transform::reflect_xy_point(
            &mut entity.insertion_point.x,
            &mut entity.insertion_point.y,
            p1,
            p2,
        );
        if let Some(ref mut a) = entity.alignment_point {
            crate::scene::transform::reflect_xy_point(&mut a.x, &mut a.y, p1, p2);
        }
        let dx = (p2.x - p1.x) as f64;
        let dy = (p2.y - p1.y) as f64;
        let line_angle = dy.atan2(dx);
        entity.rotation = 2.0 * line_angle - entity.rotation;
        entity.oblique_angle = -entity.oblique_angle;
    });
}

pub fn apply_transform_native(
    insertion: &mut [f64; 3],
    rotation_deg: &mut f64,
    tr: &EntityTransform,
) {
    crate::entities::common::transform_pt(insertion, tr);
    match tr {
        EntityTransform::Rotate { angle_rad, .. } => {
            *rotation_deg += (*angle_rad as f64).to_degrees();
        }
        EntityTransform::Mirror { p1, p2 } => {
            let dx = (p2.x - p1.x) as f64;
            let dy = (p2.y - p1.y) as f64;
            let line_angle_deg = dy.atan2(dx).to_degrees();
            *rotation_deg = 2.0 * line_angle_deg - *rotation_deg;
        }
        _ => {}
    }
}

impl TruckConvertible for Text {
    fn to_truck(&self, document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self, document))
    }
}

impl Grippable for Text {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Text {
    fn geometry_properties(&self, text_style_names: &[String]) -> PropSection {
        properties(self, text_style_names)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Text {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
