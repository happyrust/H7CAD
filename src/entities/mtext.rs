use acadrust::entities::{AttachmentPoint, DrawingDirection, MText};
use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, ro_prop as ro, square_grip, triangle_grip};
use crate::entities::text_support::{
    resolve_text_style, resolve_text_style_native, split_mtext_lines, strip_mtext_codes, word_wrap,
};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::cxf;
use crate::scene::object::{GripApply, GripDef, PropSection, PropValue, Property};
use crate::scene::wire_model::SnapHint;

fn attachment_str(a: &AttachmentPoint) -> &'static str {
    match a {
        AttachmentPoint::TopLeft => "Top Left",
        AttachmentPoint::TopCenter => "Top Center",
        AttachmentPoint::TopRight => "Top Right",
        AttachmentPoint::MiddleLeft => "Middle Left",
        AttachmentPoint::MiddleCenter => "Middle Center",
        AttachmentPoint::MiddleRight => "Middle Right",
        AttachmentPoint::BottomLeft => "Bottom Left",
        AttachmentPoint::BottomCenter => "Bottom Center",
        AttachmentPoint::BottomRight => "Bottom Right",
    }
}

fn native_attachment_str(a: i16) -> &'static str {
    match a {
        1 => "Top Left",
        2 => "Top Center",
        3 => "Top Right",
        4 => "Middle Left",
        5 => "Middle Center",
        6 => "Middle Right",
        7 => "Bottom Left",
        8 => "Bottom Center",
        9 => "Bottom Right",
        _ => "Top Left",
    }
}

fn mtext_halign_str(a: &AttachmentPoint) -> &'static str {
    match a {
        AttachmentPoint::TopLeft | AttachmentPoint::MiddleLeft | AttachmentPoint::BottomLeft => {
            "Left"
        }
        AttachmentPoint::TopCenter
        | AttachmentPoint::MiddleCenter
        | AttachmentPoint::BottomCenter => "Center",
        AttachmentPoint::TopRight | AttachmentPoint::MiddleRight | AttachmentPoint::BottomRight => {
            "Right"
        }
    }
}

fn native_mtext_halign_str(a: i16) -> &'static str {
    match a {
        2 | 5 | 8 => "Center",
        3 | 6 | 9 => "Right",
        _ => "Left",
    }
}

fn mtext_valign_str(a: &AttachmentPoint) -> &'static str {
    match a {
        AttachmentPoint::TopLeft | AttachmentPoint::TopCenter | AttachmentPoint::TopRight => "Top",
        AttachmentPoint::MiddleLeft
        | AttachmentPoint::MiddleCenter
        | AttachmentPoint::MiddleRight => "Middle",
        AttachmentPoint::BottomLeft
        | AttachmentPoint::BottomCenter
        | AttachmentPoint::BottomRight => "Bottom",
    }
}

fn native_mtext_valign_str(a: i16) -> &'static str {
    match a {
        4..=6 => "Middle",
        7..=9 => "Bottom",
        _ => "Top",
    }
}

fn native_attachment_from_align(h: &str, v: &str) -> i16 {
    match (h, v) {
        ("Left", "Top") => 1,
        ("Center", "Top") => 2,
        ("Right", "Top") => 3,
        ("Left", "Middle") => 4,
        ("Center", "Middle") => 5,
        ("Right", "Middle") => 6,
        ("Left", "Bottom") => 7,
        ("Center", "Bottom") => 8,
        ("Right", "Bottom") => 9,
        _ => 1,
    }
}

fn mtext_attachment_from_align(h: &str, v: &str) -> Option<AttachmentPoint> {
    Some(match (h, v) {
        ("Left", "Top") => AttachmentPoint::TopLeft,
        ("Center", "Top") => AttachmentPoint::TopCenter,
        ("Right", "Top") => AttachmentPoint::TopRight,
        ("Left", "Middle") => AttachmentPoint::MiddleLeft,
        ("Center", "Middle") => AttachmentPoint::MiddleCenter,
        ("Right", "Middle") => AttachmentPoint::MiddleRight,
        ("Left", "Bottom") => AttachmentPoint::BottomLeft,
        ("Center", "Bottom") => AttachmentPoint::BottomCenter,
        ("Right", "Bottom") => AttachmentPoint::BottomRight,
        _ => return None,
    })
}

