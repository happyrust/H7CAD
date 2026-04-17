use truck_modeling::{
    base::{BoundedCurve, ParametricCurve},
    builder, BSplineCurve, Curve, Edge, KnotVec, Point3,
};

use crate::command::EntityTransform;
use crate::entities::common::{pt_to_vec3, ro_prop as ro, square_grip, transform_pt};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};

// ── Free functions ──────────────────────────────────────────────────────

pub fn to_truck(
    degree: i32,
    knots: &[f64],
    control_points: &[[f64; 3]],
) -> TruckEntity {
    let ctrl_pts: Vec<Point3> = control_points
        .iter()
        .map(|p| Point3::new(p[0], p[1], p[2]))
        .collect();
    if ctrl_pts.len() < 2 {
        let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let edge = builder::line(&v, &v);
        return TruckEntity {
            object: TruckObject::Curve(edge),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }
    let knot_vec = if !knots.is_empty() {
        KnotVec::from(knots.to_vec())
    } else {
        KnotVec::uniform_knot(degree as usize, ctrl_pts.len() - 1)
    };
    let bspline = BSplineCurve::new(knot_vec, ctrl_pts);
    let (t0, t1) = bspline.range_tuple();
    let p_start = bspline.subs(t0);
    let p_end = bspline.subs(t1);
    let v_start = builder::vertex(p_start);
    let v_end = builder::vertex(p_end);
    let edge = Edge::new(&v_start, &v_end, Curve::BSplineCurve(bspline));
    TruckEntity {
        object: TruckObject::Curve(edge),
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

pub fn grips(control_points: &[[f64; 3]]) -> Vec<GripDef> {
    control_points
        .iter()
        .enumerate()
        .map(|(i, p)| square_grip(i, pt_to_vec3(p)))
        .collect()
}

pub fn properties(degree: i32, control_points: &[[f64; 3]], fit_points: &[[f64; 3]]) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Degree", "degree", degree.to_string()),
            ro("Control Pts", "ctrl_pts", control_points.len().to_string()),
            ro("Fit Pts", "fit_pts", fit_points.len().to_string()),
        ],
    }
}

pub fn apply_grip(control_points: &mut [[f64; 3]], grip_id: usize, apply: GripApply) {
    if let Some(cp) = control_points.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                cp[0] = p.x as f64;
                cp[1] = p.y as f64;
                cp[2] = p.z as f64;
            }
            GripApply::Translate(d) => {
                cp[0] += d.x as f64;
                cp[1] += d.y as f64;
                cp[2] += d.z as f64;
            }
        }
    }
}

pub fn apply_transform(
    control_points: &mut [[f64; 3]],
    fit_points: &mut [[f64; 3]],
    t: &EntityTransform,
) {
    for cp in control_points.iter_mut() {
        transform_pt(cp, t);
    }
    for fp in fit_points.iter_mut() {
        transform_pt(fp, t);
    }
}

// ── Trait impls ─────────────────────────────────────────────────────────

use crate::entities::common::{arr_to_v3, v3_to_arr};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};

#[cfg(feature = "acadrust-compat")]
impl TruckConvertible for acadrust::entities::Spline {
    fn to_truck(&self, _doc: &acadrust::CadDocument) -> Option<TruckEntity> {
        let cps: Vec<[f64; 3]> = self.control_points.iter().map(|p| v3_to_arr(p)).collect();
        Some(self::to_truck(self.degree, &self.knots, &cps))
    }
}

#[cfg(feature = "acadrust-compat")]
impl Grippable for acadrust::entities::Spline {
    fn grips(&self) -> Vec<GripDef> {
        let cps: Vec<[f64; 3]> = self.control_points.iter().map(|p| v3_to_arr(p)).collect();
        self::grips(&cps)
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut cps: Vec<[f64; 3]> = self.control_points.iter().map(|p| v3_to_arr(p)).collect();
        self::apply_grip(&mut cps, grip_id, apply);
        for (dst, src) in self.control_points.iter_mut().zip(cps.iter()) {
            *dst = arr_to_v3(src);
        }
    }
}

#[cfg(feature = "acadrust-compat")]
impl PropertyEditable for acadrust::entities::Spline {
    fn geometry_properties(&self, _: &[String]) -> PropSection {
        let cps: Vec<[f64; 3]> = self.control_points.iter().map(|p| v3_to_arr(p)).collect();
        let fps: Vec<[f64; 3]> = self.fit_points.iter().map(|p| v3_to_arr(p)).collect();
        properties(self.degree, &cps, &fps)
    }
    fn apply_geom_prop(&mut self, _field: &str, _value: &str) {}
}

#[cfg(feature = "acadrust-compat")]
impl Transformable for acadrust::entities::Spline {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut cps: Vec<[f64; 3]> = self.control_points.iter().map(|p| v3_to_arr(p)).collect();
        let mut fps: Vec<[f64; 3]> = self.fit_points.iter().map(|p| v3_to_arr(p)).collect();
        self::apply_transform(&mut cps, &mut fps, t);
        for (dst, src) in self.control_points.iter_mut().zip(cps.iter()) {
            *dst = arr_to_v3(src);
        }
        for (dst, src) in self.fit_points.iter_mut().zip(fps.iter()) {
            *dst = arr_to_v3(src);
        }
    }
}
