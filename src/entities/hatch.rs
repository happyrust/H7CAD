use acadrust::entities::{BoundaryEdge, Hatch};

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, ro_prop as ro};
use crate::entities::traits::{PropertyEditable, Transformable};
use crate::scene::object::{PropSection, PropValue, Property};

fn properties(h: &Hatch) -> PropSection {
    let pattern_type = match h.pattern_type {
        acadrust::entities::HatchPatternType::Predefined => "Predefined",
        acadrust::entities::HatchPatternType::UserDefined => "User Defined",
        acadrust::entities::HatchPatternType::Custom => "Custom",
    };
    let style = match h.style {
        acadrust::entities::HatchStyleType::Normal => "Normal",
        acadrust::entities::HatchStyleType::Outer => "Outer",
        acadrust::entities::HatchStyleType::Ignore => "Ignore",
    };
    let fill_type = if h.gradient_color.enabled {
        format!("Gradient ({})", h.gradient_color.name)
    } else if h.is_solid {
        "Solid".into()
    } else {
        format!("Pattern ({})", h.pattern.name)
    };
    let boundary_count: usize = h
        .paths
        .iter()
        .map(|p| {
            p.edges
                .iter()
                .map(|e| match e {
                    BoundaryEdge::Polyline(poly) => poly.vertices.len(),
                    _ => 1,
                })
                .sum::<usize>()
        })
        .sum();
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Fill Type", "fill_type", fill_type),
            Property {
                label: "Pattern Name".into(),
                field: "pattern_name",
                value: PropValue::HatchPatternChoice(h.pattern.name.clone()),
            },
            ro("Pattern Type", "pattern_type", pattern_type),
            edit(
                "Pattern Angle",
                "pattern_angle",
                h.pattern_angle.to_degrees(),
            ),
            edit("Pattern Scale", "pattern_scale", h.pattern_scale),
            ro("Style", "style", style),
            ro("Boundary Paths", "path_count", h.paths.len().to_string()),
            ro("Boundary Verts", "vert_count", boundary_count.to_string()),
            ro("Double", "double", if h.is_double { "Yes" } else { "No" }),
            ro(
                "Associative",
                "associative",
                if h.is_associative { "Yes" } else { "No" },
            ),
            edit("Elevation", "elevation", h.elevation),
        ],
    }
}

fn apply_geom_prop(h: &mut Hatch, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "pattern_angle" => h.pattern_angle = v.to_radians(),
        "pattern_scale" if v > 0.0 => h.pattern_scale = v,
        "elevation" => h.elevation = v,
        _ => {}
    }
}

fn apply_transform(h: &mut Hatch, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(h, t, |entity, p1, p2| {
        let dx = (p2.x - p1.x) as f64;
        let dy = (p2.y - p1.y) as f64;
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-12 {
            return;
        }
        let line_angle_deg = dy.atan2(dx).to_degrees();
        for path in &mut entity.paths {
            for edge in &mut path.edges {
                match edge {
                    BoundaryEdge::Line(l) => {
                        crate::scene::transform::reflect_xy_point(
                            &mut l.start.x,
                            &mut l.start.y,
                            p1,
                            p2,
                        );
                        crate::scene::transform::reflect_xy_point(
                            &mut l.end.x,
                            &mut l.end.y,
                            p1,
                            p2,
                        );
                    }
                    BoundaryEdge::CircularArc(a) => {
                        crate::scene::transform::reflect_xy_point(
                            &mut a.center.x,
                            &mut a.center.y,
                            p1,
                            p2,
                        );
                        let tmp = a.start_angle;
                        a.start_angle = 2.0 * line_angle_deg - a.end_angle;
                        a.end_angle = 2.0 * line_angle_deg - tmp;
                    }
                    BoundaryEdge::EllipticArc(e) => {
                        crate::scene::transform::reflect_xy_point(
                            &mut e.center.x,
                            &mut e.center.y,
                            p1,
                            p2,
                        );
                        let ax = dx;
                        let ay = dy;
                        let rx = e.major_axis_endpoint.x;
                        let ry = e.major_axis_endpoint.y;
                        let dot = rx * ax + ry * ay;
                        e.major_axis_endpoint.x = 2.0 * dot * ax / len2 - rx;
                        e.major_axis_endpoint.y = 2.0 * dot * ay / len2 - ry;
                        let tmp = e.start_angle;
                        e.start_angle = 2.0 * line_angle_deg - e.end_angle;
                        e.end_angle = 2.0 * line_angle_deg - tmp;
                    }
                    BoundaryEdge::Spline(s) => {
                        for cp in &mut s.control_points {
                            crate::scene::transform::reflect_xy_point(&mut cp.x, &mut cp.y, p1, p2);
                        }
                        for fp in &mut s.fit_points {
                            crate::scene::transform::reflect_xy_point(&mut fp.x, &mut fp.y, p1, p2);
                        }
                    }
                    BoundaryEdge::Polyline(p) => {
                        for v in &mut p.vertices {
                            crate::scene::transform::reflect_xy_point(&mut v.x, &mut v.y, p1, p2);
                        }
                    }
                }
            }
        }
    });
}

impl PropertyEditable for Hatch {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Hatch {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