fn drawing_dir_str(d: &DrawingDirection) -> &'static str {
    match d {
        DrawingDirection::LeftToRight => "Left to Right",
        DrawingDirection::TopToBottom => "Top to Bottom",
        DrawingDirection::ByStyle => "By Style",
    }
}

fn to_truck(t: &MText, document: &acadrust::CadDocument) -> TruckEntity {
    let resolved_style = resolve_text_style(&t.style, document);
    let font_name = resolved_style.font_name;
    let font = cxf::get_font(&font_name);
    let style_width_factor = resolved_style.width_factor.max(0.01);
    let style_oblique = resolved_style.oblique_angle;
    let plain = strip_mtext_codes(&t.value);
    let explicit_lines = split_mtext_lines(&plain);
    let lines: Vec<String> = if t.rectangle_width > 0.0 {
        let scale = t.height as f32 / 9.0 * style_width_factor;
        let max_w = t.rectangle_width as f32;
        explicit_lines
            .iter()
            .flat_map(|line| word_wrap(line, max_w, scale, font))
            .collect()
    } else {
        explicit_lines
    };
    let n_lines = lines.len().max(1) as f32;
    let ls_factor = if t.line_spacing_factor > 0.0 {
        t.line_spacing_factor as f32
    } else {
        1.0
    };
    let line_h = t.height as f32 * ls_factor * font.line_spacing;
    let total_h = line_h * n_lines;
    let v_offset = match t.attachment_point {
        AttachmentPoint::TopLeft | AttachmentPoint::TopCenter | AttachmentPoint::TopRight => 0.0,
        AttachmentPoint::MiddleLeft
        | AttachmentPoint::MiddleCenter
        | AttachmentPoint::MiddleRight => -total_h * 0.5,
        AttachmentPoint::BottomLeft
        | AttachmentPoint::BottomCenter
        | AttachmentPoint::BottomRight => -total_h,
    };
    let h_anchor = match t.attachment_point {
        AttachmentPoint::TopCenter
        | AttachmentPoint::MiddleCenter
        | AttachmentPoint::BottomCenter => 0.5,
        AttachmentPoint::TopRight | AttachmentPoint::MiddleRight | AttachmentPoint::BottomRight => {
            1.0
        }
        _ => 0.0,
    };
    let vertical_text = matches!(t.drawing_direction, DrawingDirection::TopToBottom);
    let rot = t.rotation as f32;
    let (cos_r, sin_r) = (rot.cos(), rot.sin());
    let insertion = Vec3::new(
        t.insertion_point.x as f32,
        t.insertion_point.y as f32,
        t.insertion_point.z as f32,
    );
    let mut all_strokes = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let li = i as f32;
        let (ox, oy) = if vertical_text {
            let col_offset = li * t.height as f32 * 1.2;
            (
                t.insertion_point.x as f32 + col_offset * cos_r + v_offset * (-sin_r),
                t.insertion_point.y as f32 + col_offset * sin_r + v_offset * cos_r,
            )
        } else {
            let line_y = -(li * line_h) + v_offset;
            (
                t.insertion_point.x as f32 + line_y * (-sin_r),
                t.insertion_point.y as f32 + line_y * cos_r,
            )
        };
        let line_w = if h_anchor > 0.0 {
            let scale = t.height as f32 / 9.0 * style_width_factor;
            line.chars()
                .map(|c| {
                    if c == ' ' {
                        return font.word_spacing * scale;
                    }
                    font.glyph(c)
                        .map(|g| (g.advance + font.letter_spacing) * scale)
                        .unwrap_or(t.height as f32 * 0.6)
                })
                .sum()
        } else {
            0.0
        };
        let h_shift = -line_w * h_anchor;
        let origin_x = ox + h_shift * cos_r;
        let origin_y = oy + h_shift * sin_r;
        let strokes = cxf::tessellate_text_ex(
            [origin_x, origin_y],
            t.height as f32,
            rot,
            style_width_factor,
            style_oblique,
            &font_name,
            line,
        );
        all_strokes.extend(strokes);
    }
    TruckEntity {
        object: TruckObject::Text(all_strokes),
        snap_pts: vec![(insertion, SnapHint::Insertion)],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

pub fn to_truck_native(
    insertion: &[f64; 3],
    height: f64,
    width: f64,
    rectangle_height: Option<f64>,
    value: &str,
    rotation_deg: f64,
    style_name: &str,
    attachment_point: i16,
    line_spacing_factor: f64,
    drawing_direction: i16,
    document: &nm::CadDocument,
) -> TruckEntity {
    let resolved_style = resolve_text_style_native(style_name, document);
    let font_name = resolved_style.font_name;
    let font = cxf::get_font(&font_name);
    let style_width_factor = resolved_style.width_factor.max(0.01);
    let style_oblique = resolved_style.oblique_angle;
    let plain = strip_mtext_codes(value);
    let explicit_lines = split_mtext_lines(&plain);
    let lines: Vec<String> = if width > 0.0 {
        let scale = height as f32 / 9.0 * style_width_factor;
        let max_w = width as f32;
        explicit_lines
            .iter()
            .flat_map(|line| word_wrap(line, max_w, scale, font))
            .collect()
    } else {
        explicit_lines
    };
    let ls_factor = if line_spacing_factor > 0.0 {
        line_spacing_factor as f32
    } else {
        1.0
    };
    let line_h = height as f32 * ls_factor * font.line_spacing;
    let total_h = rectangle_height.map(|v| v as f32).unwrap_or(line_h * lines.len().max(1) as f32);
    let v_offset = match attachment_point {
        4..=6 => -total_h * 0.5,
        7..=9 => -total_h,
        _ => 0.0,
    };
    let h_anchor = match attachment_point {
        2 | 5 | 8 => 0.5,
        3 | 6 | 9 => 1.0,
        _ => 0.0,
    };
    let vertical_text = drawing_direction == 3;
    let rot = rotation_deg.to_radians() as f32;
    let (cos_r, sin_r) = (rot.cos(), rot.sin());
    let insertion_vec = Vec3::new(insertion[0] as f32, insertion[1] as f32, insertion[2] as f32);
    let mut all_strokes = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let (ox, oy) = if vertical_text {
            let col_offset = i as f32 * height as f32 * 1.2;
            (
                insertion[0] as f32 + col_offset * cos_r + v_offset * (-sin_r),
                insertion[1] as f32 + col_offset * sin_r + v_offset * cos_r,
            )
        } else {
            let line_y = -(i as f32 * line_h) + v_offset;
            (
                insertion[0] as f32 + line_y * (-sin_r),
                insertion[1] as f32 + line_y * cos_r,
            )
        };
        let line_w = if h_anchor > 0.0 {
            let scale = height as f32 / 9.0 * style_width_factor;
            line.chars()
                .map(|c| {
                    if c == ' ' {
                        return font.word_spacing * scale;
                    }
                    font.glyph(c)
                        .map(|g| (g.advance + font.letter_spacing) * scale)
                        .unwrap_or(height as f32 * 0.6)
                })
                .sum()
        } else {
            0.0
        };
        let h_shift = -line_w * h_anchor;
        let origin_x = ox + h_shift * cos_r;
        let origin_y = oy + h_shift * sin_r;
        let strokes = cxf::tessellate_text_ex(
            [origin_x, origin_y],
            height as f32,
            rot,
            style_width_factor,
            style_oblique,
            &font_name,
            line,
        );
        all_strokes.extend(strokes);
    }
    TruckEntity {
        object: TruckObject::Text(all_strokes),
        snap_pts: vec![(insertion_vec, SnapHint::Insertion)],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

fn grips(t: &MText) -> Vec<GripDef> {
    let p = Vec3::new(
        t.insertion_point.x as f32,
        t.insertion_point.y as f32,
        t.insertion_point.z as f32,
    );
    let dir = Vec3::new((t.rotation as f32).cos(), (t.rotation as f32).sin(), 0.0);
    let width_grip = p + dir * t.rectangle_width.max(0.0) as f32;
    vec![square_grip(0, p), triangle_grip(1, width_grip)]
}

pub fn grips_native(insertion: &[f64; 3], width: f64, rotation_deg: f64) -> Vec<GripDef> {
    let p = Vec3::new(insertion[0] as f32, insertion[1] as f32, insertion[2] as f32);
    let rot = rotation_deg.to_radians() as f32;
    let dir = Vec3::new(rot.cos(), rot.sin(), 0.0);
    let width_grip = p + dir * width.max(0.0) as f32;
    vec![square_grip(0, p), triangle_grip(1, width_grip)]
}

fn properties(t: &MText, text_style_names: &[String]) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Insert X", "ins_x", t.insertion_point.x),
            edit("Insert Y", "ins_y", t.insertion_point.y),
            edit("Insert Z", "ins_z", t.insertion_point.z),
            edit("Height", "height", t.height),
            edit("Width", "rect_w", t.rectangle_width),
            edit("Rect Height", "rect_h", t.rectangle_height.unwrap_or(0.0)),
            edit("Rotation", "rotation", t.rotation.to_degrees()),
            edit("Line Spacing", "line_spacing", t.line_spacing_factor),
            Property {
                label: "H-Align".into(),
                field: "h_align",
                value: PropValue::Choice {
                    selected: mtext_halign_str(&t.attachment_point).to_string(),
                    options: ["Left", "Center", "Right"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "V-Align".into(),
                field: "v_align",
                value: PropValue::Choice {
                    selected: mtext_valign_str(&t.attachment_point).to_string(),
                    options: ["Top", "Middle", "Bottom"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            ro(
                "Attachment",
                "attachment",
                attachment_str(&t.attachment_point).to_string(),
            ),
            ro(
                "Direction",
                "direction",
                drawing_dir_str(&t.drawing_direction).to_string(),
            ),
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
    width: f64,
    rectangle_height: Option<f64>,
    value: &str,
    rotation_deg: f64,
    style_name: &str,
    attachment_point: i16,
    line_spacing_factor: f64,
    drawing_direction: i16,
    text_style_names: &[String],
) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Insert X", "ins_x", insertion[0]),
            edit("Insert Y", "ins_y", insertion[1]),
            edit("Insert Z", "ins_z", insertion[2]),
            edit("Height", "height", height),
            edit("Width", "rect_w", width),
            edit("Rect Height", "rect_h", rectangle_height.unwrap_or(0.0)),
            edit("Rotation", "rotation", rotation_deg),
            edit("Line Spacing", "line_spacing", line_spacing_factor),
            Property {
                label: "H-Align".into(),
                field: "h_align",
                value: PropValue::Choice {
                    selected: native_mtext_halign_str(attachment_point).to_string(),
                    options: ["Left", "Center", "Right"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            Property {
                label: "V-Align".into(),
                field: "v_align",
                value: PropValue::Choice {
                    selected: native_mtext_valign_str(attachment_point).to_string(),
                    options: ["Top", "Middle", "Bottom"]
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                },
            },
            ro("Attachment", "attachment", native_attachment_str(attachment_point).to_string()),
            ro(
                "Direction",
                "direction",
                match drawing_direction {
                    3 => "Top to Bottom".to_string(),
                    5 => "By Style".to_string(),
                    _ => "Left to Right".to_string(),
                },
            ),
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

fn apply_geom_prop(t: &mut MText, field: &str, value: &str) {
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
            if let Some(next) =
                mtext_attachment_from_align(value, mtext_valign_str(&t.attachment_point))
            {
                t.attachment_point = next;
            }
            return;
        }
        "v_align" => {
            if let Some(next) =
                mtext_attachment_from_align(mtext_halign_str(&t.attachment_point), value)
            {
                t.attachment_point = next;
            }
            return;
        }
        _ => {}
    }
    let Some(v) = crate::entities::common::parse_f64(value) else {
        return;
    };
    match field {
        "ins_x" => t.insertion_point.x = v,
        "ins_y" => t.insertion_point.y = v,
        "ins_z" => t.insertion_point.z = v,
        "height" if v > 0.0 => t.height = v,
        "rect_w" if v > 0.0 => t.rectangle_width = v,
        "rect_h" if v > 0.0 => t.rectangle_height = Some(v),
        "rotation" => t.rotation = v.to_radians(),
        "line_spacing" if v > 0.0 => t.line_spacing_factor = v,
        _ => {}
    }
}

pub fn apply_geom_prop_native(
    insertion: &mut [f64; 3],
    height: &mut f64,
    width: &mut f64,
    rectangle_height: &mut Option<f64>,
    value_text: &mut String,
    rotation_deg: &mut f64,
    style_name: &mut String,
    attachment_point: &mut i16,
    line_spacing_factor: &mut f64,
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
            *attachment_point = native_attachment_from_align(value, native_mtext_valign_str(*attachment_point));
            return;
        }
        "v_align" => {
            *attachment_point = native_attachment_from_align(native_mtext_halign_str(*attachment_point), value);
            return;
        }
        _ => {}
    }
    let Some(v) = crate::entities::common::parse_f64(value) else {
        return;
    };
    match field {
        "ins_x" => insertion[0] = v,
        "ins_y" => insertion[1] = v,
        "ins_z" => insertion[2] = v,
        "height" if v > 0.0 => *height = v,
        "rect_w" if v > 0.0 => *width = v,
        "rect_h" if v > 0.0 => *rectangle_height = Some(v),
        "rotation" => *rotation_deg = v,
        "line_spacing" if v > 0.0 => *line_spacing_factor = v,
        _ => {}
    }
}

fn apply_grip(t: &mut MText, grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            t.insertion_point.x = p.x as f64;
            t.insertion_point.y = p.y as f64;
            t.insertion_point.z = p.z as f64;
        }
        (0, GripApply::Translate(d)) => {
            t.insertion_point.x += d.x as f64;
            t.insertion_point.y += d.y as f64;
            t.insertion_point.z += d.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            let dir_x = t.rotation.cos();
            let dir_y = t.rotation.sin();
            let dx = p.x as f64 - t.insertion_point.x;
            let dy = p.y as f64 - t.insertion_point.y;
            let projected = dx * dir_x + dy * dir_y;
            t.rectangle_width = projected.max(0.01);
        }
        _ => {}
    }
}

pub fn apply_grip_native(
    insertion: &mut [f64; 3],
    width: &mut f64,
    rotation_deg: f64,
    grip_id: usize,
    apply: GripApply,
) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            insertion[0] = p.x as f64;
            insertion[1] = p.y as f64;
            insertion[2] = p.z as f64;
        }
        (0, GripApply::Translate(d)) => {
            insertion[0] += d.x as f64;
            insertion[1] += d.y as f64;
            insertion[2] += d.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            let rot = rotation_deg.to_radians();
            let dir_x = rot.cos();
            let dir_y = rot.sin();
            let dx = p.x as f64 - insertion[0];
            let dy = p.y as f64 - insertion[1];
            let projected = dx * dir_x + dy * dir_y;
            *width = projected.max(0.01);
        }
        _ => {}
    }
}

fn apply_transform(t: &mut MText, tr: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(t, tr, |entity, p1, p2| {
        crate::scene::transform::reflect_xy_point(
            &mut entity.insertion_point.x,
            &mut entity.insertion_point.y,
            p1,
            p2,
        );
        let dx = (p2.x - p1.x) as f64;
        let dy = (p2.y - p1.y) as f64;
        let line_angle = dy.atan2(dx);
        entity.rotation = 2.0 * line_angle - entity.rotation;
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

impl TruckConvertible for MText {
    fn to_truck(&self, document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self, document))
    }
}

impl Grippable for MText {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for MText {
    fn geometry_properties(&self, text_style_names: &[String]) -> PropSection {
        properties(self, text_style_names)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for MText {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
