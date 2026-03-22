use acadrust::entities::Ellipse;
use glam::Vec3;
use truck_modeling::{builder, BSplineCurve, Curve, Edge, KnotVec, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{diamond_grip, edit_prop as edit, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

const TAU: f64 = std::f64::consts::TAU;

fn to_truck(ell: &Ellipse) -> TruckEntity {
    let cx = ell.center.x;
    let cy = ell.center.y;
    let cz = ell.center.z;
    let maj = glam::Vec3::new(
        ell.major_axis.x as f32,
        ell.major_axis.y as f32,
        ell.major_axis.z as f32,
    );
    let r_major = maj.length() as f64;
    let r_minor = r_major * ell.minor_axis_ratio;
    let t0 = ell.start_parameter;
    let mut t1 = ell.end_parameter;
    if t1 <= t0 {
        t1 += TAU;
    }
    let u = if r_major > 1e-9 {
        glam::Vec3::from(maj) / maj.length()
    } else {
        glam::Vec3::X
    };
    let v_axis = glam::Vec3::Z.cross(u);
    let center_v3 = Vec3::new(cx as f32, cy as f32, cz as f32);
    let is_closed = (t1 - t0 - TAU).abs() < 1e-6;

    if is_closed {
        let n = 16usize;
        let pts_upper: Vec<Point3> = (0..=n)
            .map(|i| {
                let t = (i as f64 / n as f64) * std::f64::consts::PI;
                let lx = (r_major * t.cos()) as f32;
                let lz = (r_minor * t.sin()) as f32;
                Point3::new(
                    cx + (lx * u.x + lz * v_axis.x) as f64,
                    cy + (lx * u.y + lz * v_axis.y) as f64,
                    cz + (lx * u.z + lz * v_axis.z) as f64,
                )
            })
            .collect();
        let pts_lower: Vec<Point3> = (0..=n)
            .map(|i| {
                let t = std::f64::consts::PI + (i as f64 / n as f64) * std::f64::consts::PI;
                let lx = (r_major * t.cos()) as f32;
                let lz = (r_minor * t.sin()) as f32;
                Point3::new(
                    cx + (lx * u.x + lz * v_axis.x) as f64,
                    cy + (lx * u.y + lz * v_axis.y) as f64,
                    cz + (lx * u.z + lz * v_axis.z) as f64,
                )
            })
            .collect();
        let v_pos = builder::vertex(*pts_upper.first().unwrap());
        let v_neg = builder::vertex(*pts_upper.last().unwrap());
        let kv_u = KnotVec::uniform_knot(1, n);
        let kv_l = KnotVec::uniform_knot(1, n);
        let spl_u = BSplineCurve::new(kv_u, pts_upper);
        let spl_l = BSplineCurve::new(kv_l, pts_lower);
        let edge_upper = Edge::new(&v_pos, &v_neg, Curve::BSplineCurve(spl_u));
        let edge_lower = Edge::new(&v_neg, &v_pos, Curve::BSplineCurve(spl_l));
        let wire: Wire = [edge_upper, edge_lower].into_iter().collect();
        TruckEntity {
            object: TruckObject::Contour(wire),
            snap_pts: vec![(center_v3, SnapHint::Center)],
            tangent_geoms: vec![],
            key_vertices: vec![],
        }
    } else {
        let n = 32usize;
        let ctrl_pts: Vec<Point3> = (0..=n)
            .map(|i| {
                let t = t0 + (t1 - t0) * (i as f64 / n as f64);
                let lx = (r_major * t.cos()) as f32;
                let lz = (r_minor * t.sin()) as f32;
                Point3::new(
                    cx + (lx * u.x + lz * v_axis.x) as f64,
                    cy + (lx * u.y + lz * v_axis.y) as f64,
                    cz + (lx * u.z + lz * v_axis.z) as f64,
                )
            })
            .collect();
        let kv = KnotVec::uniform_knot(1, n);
        let bspline = BSplineCurve::new(kv, ctrl_pts.clone());
        let v_start = builder::vertex(*ctrl_pts.first().unwrap());
        let v_end = builder::vertex(*ctrl_pts.last().unwrap());
        let edge = Edge::new(&v_start, &v_end, Curve::BSplineCurve(bspline));
        TruckEntity {
            object: TruckObject::Curve(edge),
            snap_pts: vec![(center_v3, SnapHint::Center)],
            tangent_geoms: vec![],
            key_vertices: vec![],
        }
    }
}

fn grips(ell: &Ellipse) -> Vec<GripDef> {
    let ctr = Vec3::new(
        ell.center.x as f32,
        ell.center.y as f32,
        ell.center.z as f32,
    );
    let maj = Vec3::new(
        (ell.center.x + ell.major_axis.x) as f32,
        (ell.center.y + ell.major_axis.y) as f32,
        (ell.center.z + ell.major_axis.z) as f32,
    );
    let major_xy =
        ((ell.major_axis.x * ell.major_axis.x + ell.major_axis.y * ell.major_axis.y) as f64).sqrt();
    let (px, py) = if major_xy > 1e-10 {
        let s = ell.major_axis_length() * ell.minor_axis_ratio / major_xy;
        (-ell.major_axis.y * s, ell.major_axis.x * s)
    } else {
        (0.0, ell.major_axis_length() * ell.minor_axis_ratio)
    };
    let min = Vec3::new(
        (ell.center.x + px) as f32,
        (ell.center.y + py) as f32,
        ell.center.z as f32,
    );
    vec![
        diamond_grip(0, ctr),
        square_grip(1, maj),
        square_grip(2, min),
    ]
}

fn properties(ell: &Ellipse) -> PropSection {
    let r_major = (ell.major_axis.x * ell.major_axis.x
        + ell.major_axis.y * ell.major_axis.y
        + ell.major_axis.z * ell.major_axis.z)
        .sqrt();
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", ell.center.x),
            edit("Center Y", "center_y", ell.center.y),
            edit("Center Z", "center_z", ell.center.z),
            ro("Major Radius", "major_r", format!("{r_major:.4}")),
            ro(
                "Minor Radius",
                "minor_r",
                format!("{:.4}", r_major * ell.minor_axis_ratio),
            ),
            ro(
                "Minor/Major",
                "ratio",
                format!("{:.4}", ell.minor_axis_ratio),
            ),
        ],
    }
}

