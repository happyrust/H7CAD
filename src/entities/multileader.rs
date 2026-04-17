use acadrust::entities::{LeaderContentType, MultiLeader, MultiLeaderPathType, TextAttachmentType};
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{diamond_grip, edit_prop as edit, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection, PropValue, Property};
use crate::scene::wire_model::TangentGeom;

// ── TruckConvertible ────────────────────────────────────────────────────────

fn to_truck(ml: &MultiLeader, document: &acadrust::CadDocument) -> Option<TruckEntity> {
    let nan = [f32::NAN; 3];
    let p3 = |v: &crate::types::Vector3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };

    let arrow_size = ml.arrowhead_size as f32;
    let draw_arrow = arrow_size > 0.0;
    let invisible = ml.path_type == MultiLeaderPathType::Invisible;

    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut tangents: Vec<TangentGeom> = Vec::new();
    let mut key_verts: Vec<[f32; 3]> = Vec::new();
    let mut first = true;

    for root in &ml.context.leader_roots {
        let cp = &root.connection_point;
        let cp_f = p3(cp);

        for line in &root.lines {
            if line.points.is_empty() { continue; }

            if !invisible {
                if !first { points.push(nan); }
                first = false;

                for p in &line.points {
                    points.push(p3(p));
                    key_verts.push(p3(p));
                }

                // Closing segment: last bend → connection_point
                let last = line.points.last().unwrap();
                let last_f = p3(last);
                let dist = ((last_f[0]-cp_f[0]).powi(2) + (last_f[1]-cp_f[1]).powi(2)).sqrt();
                if dist > 1e-9 {
                    points.push(cp_f);
                    key_verts.push(cp_f);
                }

                for i in 0..line.points.len().saturating_sub(1) {
                    tangents.push(TangentGeom::Line {
                        p1: p3(&line.points[i]),
                        p2: p3(&line.points[i + 1]),
                    });
                }
            }

            // Arrowhead
            if draw_arrow {
                let tip = &line.points[0];
                let tip_f = p3(tip);
                let next = if line.points.len() >= 2 { line.points[1] } else { *cp };
                let dx = (next.x - tip.x) as f32;
                let dy = (next.y - tip.y) as f32;
                let dl = (dx*dx + dy*dy).sqrt().max(1e-9);
                let (dx, dy) = (dx / dl, dy / dl);
                let a = std::f32::consts::PI / 6.0;
                let (s, c) = a.sin_cos();
                points.push(nan);
                points.push([tip_f[0]+(dx*c-dy*s)*arrow_size, tip_f[1]+(dx*s+dy*c)*arrow_size, tip_f[2]]);
                points.push(tip_f);
                points.push([tip_f[0]+(dx*c+dy*s)*arrow_size, tip_f[1]+(-dx*s+dy*c)*arrow_size, tip_f[2]]);
            }
        }

        // Landing shelf at connection_point
        if ml.enable_landing && ml.enable_dogleg && ml.dogleg_length > 0.0 {
            let dir = &root.direction;
            let dl = (dir.x*dir.x + dir.y*dir.y).sqrt().max(1e-9);
            let d = ml.dogleg_length;
            points.push(nan);
            points.push(cp_f);
            points.push([(cp.x + dir.x/dl*d) as f32, (cp.y + dir.y/dl*d) as f32, cp.z as f32]);
        }
    }

    // Text strokes (MText content rendered inline)
    if ml.content_type == LeaderContentType::MText && !ml.context.text_string.is_empty() {
        let height = if ml.context.text_height > 0.0 { ml.context.text_height } else { ml.text_height };
        let ins = &ml.context.text_location;
        let z = ins.z as f32;
        let style = crate::entities::text_support::resolve_text_style("STANDARD", document);
        let strokes = crate::scene::cxf::tessellate_text_ex(
            [ins.x as f32, ins.y as f32],
            height as f32,
            0.0,
            style.width_factor.max(0.01),
            style.oblique_angle,
            &style.font_name,
            &ml.context.text_string,
        );
        for stroke in &strokes {
            if stroke.len() < 2 { continue; }
            points.push(nan);
            for &[x, y] in stroke {
                points.push([x, y, z]);
            }
        }
    }

    if points.is_empty() { return None; }

    Some(TruckEntity {
        object: TruckObject::Lines(points),
        snap_pts: vec![],
        tangent_geoms: tangents,
        key_vertices: key_verts,
    })
}

// ── Grips ──────────────────────────────────────────────────────────────────
//
// IDs are assigned in two passes:
//   0 .. total_line_pts - 1  : square grips on every LeaderLine vertex
//   total_line_pts            : diamond grip on context.text_location (if MText)

