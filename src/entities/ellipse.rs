use glam::Vec3;
use truck_modeling::{builder, BSplineCurve, Curve, Edge, KnotVec, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, pt_to_vec3, ro_prop as ro, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

const TAU: f64 = std::f64::consts::TAU;

fn major_len(major_axis: &[f64; 3]) -> f64 {
    (major_axis[0] * major_axis[0] + major_axis[1] * major_axis[1] + major_axis[2] * major_axis[2])
        .sqrt()
}

// ── Free functions ──────────────────────────────────────────────────────

pub fn to_truck(
    center: &[f64; 3],
    major_axis: &[f64; 3],
    ratio: f64,
    start_param: f64,
    end_param: f64,
) -> TruckEntity {
    let (cx, cy, cz) = (center[0], center[1], center[2]);
    let maj = Vec3::new(major_axis[0] as f32, major_axis[1] as f32, major_axis[2] as f32);
    let r_major = maj.length() as f64;
    let r_minor = r_major * ratio;
    let t0 = start_param;
    let mut t1 = end_param;
    if t1 <= t0 {
        t1 += TAU;
    }
    let u = if r_major > 1e-9 { maj / maj.length() } else { Vec3::X };
    let v_axis = Vec3::Z.cross(u);
    let center_v3 = pt_to_vec3(center);
    let is_closed = (t1 - t0 - TAU).abs() < 1e-6;

    if is_closed {
        let n = 16usize;
        let make_pts = |range_start: f64, range_end: f64| -> Vec<Point3> {
            (0..=n)
                .map(|i| {
                    let t = range_start + (range_end - range_start) * (i as f64 / n as f64);
                    let lx = (r_major * t.cos()) as f32;
                    let lz = (r_minor * t.sin()) as f32;
                    Point3::new(
                        cx + (lx * u.x + lz * v_axis.x) as f64,
                        cy + (lx * u.y + lz * v_axis.y) as f64,
                        cz + (lx * u.z + lz * v_axis.z) as f64,
                    )
                })
                .collect()
        };
        let pts_upper = make_pts(0.0, std::f64::consts::PI);
        let pts_lower = make_pts(std::f64::consts::PI, TAU);
        let v_pos = builder::vertex(*pts_upper.first().unwrap());
        let v_neg = builder::vertex(*pts_upper.last().unwrap());
        let spl_u = BSplineCurve::new(KnotVec::uniform_knot(1, n), pts_upper);
        let spl_l = BSplineCurve::new(KnotVec::uniform_knot(1, n), pts_lower);
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

pub fn grips(center: &[f64; 3], major_axis: &[f64; 3], ratio: f64) -> Vec<GripDef> {
    let ctr = pt_to_vec3(center);
    let maj = Vec3::new(
        (center[0] + major_axis[0]) as f32,
        (center[1] + major_axis[1]) as f32,
        (center[2] + major_axis[2]) as f32,
    );
    let ml = major_len(major_axis);
    let major_xy = (major_axis[0] * major_axis[0] + major_axis[1] * major_axis[1]).sqrt();
    let (px, py) = if major_xy > 1e-10 {
        let s = ml * ratio / major_xy;
        (-major_axis[1] * s, major_axis[0] * s)
    } else {
        (0.0, ml * ratio)
    };
    let min = Vec3::new(
        (center[0] + px) as f32,
        (center[1] + py) as f32,
        center[2] as f32,
    );
    vec![diamond_grip(0, ctr), square_grip(1, maj), square_grip(2, min)]
}

pub fn properties(center: &[f64; 3], major_axis: &[f64; 3], ratio: f64) -> PropSection {
    let r_major = major_len(major_axis);
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", center[0]),
            edit("Center Y", "center_y", center[1]),
            edit("Center Z", "center_z", center[2]),
            ro("Major Radius", "major_r", format!("{r_major:.4}")),
            ro("Minor Radius", "minor_r", format!("{:.4}", r_major * ratio)),
            ro("Minor/Major", "ratio", format!("{ratio:.4}")),
        ],
    }
}

pub fn apply_geom_prop(
    _center: &mut [f64; 3],
    _major_axis: &mut [f64; 3],
    _ratio: &mut f64,
    _field: &str,
    _value: &str,
) {
}

pub fn apply_grip(
    center: &mut [f64; 3],
    major_axis: &mut [f64; 3],
    ratio: &mut f64,
    grip_id: usize,
    apply: GripApply,
) {
    match (grip_id, apply) {
        (0, GripApply::Translate(d)) => {
            center[0] += d.x as f64;
            center[1] += d.y as f64;
            center[2] += d.z as f64;
        }
        (0, GripApply::Absolute(p)) => {
            center[0] = p.x as f64;
            center[1] = p.y as f64;
            center[2] = p.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            major_axis[0] = p.x as f64 - center[0];
            major_axis[1] = p.y as f64 - center[1];
            major_axis[2] = p.z as f64 - center[2];
        }
        (2, GripApply::Absolute(p)) => {
            let ml = major_len(major_axis);
            if ml > 1e-10 {
                let dx = p.x as f64 - center[0];
                let dy = p.y as f64 - center[1];
                let dist = (dx * dx + dy * dy).sqrt();
                *ratio = (dist / ml).clamp(0.001, 1.0);
            }
        }
        _ => {}
    }
}

pub fn apply_transform(
    center: &mut [f64; 3],
    major_axis: &mut [f64; 3],
    t: &EntityTransform,
) {
    transform_pt(center, t);
    crate::entities::common::transform_dir(major_axis, t);
    if let EntityTransform::Scale { center: c, factor } = t {
        let f = *factor as f64;
        let _ = c;
        for v in major_axis.iter_mut() {
            *v *= f;
        }
    }
}

// ── Trait impls ─────────────────────────────────────────────────────────

use crate::entities::common::{arr_to_v3, v3_to_arr};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};