fn apply_geom_prop(_ell: &mut Ellipse, _field: &str, _value: &str) {}

fn apply_grip(ell: &mut Ellipse, grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Translate(d)) => {
            ell.center.x += d.x as f64;
            ell.center.y += d.y as f64;
            ell.center.z += d.z as f64;
        }
        (0, GripApply::Absolute(p)) => {
            ell.center.x = p.x as f64;
            ell.center.y = p.y as f64;
            ell.center.z = p.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            ell.major_axis.x = p.x as f64 - ell.center.x;
            ell.major_axis.y = p.y as f64 - ell.center.y;
            ell.major_axis.z = p.z as f64 - ell.center.z;
        }
        (2, GripApply::Absolute(p)) => {
            let major_len = ell.major_axis_length();
            if major_len > 1e-10 {
                let dx = p.x as f64 - ell.center.x;
                let dy = p.y as f64 - ell.center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                ell.minor_axis_ratio = (dist / major_len).clamp(0.001, 1.0);
            }
        }
        _ => {}
    }
}

fn apply_transform(ell: &mut Ellipse, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(ell, t, |entity, p1, p2| {
        crate::scene::transform::reflect_xy_point(
            &mut entity.center.x,
            &mut entity.center.y,
            p1,
            p2,
        );
        crate::scene::transform::reflect_xy_point(
            &mut entity.major_axis.x,
            &mut entity.major_axis.y,
            p1,
            p2,
        );
    });
}

impl TruckConvertible for Ellipse {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self))
    }
}

impl Grippable for Ellipse {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Ellipse {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Ellipse {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