fn grips(ml: &MultiLeader) -> Vec<GripDef> {
    let mut result: Vec<GripDef> = Vec::new();
    let mut id = 0usize;

    for root in &ml.context.leader_roots {
        for line in &root.lines {
            for p in &line.points {
                result.push(square_grip(id, Vec3::new(p.x as f32, p.y as f32, p.z as f32)));
                id += 1;
            }
        }
    }

    if ml.content_type == LeaderContentType::MText {
        let tl = &ml.context.text_location;
        result.push(diamond_grip(id, Vec3::new(tl.x as f32, tl.y as f32, tl.z as f32)));
    }

    result
}

fn apply_grip(ml: &mut MultiLeader, grip_id: usize, apply: GripApply) {
    let mut idx = 0usize;

    for root in &mut ml.context.leader_roots {
        for line in &mut root.lines {
            for p in &mut line.points {
                if idx == grip_id {
                    match apply {
                        GripApply::Absolute(a) => {
                            p.x = a.x as f64;
                            p.y = a.y as f64;
                            p.z = a.z as f64;
                        }
                        GripApply::Translate(d) => {
                            p.x += d.x as f64;
                            p.y += d.y as f64;
                            p.z += d.z as f64;
                        }
                    }
                    return;
                }
                idx += 1;
            }
        }
    }

    // Text location grip
    if ml.content_type == LeaderContentType::MText && idx == grip_id {
        let tl = &mut ml.context.text_location;
        match apply {
            GripApply::Absolute(a) => {
                tl.x = a.x as f64;
                tl.y = a.y as f64;
                tl.z = a.z as f64;
            }
            GripApply::Translate(d) => {
                tl.x += d.x as f64;
                tl.y += d.y as f64;
                tl.z += d.z as f64;
            }
        }
    }
}

// ── Properties ─────────────────────────────────────────────────────────────

fn content_type_str(ct: &LeaderContentType) -> &'static str {
    match ct {
        LeaderContentType::None => "None",
        LeaderContentType::Block => "Block",
        LeaderContentType::MText => "MText",
        LeaderContentType::Tolerance => "Tolerance",
    }
}

fn path_type_str(pt: &MultiLeaderPathType) -> &'static str {
    match pt {
        MultiLeaderPathType::Invisible => "Invisible",
        MultiLeaderPathType::StraightLineSegments => "Straight",
        MultiLeaderPathType::Spline => "Spline",
    }
}

fn attachment_str(a: &TextAttachmentType) -> &'static str {
    match a {
        TextAttachmentType::TopOfTopLine => "Top of Top",
        TextAttachmentType::MiddleOfTopLine => "Mid of Top",
        TextAttachmentType::MiddleOfText => "Mid of Text",
        TextAttachmentType::MiddleOfBottomLine => "Mid of Bot",
        TextAttachmentType::BottomOfBottomLine => "Bot of Bot",
        TextAttachmentType::BottomLine => "Bottom Line",
        _ => "Other",
    }
}

fn bool_toggle(label: &str, field: &'static str, value: bool) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::BoolToggle { field, value },
    }
}

fn choice(label: &str, field: &'static str, selected: &str, opts: &[&str]) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::Choice {
            selected: selected.to_string(),
            options: opts.iter().map(|s| s.to_string()).collect(),
        },
    }
}

fn properties(ml: &MultiLeader) -> PropSection {
    let ctx = &ml.context;
    let total_pts: usize = ctx
        .leader_roots
        .iter()
        .flat_map(|r| r.lines.iter())
        .map(|l| l.points.len())
        .sum();

    let mut props = vec![
        // Content
        choice("Content Type", "content_type", content_type_str(&ml.content_type),
               &["None", "MText", "Block", "Tolerance"]),
        Property {
            label: "Text".into(),
            field: "text_string",
            value: PropValue::EditText(ctx.text_string.clone()),
        },
        edit("Text Height", "text_height", ml.text_height),
        edit("Text X", "text_x", ctx.text_location.x),
        edit("Text Y", "text_y", ctx.text_location.y),
        edit("Text Z", "text_z", ctx.text_location.z),
        bool_toggle("Text Frame", "text_frame", ml.text_frame),
        // Leader line
        choice("Path Type", "path_type", path_type_str(&ml.path_type),
               &["Straight", "Spline", "Invisible"]),
        bool_toggle("Landing", "enable_landing", ml.enable_landing),
        bool_toggle("Dogleg", "enable_dogleg", ml.enable_dogleg),
        edit("Dogleg Length", "dogleg_length", ml.dogleg_length),
        edit("Arrow Size", "arrowhead_size", ml.arrowhead_size),
        edit("Scale", "scale_factor", ml.scale_factor),
        bool_toggle("Annotation Scale", "enable_annotation_scale", ml.enable_annotation_scale),
        // Text attachment
        choice("Left Attach", "text_left_attachment",
               attachment_str(&ml.text_left_attachment),
               &["Top of Top","Mid of Top","Mid of Text","Mid of Bot","Bot of Bot","Bottom Line"]),
        choice("Right Attach", "text_right_attachment",
               attachment_str(&ml.text_right_attachment),
               &["Top of Top","Mid of Top","Mid of Text","Mid of Bot","Bot of Bot","Bottom Line"]),
        // Stats
        ro("Leader Pts", "total_pts", total_pts.to_string()),
        ro("Roots", "root_count", ctx.leader_roots.len().to_string()),
    ];

    // Connection point for first root (most common case)
    if let Some(root) = ctx.leader_roots.first() {
        props.push(edit("Root Conn X", "conn_x", root.connection_point.x));
        props.push(edit("Root Conn Y", "conn_y", root.connection_point.y));
        props.push(edit("Root Conn Z", "conn_z", root.connection_point.z));
    }

    PropSection { title: "Geometry".into(), props }
}