impl TruckConvertible for acadrust::entities::Ellipse {
    fn to_truck(&self, _doc: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(self::to_truck(
            &v3_to_arr(&self.center),
            &v3_to_arr(&self.major_axis),
            self.minor_axis_ratio,
            self.start_parameter,
            self.end_parameter,
        ))
    }
}

impl Grippable for acadrust::entities::Ellipse {
    fn grips(&self) -> Vec<GripDef> {
        self::grips(&v3_to_arr(&self.center), &v3_to_arr(&self.major_axis), self.minor_axis_ratio)
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut c = v3_to_arr(&self.center);
        let mut ma = v3_to_arr(&self.major_axis);
        let mut r = self.minor_axis_ratio;
        self::apply_grip(&mut c, &mut ma, &mut r, grip_id, apply);
        self.center = arr_to_v3(&c);
        self.major_axis = arr_to_v3(&ma);
        self.minor_axis_ratio = r;
    }
}

impl PropertyEditable for acadrust::entities::Ellipse {
    fn geometry_properties(&self, _: &[String]) -> PropSection {
        properties(&v3_to_arr(&self.center), &v3_to_arr(&self.major_axis), self.minor_axis_ratio)
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let mut c = v3_to_arr(&self.center);
        let mut ma = v3_to_arr(&self.major_axis);
        let mut r = self.minor_axis_ratio;
        self::apply_geom_prop(&mut c, &mut ma, &mut r, field, value);
        self.center = arr_to_v3(&c);
        self.major_axis = arr_to_v3(&ma);
        self.minor_axis_ratio = r;
    }
}

impl Transformable for acadrust::entities::Ellipse {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut c = v3_to_arr(&self.center);
        let mut ma = v3_to_arr(&self.major_axis);
        self::apply_transform(&mut c, &mut ma, t);
        self.center = arr_to_v3(&c);
        self.major_axis = arr_to_v3(&ma);
    }
}