fn apply_geom_prop(ml: &mut MultiLeader, field: &str, value: &str) {
    let f64 = |s: &str| -> Option<f64> { s.trim().parse().ok() };

    match field {
        "content_type" => {
            ml.content_type = match value {
                "Block" => LeaderContentType::Block,
                "MText" => LeaderContentType::MText,
                "Tolerance" => LeaderContentType::Tolerance,
                _ => LeaderContentType::None,
            };
        }
        "text_string" => ml.context.text_string = value.to_string(),
        "text_height" => {
            if let Some(v) = f64(value) { ml.text_height = v; ml.context.text_height = v; }
        }
        "text_x" => { if let Some(v) = f64(value) { ml.context.text_location.x = v; } }
        "text_y" => { if let Some(v) = f64(value) { ml.context.text_location.y = v; } }
        "text_z" => { if let Some(v) = f64(value) { ml.context.text_location.z = v; } }
        "text_frame" => ml.text_frame = if value == "toggle" { !ml.text_frame } else { value == "true" },
        "path_type" => {
            ml.path_type = match value {
                "Spline" => MultiLeaderPathType::Spline,
                "Invisible" => MultiLeaderPathType::Invisible,
                _ => MultiLeaderPathType::StraightLineSegments,
            };
        }
        "enable_landing" => ml.enable_landing = if value == "toggle" { !ml.enable_landing } else { value == "true" },
        "enable_dogleg" => ml.enable_dogleg = if value == "toggle" { !ml.enable_dogleg } else { value == "true" },
        "enable_annotation_scale" => ml.enable_annotation_scale = if value == "toggle" { !ml.enable_annotation_scale } else { value == "true" },
        "dogleg_length" => { if let Some(v) = f64(value) { ml.dogleg_length = v; } }
        "arrowhead_size" => { if let Some(v) = f64(value) { ml.arrowhead_size = v; } }
        "scale_factor" => { if let Some(v) = f64(value) { ml.scale_factor = v; } }
        "conn_x" => {
            if let (Some(v), Some(root)) = (f64(value), ml.context.leader_roots.first_mut()) {
                root.connection_point.x = v;
            }
        }
        "conn_y" => {
            if let (Some(v), Some(root)) = (f64(value), ml.context.leader_roots.first_mut()) {
                root.connection_point.y = v;
            }
        }
        "conn_z" => {
            if let (Some(v), Some(root)) = (f64(value), ml.context.leader_roots.first_mut()) {
                root.connection_point.z = v;
            }
        }
        "text_left_attachment" => {
            ml.text_left_attachment = parse_attachment(value);
            ml.context.text_left_attachment = parse_attachment(value);
        }
        "text_right_attachment" => {
            ml.text_right_attachment = parse_attachment(value);
            ml.context.text_right_attachment = parse_attachment(value);
        }
        _ => {}
    }
}

fn parse_attachment(s: &str) -> TextAttachmentType {
    match s {
        "Top of Top" => TextAttachmentType::TopOfTopLine,
        "Mid of Top" => TextAttachmentType::MiddleOfTopLine,
        "Mid of Bot" => TextAttachmentType::MiddleOfBottomLine,
        "Bot of Bot" => TextAttachmentType::BottomOfBottomLine,
        "Bottom Line" => TextAttachmentType::BottomLine,
        _ => TextAttachmentType::MiddleOfText,
    }
}

// ── Transform ──────────────────────────────────────────────────────────────

fn apply_transform(ml: &mut MultiLeader, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(ml, t, |entity, p1, p2| {
        for root in &mut entity.context.leader_roots {
            for line in &mut root.lines {
                for p in &mut line.points {
                    crate::scene::transform::reflect_xy_point(&mut p.x, &mut p.y, p1, p2);
                }
            }
            crate::scene::transform::reflect_xy_point(
                &mut root.connection_point.x, &mut root.connection_point.y, p1, p2);
        }
        crate::scene::transform::reflect_xy_point(
            &mut entity.context.text_location.x, &mut entity.context.text_location.y, p1, p2);
    });
}

// ── Trait impls ────────────────────────────────────────────────────────────

impl TruckConvertible for MultiLeader {
    fn to_truck(&self, document: &acadrust::CadDocument) -> Option<TruckEntity> {
        to_truck(self, document)
    }
}

impl Grippable for MultiLeader {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for MultiLeader {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for MultiLeader {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
