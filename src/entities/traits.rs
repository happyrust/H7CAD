use acadrust::{CadDocument, EntityType};
use h7cad_native_model as nm;

use crate::command::EntityTransform;
use crate::entities::{arc, circle, ellipse, line, lwpolyline, point, ray, shape, solid, spline};
use crate::scene::acad_to_truck::TruckEntity;
use crate::scene::object::{GripApply, GripDef, PropSection};

// ── B5c helper: LwPolyline vertex bridge ─────────────────────────────────
// 把 acadrust::entities::LwVertex 列表转成 nm::LwVertex 列表，以便调用
// native free function。不修改 nm schema。

fn lwv_ar_to_nm(verts: &[acadrust::entities::LwVertex]) -> Vec<nm::LwVertex> {
    verts
        .iter()
        .map(|v| nm::LwVertex {
            x: v.location.x,
            y: v.location.y,
            bulge: v.bulge,
        })
        .collect()
}

fn lwv_write_back(dst: &mut [acadrust::entities::LwVertex], src: &[nm::LwVertex]) {
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        d.location.x = s.x;
        d.location.y = s.y;
        d.bulge = s.bulge;
    }
}

// ── Native dispatch (B5a) ────────────────────────────────────────────────
//
// Parallel entry points for `nm::EntityData`. Right now only the 5 simple
// primitive types are covered (Line/Circle/Arc/Point/Ellipse) — every other
// variant falls through to the empty default. 未来批次逐步补齐 LwPolyline /
// Spline / Text / Dimension / Hatch / Insert / Viewport 等。
//
// 这些函数本身**尚未被调用**（acadrust EntityType dispatch 仍是主路径）。
// B5 逐个命令切到 native_store 时依次接通。保留 `#[allow(dead_code)]` 避免
// 在批次过渡期造成 warning 噪音。

#[allow(dead_code)]
pub fn to_truck_native(data: &nm::EntityData) -> Option<TruckEntity> {
    match data {
        nm::EntityData::Line { start, end } => Some(line::to_truck(start, end)),
        nm::EntityData::Circle { center, radius } => Some(circle::to_truck(center, *radius)),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Some(arc::to_truck(center, *radius, *start_angle, *end_angle)),
        nm::EntityData::Point { position } => Some(point::to_truck(position)),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => Some(ellipse::to_truck(
            center,
            major_axis,
            *ratio,
            *start_param,
            *end_param,
        )),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn grips_native(data: &nm::EntityData) -> Vec<GripDef> {
    match data {
        nm::EntityData::Line { start, end } => line::grips(start, end),
        nm::EntityData::Circle { center, radius } => circle::grips(center, *radius),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::grips(center, *radius, *start_angle, *end_angle),
        nm::EntityData::Point { position } => point::grips(position),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            ..
        } => ellipse::grips(center, major_axis, *ratio),
        _ => vec![],
    }
}

#[allow(dead_code)]
pub fn properties_native(data: &nm::EntityData) -> Option<PropSection> {
    match data {
        nm::EntityData::Line { start, end } => Some(line::properties(start, end)),
        nm::EntityData::Circle { center, radius } => Some(circle::properties(center, *radius)),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Some(arc::properties(center, *radius, *start_angle, *end_angle)),
        nm::EntityData::Point { position } => Some(point::properties(position)),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            ..
        } => Some(ellipse::properties(center, major_axis, *ratio)),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn apply_geom_prop_native(data: &mut nm::EntityData, field: &str, value: &str) {
    match data {
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
        nm::EntityData::Point { position } => point::apply_geom_prop(position, field, value),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            ..
        } => ellipse::apply_geom_prop(center, major_axis, ratio, field, value),
        _ => {}
    }
}

#[allow(dead_code)]
pub fn apply_grip_native(data: &mut nm::EntityData, grip_id: usize, apply: GripApply) {
    match data {
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
        nm::EntityData::Point { position } => point::apply_grip(position, grip_id, apply),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            ..
        } => ellipse::apply_grip(center, major_axis, ratio, grip_id, apply),
        _ => {}
    }
}

#[allow(dead_code)]
pub fn apply_transform_native(data: &mut nm::EntityData, t: &EntityTransform) {
    match data {
        nm::EntityData::Line { start, end } => line::apply_transform(start, end, t),
        nm::EntityData::Circle { center, radius } => circle::apply_transform(center, radius, t),
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc::apply_transform(center, radius, start_angle, end_angle, t),
        nm::EntityData::Point { position } => point::apply_transform(position, t),
        nm::EntityData::Ellipse {
            center,
            major_axis,
            ..
        } => ellipse::apply_transform(center, major_axis, t),
        _ => {}
    }
}

// ── Legacy acadrust dispatch ─────────────────────────────────────────────

pub trait TruckConvertible {
    fn to_truck(&self, document: &CadDocument) -> Option<TruckEntity>;
}

pub trait Grippable {
    fn grips(&self) -> Vec<GripDef>;
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply);
}

pub trait PropertyEditable {
    fn geometry_properties(&self, text_style_names: &[String]) -> PropSection;
    fn apply_geom_prop(&mut self, field: &str, value: &str);
}

pub trait Transformable {
    fn apply_transform(&mut self, t: &EntityTransform);
}

pub trait EntityTypeOps {
    fn to_truck_entity(&self, document: &CadDocument) -> Option<TruckEntity>;
    fn grips(&self) -> Vec<GripDef>;
    fn geometry_properties(&self, text_style_names: &[String]) -> Option<PropSection>;
    fn apply_geom_prop(&mut self, field: &str, value: &str);
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply);
    fn apply_transform(&mut self, t: &EntityTransform);
}

impl EntityTypeOps for EntityType {
    fn to_truck_entity(&self, document: &CadDocument) -> Option<TruckEntity> {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Point(pt) => {
                let p = [pt.location.x, pt.location.y, pt.location.z];
                Some(point::to_truck(&p))
            }
            EntityType::Line(l) => {
                let s = [l.start.x, l.start.y, l.start.z];
                let e = [l.end.x, l.end.y, l.end.z];
                Some(line::to_truck(&s, &e))
            }
            EntityType::Circle(c) => {
                let center = [c.center.x, c.center.y, c.center.z];
                Some(circle::to_truck(&center, c.radius))
            }
            EntityType::Arc(a) => {
                let center = [a.center.x, a.center.y, a.center.z];
                Some(arc::to_truck(
                    &center,
                    a.radius,
                    a.start_angle.to_radians(),
                    a.end_angle.to_radians(),
                ))
            }
            EntityType::Ellipse(e) => {
                let center = [e.center.x, e.center.y, e.center.z];
                let major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                Some(ellipse::to_truck(
                    &center,
                    &major,
                    e.minor_axis_ratio,
                    e.start_parameter,
                    e.end_parameter,
                ))
            }
            // ── B5e: Ray/XLine/Solid/Spline/Shape inline native dispatch ──
            EntityType::Spline(sp) => {
                let cps: Vec<[f64; 3]> = sp
                    .control_points
                    .iter()
                    .map(|p| [p.x, p.y, p.z])
                    .collect();
                Some(spline::to_truck(sp.degree, &sp.knots, &cps))
            }
            EntityType::Ray(r) => {
                let o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let d = [r.direction.x, r.direction.y, r.direction.z];
                Some(ray::ray_to_truck(&o, &d))
            }
            EntityType::XLine(xl) => {
                let o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let d = [xl.direction.x, xl.direction.y, xl.direction.z];
                Some(ray::xline_to_truck(&o, &d))
            }
            EntityType::Solid(sd) => {
                let corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                Some(solid::to_truck(&corners))
            }
            EntityType::Shape(shp) => {
                let ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                Some(shape::to_truck(&ins, shp.size))
            }
            // ── 其余类型暂走 acadrust adapter（B5 后续批次扩展） ────────────
            EntityType::LwPolyline(pline) => {
                let verts = lwv_ar_to_nm(&pline.vertices);
                Some(lwpolyline::to_truck(&verts, pline.is_closed, pline.elevation))
            }
            EntityType::Polyline(pl) => TruckConvertible::to_truck(pl, document),
            EntityType::Polyline2D(pl) => TruckConvertible::to_truck(pl, document),
            EntityType::Polyline3D(pl) => TruckConvertible::to_truck(pl, document),
            EntityType::RasterImage(img) => TruckConvertible::to_truck(img, document),
            EntityType::Wipeout(wo) => TruckConvertible::to_truck(wo, document),
            EntityType::AttributeDefinition(a) => TruckConvertible::to_truck(a, document),
            EntityType::AttributeEntity(a) => TruckConvertible::to_truck(a, document),
            EntityType::MLine(ml) => TruckConvertible::to_truck(ml, document),
            EntityType::Tolerance(tol) => TruckConvertible::to_truck(tol, document),
            EntityType::Face3D(f) => TruckConvertible::to_truck(f, document),
            EntityType::PolygonMesh(pm) => TruckConvertible::to_truck(pm, document),
            EntityType::PolyfaceMesh(pfm) => TruckConvertible::to_truck(pfm, document),
            EntityType::Table(tbl) => TruckConvertible::to_truck(tbl, document),
            EntityType::Text(text) => TruckConvertible::to_truck(text, document),
            EntityType::MText(text) => TruckConvertible::to_truck(text, document),
            EntityType::Leader(leader) => TruckConvertible::to_truck(leader, document),
            EntityType::MultiLeader(ml) => TruckConvertible::to_truck(ml, document),
            EntityType::Underlay(ul) => TruckConvertible::to_truck(ul, document),
            _ => None,
        }
    }

    fn grips(&self) -> Vec<GripDef> {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Line(l) => {
                let s = [l.start.x, l.start.y, l.start.z];
                let e = [l.end.x, l.end.y, l.end.z];
                line::grips(&s, &e)
            }
            EntityType::Circle(c) => {
                let center = [c.center.x, c.center.y, c.center.z];
                circle::grips(&center, c.radius)
            }
            EntityType::Arc(a) => {
                let center = [a.center.x, a.center.y, a.center.z];
                arc::grips(
                    &center,
                    a.radius,
                    a.start_angle.to_radians(),
                    a.end_angle.to_radians(),
                )
            }
            EntityType::Ellipse(e) => {
                let center = [e.center.x, e.center.y, e.center.z];
                let major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                ellipse::grips(&center, &major, e.minor_axis_ratio)
            }
            EntityType::Point(pt) => {
                let p = [pt.location.x, pt.location.y, pt.location.z];
                point::grips(&p)
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::LwPolyline(pline) => {
                let verts = lwv_ar_to_nm(&pline.vertices);
                lwpolyline::grips(&verts, pline.elevation)
            }
            // ── B5e: Ray/XLine/Solid/Spline/Shape inline ──────────────────
            EntityType::Ray(r) => {
                let o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let d = [r.direction.x, r.direction.y, r.direction.z];
                ray::ray_grips(&o, &d)
            }
            EntityType::XLine(xl) => {
                let o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let d = [xl.direction.x, xl.direction.y, xl.direction.z];
                ray::ray_grips(&o, &d)
            }
            EntityType::Solid(sd) => {
                let corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                solid::grips(&corners)
            }
            EntityType::Spline(sp) => {
                let cps: Vec<[f64; 3]> =
                    sp.control_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                spline::grips(&cps)
            }
            EntityType::Shape(shp) => {
                let ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                shape::grips(&ins)
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::Polyline(pl) => Grippable::grips(pl),
            EntityType::Polyline2D(pl) => Grippable::grips(pl),
            EntityType::Polyline3D(pl) => Grippable::grips(pl),
            EntityType::RasterImage(img) => Grippable::grips(img),
            EntityType::Wipeout(wo) => Grippable::grips(wo),
            EntityType::AttributeDefinition(a) => Grippable::grips(a),
            EntityType::AttributeEntity(a) => Grippable::grips(a),
            EntityType::MLine(ml) => Grippable::grips(ml),
            EntityType::Tolerance(tol) => Grippable::grips(tol),
            EntityType::Face3D(f) => Grippable::grips(f),
            EntityType::PolygonMesh(pm) => Grippable::grips(pm),
            EntityType::PolyfaceMesh(pfm) => Grippable::grips(pfm),
            EntityType::Table(tbl) => Grippable::grips(tbl),
            EntityType::Text(text) => Grippable::grips(text),
            EntityType::MText(text) => Grippable::grips(text),
            EntityType::Viewport(vp) => Grippable::grips(vp),
            EntityType::Insert(ins) => Grippable::grips(ins),
            EntityType::Leader(leader) => Grippable::grips(leader),
            EntityType::MultiLeader(ml) => Grippable::grips(ml),
            EntityType::Dimension(dim) => Grippable::grips(dim),
            EntityType::Hatch(hatch) => Grippable::grips(hatch),
            EntityType::Underlay(ul) => Grippable::grips(ul),
            _ => vec![],
        }
    }

    fn geometry_properties(&self, text_style_names: &[String]) -> Option<PropSection> {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Line(l) => {
                let s = [l.start.x, l.start.y, l.start.z];
                let e = [l.end.x, l.end.y, l.end.z];
                Some(line::properties(&s, &e))
            }
            EntityType::Circle(c) => {
                let center = [c.center.x, c.center.y, c.center.z];
                Some(circle::properties(&center, c.radius))
            }
            EntityType::Arc(a) => {
                let center = [a.center.x, a.center.y, a.center.z];
                Some(arc::properties(
                    &center,
                    a.radius,
                    a.start_angle.to_radians(),
                    a.end_angle.to_radians(),
                ))
            }
            EntityType::Ellipse(e) => {
                let center = [e.center.x, e.center.y, e.center.z];
                let major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                Some(ellipse::properties(&center, &major, e.minor_axis_ratio))
            }
            EntityType::Point(pt) => {
                let p = [pt.location.x, pt.location.y, pt.location.z];
                Some(point::properties(&p))
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::LwPolyline(pline) => {
                let verts = lwv_ar_to_nm(&pline.vertices);
                Some(lwpolyline::properties(&verts, pline.is_closed, pline.elevation))
            }
            // ── B5e: Ray/XLine/Solid/Spline/Shape inline ──────────────────
            EntityType::Ray(r) => {
                let o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let d = [r.direction.x, r.direction.y, r.direction.z];
                Some(ray::ray_properties(&o, &d, "ray"))
            }
            EntityType::XLine(xl) => {
                let o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let d = [xl.direction.x, xl.direction.y, xl.direction.z];
                Some(ray::ray_properties(&o, &d, "xl"))
            }
            EntityType::Solid(sd) => {
                let corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                Some(solid::properties(&corners, sd.thickness))
            }
            EntityType::Spline(sp) => {
                let cps: Vec<[f64; 3]> =
                    sp.control_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                let fps: Vec<[f64; 3]> =
                    sp.fit_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                Some(spline::properties(sp.degree, &cps, &fps))
            }
            EntityType::Shape(shp) => {
                let ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                Some(shape::properties(
                    &ins,
                    shp.size,
                    shp.rotation,
                    &shp.shape_name,
                    &shp.style_name,
                ))
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::Polyline(pl) => Some(PropertyEditable::geometry_properties(
                pl,
                text_style_names,
            )),
            EntityType::Polyline2D(pl) => Some(PropertyEditable::geometry_properties(
                pl,
                text_style_names,
            )),
            EntityType::Polyline3D(pl) => Some(PropertyEditable::geometry_properties(
                pl,
                text_style_names,
            )),
            EntityType::RasterImage(img) => Some(PropertyEditable::geometry_properties(
                img,
                text_style_names,
            )),
            EntityType::Wipeout(wo) => Some(PropertyEditable::geometry_properties(
                wo,
                text_style_names,
            )),
            EntityType::AttributeDefinition(a) => Some(PropertyEditable::geometry_properties(
                a,
                text_style_names,
            )),
            EntityType::AttributeEntity(a) => Some(PropertyEditable::geometry_properties(
                a,
                text_style_names,
            )),
            EntityType::MLine(ml) => Some(PropertyEditable::geometry_properties(
                ml,
                text_style_names,
            )),
            EntityType::Tolerance(tol) => Some(PropertyEditable::geometry_properties(
                tol,
                text_style_names,
            )),
            EntityType::Face3D(f) => Some(PropertyEditable::geometry_properties(
                f,
                text_style_names,
            )),
            EntityType::PolygonMesh(pm) => Some(PropertyEditable::geometry_properties(
                pm,
                text_style_names,
            )),
            EntityType::PolyfaceMesh(pfm) => Some(PropertyEditable::geometry_properties(
                pfm,
                text_style_names,
            )),
            EntityType::Table(tbl) => Some(PropertyEditable::geometry_properties(
                tbl,
                text_style_names,
            )),
            EntityType::Hatch(hatch) => Some(PropertyEditable::geometry_properties(
                hatch,
                text_style_names,
            )),
            EntityType::Text(text) => Some(PropertyEditable::geometry_properties(
                text,
                text_style_names,
            )),
            EntityType::MText(text) => Some(PropertyEditable::geometry_properties(
                text,
                text_style_names,
            )),
            EntityType::Viewport(vp) => {
                Some(PropertyEditable::geometry_properties(vp, text_style_names))
            }
            EntityType::Insert(ins) => {
                Some(PropertyEditable::geometry_properties(ins, text_style_names))
            }
            EntityType::Dimension(dim) => Some(PropertyEditable::geometry_properties(
                dim,
                text_style_names,
            )),
            EntityType::Leader(leader) => Some(PropertyEditable::geometry_properties(
                leader,
                text_style_names,
            )),
            EntityType::MultiLeader(ml) => Some(PropertyEditable::geometry_properties(
                ml,
                text_style_names,
            )),
            EntityType::Underlay(ul) => Some(PropertyEditable::geometry_properties(
                ul,
                text_style_names,
            )),
            _ => None,
        }
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Line(l) => {
                let mut s = [l.start.x, l.start.y, l.start.z];
                let mut e = [l.end.x, l.end.y, l.end.z];
                line::apply_geom_prop(&mut s, &mut e, field, value);
                l.start.x = s[0]; l.start.y = s[1]; l.start.z = s[2];
                l.end.x = e[0]; l.end.y = e[1]; l.end.z = e[2];
            }
            EntityType::Circle(c) => {
                let mut center = [c.center.x, c.center.y, c.center.z];
                let mut radius = c.radius;
                circle::apply_geom_prop(&mut center, &mut radius, field, value);
                c.center.x = center[0]; c.center.y = center[1]; c.center.z = center[2];
                c.radius = radius;
            }
            EntityType::Arc(a) => {
                let mut center = [a.center.x, a.center.y, a.center.z];
                let mut radius = a.radius;
                let mut sa = a.start_angle.to_radians();
                let mut ea = a.end_angle.to_radians();
                arc::apply_geom_prop(&mut center, &mut radius, &mut sa, &mut ea, field, value);
                a.center.x = center[0]; a.center.y = center[1]; a.center.z = center[2];
                a.radius = radius;
                a.start_angle = sa.to_degrees();
                a.end_angle = ea.to_degrees();
            }
            EntityType::Ellipse(e) => {
                let mut center = [e.center.x, e.center.y, e.center.z];
                let mut major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                let mut ratio = e.minor_axis_ratio;
                ellipse::apply_geom_prop(&mut center, &mut major, &mut ratio, field, value);
                e.center.x = center[0]; e.center.y = center[1]; e.center.z = center[2];
                e.major_axis.x = major[0]; e.major_axis.y = major[1]; e.major_axis.z = major[2];
                e.minor_axis_ratio = ratio;
            }
            EntityType::Point(pt) => {
                let mut p = [pt.location.x, pt.location.y, pt.location.z];
                point::apply_geom_prop(&mut p, field, value);
                pt.location.x = p[0]; pt.location.y = p[1]; pt.location.z = p[2];
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::LwPolyline(pline) => {
                lwpolyline::apply_geom_prop(&mut pline.elevation, field, value);
            }
            // ── B5e: Ray/XLine/Solid/Shape inline (Spline 空实现 noop) ────
            EntityType::Ray(r) => {
                let mut o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let mut d = [r.direction.x, r.direction.y, r.direction.z];
                ray::ray_apply_geom_prop(&mut o, &mut d, field, value);
                r.base_point.x = o[0];
                r.base_point.y = o[1];
                r.base_point.z = o[2];
                r.direction.x = d[0];
                r.direction.y = d[1];
                r.direction.z = d[2];
            }
            EntityType::XLine(xl) => {
                let mut o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let mut d = [xl.direction.x, xl.direction.y, xl.direction.z];
                ray::ray_apply_geom_prop(&mut o, &mut d, field, value);
                xl.base_point.x = o[0];
                xl.base_point.y = o[1];
                xl.base_point.z = o[2];
                xl.direction.x = d[0];
                xl.direction.y = d[1];
                xl.direction.z = d[2];
            }
            EntityType::Solid(sd) => {
                let mut corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                let mut th = sd.thickness;
                solid::apply_geom_prop(&mut corners, &mut th, field, value);
                sd.first_corner.x = corners[0][0];
                sd.first_corner.y = corners[0][1];
                sd.first_corner.z = corners[0][2];
                sd.second_corner.x = corners[1][0];
                sd.second_corner.y = corners[1][1];
                sd.second_corner.z = corners[1][2];
                sd.third_corner.x = corners[2][0];
                sd.third_corner.y = corners[2][1];
                sd.third_corner.z = corners[2][2];
                sd.fourth_corner.x = corners[3][0];
                sd.fourth_corner.y = corners[3][1];
                sd.fourth_corner.z = corners[3][2];
                sd.thickness = th;
            }
            EntityType::Spline(_) => {
                // native spline 无 apply_geom_prop（只读属性）
            }
            EntityType::Shape(shp) => {
                let mut ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                let mut sz = shp.size;
                let mut rot = shp.rotation;
                shape::apply_geom_prop(&mut ins, &mut sz, &mut rot, field, value);
                shp.insertion_point.x = ins[0];
                shp.insertion_point.y = ins[1];
                shp.insertion_point.z = ins[2];
                shp.size = sz;
                shp.rotation = rot;
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::Polyline(pl) => PropertyEditable::apply_geom_prop(pl, field, value),
            EntityType::Polyline2D(pl) => PropertyEditable::apply_geom_prop(pl, field, value),
            EntityType::Polyline3D(pl) => PropertyEditable::apply_geom_prop(pl, field, value),
            EntityType::RasterImage(img) => PropertyEditable::apply_geom_prop(img, field, value),
            EntityType::Wipeout(wo) => PropertyEditable::apply_geom_prop(wo, field, value),
            EntityType::AttributeDefinition(a) => PropertyEditable::apply_geom_prop(a, field, value),
            EntityType::AttributeEntity(a) => PropertyEditable::apply_geom_prop(a, field, value),
            EntityType::MLine(ml) => PropertyEditable::apply_geom_prop(ml, field, value),
            EntityType::Tolerance(tol) => PropertyEditable::apply_geom_prop(tol, field, value),
            EntityType::Face3D(f) => PropertyEditable::apply_geom_prop(f, field, value),
            EntityType::PolygonMesh(pm) => PropertyEditable::apply_geom_prop(pm, field, value),
            EntityType::PolyfaceMesh(pfm) => PropertyEditable::apply_geom_prop(pfm, field, value),
            EntityType::Table(tbl) => PropertyEditable::apply_geom_prop(tbl, field, value),
            EntityType::Hatch(hatch) => PropertyEditable::apply_geom_prop(hatch, field, value),
            EntityType::Text(text) => PropertyEditable::apply_geom_prop(text, field, value),
            EntityType::MText(text) => PropertyEditable::apply_geom_prop(text, field, value),
            EntityType::Viewport(vp) => PropertyEditable::apply_geom_prop(vp, field, value),
            EntityType::Insert(ins) => PropertyEditable::apply_geom_prop(ins, field, value),
            EntityType::Dimension(dim) => PropertyEditable::apply_geom_prop(dim, field, value),
            EntityType::Leader(leader) => PropertyEditable::apply_geom_prop(leader, field, value),
            EntityType::MultiLeader(ml) => PropertyEditable::apply_geom_prop(ml, field, value),
            EntityType::Underlay(ul) => PropertyEditable::apply_geom_prop(ul, field, value),
            _ => {}
        }
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Line(l) => {
                let mut s = [l.start.x, l.start.y, l.start.z];
                let mut e = [l.end.x, l.end.y, l.end.z];
                line::apply_grip(&mut s, &mut e, grip_id, apply);
                l.start.x = s[0]; l.start.y = s[1]; l.start.z = s[2];
                l.end.x = e[0]; l.end.y = e[1]; l.end.z = e[2];
            }
            EntityType::Circle(c) => {
                let mut center = [c.center.x, c.center.y, c.center.z];
                let mut radius = c.radius;
                circle::apply_grip(&mut center, &mut radius, grip_id, apply);
                c.center.x = center[0]; c.center.y = center[1]; c.center.z = center[2];
                c.radius = radius;
            }
            EntityType::Arc(a) => {
                let mut center = [a.center.x, a.center.y, a.center.z];
                let mut radius = a.radius;
                let mut sa = a.start_angle.to_radians();
                let mut ea = a.end_angle.to_radians();
                arc::apply_grip(&mut center, &mut radius, &mut sa, &mut ea, grip_id, apply);
                a.center.x = center[0]; a.center.y = center[1]; a.center.z = center[2];
                a.radius = radius;
                a.start_angle = sa.to_degrees();
                a.end_angle = ea.to_degrees();
            }
            EntityType::Ellipse(e) => {
                let mut center = [e.center.x, e.center.y, e.center.z];
                let mut major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                let mut ratio = e.minor_axis_ratio;
                ellipse::apply_grip(&mut center, &mut major, &mut ratio, grip_id, apply);
                e.center.x = center[0]; e.center.y = center[1]; e.center.z = center[2];
                e.major_axis.x = major[0]; e.major_axis.y = major[1]; e.major_axis.z = major[2];
                e.minor_axis_ratio = ratio;
            }
            EntityType::Point(pt) => {
                let mut p = [pt.location.x, pt.location.y, pt.location.z];
                point::apply_grip(&mut p, grip_id, apply);
                pt.location.x = p[0]; pt.location.y = p[1]; pt.location.z = p[2];
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::LwPolyline(pline) => {
                let mut verts = lwv_ar_to_nm(&pline.vertices);
                lwpolyline::apply_grip(&mut verts, grip_id, apply);
                lwv_write_back(&mut pline.vertices, &verts);
            }
            // ── B5e: Ray/XLine/Solid/Spline/Shape inline ──────────────────
            EntityType::Ray(r) => {
                let mut o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let mut d = [r.direction.x, r.direction.y, r.direction.z];
                ray::ray_apply_grip(&mut o, &mut d, grip_id, apply);
                r.base_point.x = o[0];
                r.base_point.y = o[1];
                r.base_point.z = o[2];
                r.direction.x = d[0];
                r.direction.y = d[1];
                r.direction.z = d[2];
            }
            EntityType::XLine(xl) => {
                let mut o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let mut d = [xl.direction.x, xl.direction.y, xl.direction.z];
                ray::ray_apply_grip(&mut o, &mut d, grip_id, apply);
                xl.base_point.x = o[0];
                xl.base_point.y = o[1];
                xl.base_point.z = o[2];
                xl.direction.x = d[0];
                xl.direction.y = d[1];
                xl.direction.z = d[2];
            }
            EntityType::Solid(sd) => {
                let mut corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                solid::apply_grip(&mut corners, grip_id, apply);
                sd.first_corner.x = corners[0][0];
                sd.first_corner.y = corners[0][1];
                sd.first_corner.z = corners[0][2];
                sd.second_corner.x = corners[1][0];
                sd.second_corner.y = corners[1][1];
                sd.second_corner.z = corners[1][2];
                sd.third_corner.x = corners[2][0];
                sd.third_corner.y = corners[2][1];
                sd.third_corner.z = corners[2][2];
                sd.fourth_corner.x = corners[3][0];
                sd.fourth_corner.y = corners[3][1];
                sd.fourth_corner.z = corners[3][2];
            }
            EntityType::Spline(sp) => {
                let mut cps: Vec<[f64; 3]> =
                    sp.control_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                spline::apply_grip(&mut cps, grip_id, apply);
                for (dst, src) in sp.control_points.iter_mut().zip(cps.iter()) {
                    dst.x = src[0];
                    dst.y = src[1];
                    dst.z = src[2];
                }
            }
            EntityType::Shape(shp) => {
                let mut ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                shape::apply_grip(&mut ins, grip_id, apply);
                shp.insertion_point.x = ins[0];
                shp.insertion_point.y = ins[1];
                shp.insertion_point.z = ins[2];
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::Polyline(pl) => Grippable::apply_grip(pl, grip_id, apply),
            EntityType::Polyline2D(pl) => Grippable::apply_grip(pl, grip_id, apply),
            EntityType::Polyline3D(pl) => Grippable::apply_grip(pl, grip_id, apply),
            EntityType::RasterImage(img) => Grippable::apply_grip(img, grip_id, apply),
            EntityType::Wipeout(wo) => Grippable::apply_grip(wo, grip_id, apply),
            EntityType::AttributeDefinition(a) => Grippable::apply_grip(a, grip_id, apply),
            EntityType::AttributeEntity(a) => Grippable::apply_grip(a, grip_id, apply),
            EntityType::MLine(ml) => Grippable::apply_grip(ml, grip_id, apply),
            EntityType::Tolerance(tol) => Grippable::apply_grip(tol, grip_id, apply),
            EntityType::Face3D(f) => Grippable::apply_grip(f, grip_id, apply),
            EntityType::PolygonMesh(pm) => Grippable::apply_grip(pm, grip_id, apply),
            EntityType::PolyfaceMesh(pfm) => Grippable::apply_grip(pfm, grip_id, apply),
            EntityType::Table(tbl) => Grippable::apply_grip(tbl, grip_id, apply),
            EntityType::Text(text) => Grippable::apply_grip(text, grip_id, apply),
            EntityType::MText(text) => Grippable::apply_grip(text, grip_id, apply),
            EntityType::Viewport(vp) => Grippable::apply_grip(vp, grip_id, apply),
            EntityType::Insert(ins) => Grippable::apply_grip(ins, grip_id, apply),
            EntityType::Leader(leader) => Grippable::apply_grip(leader, grip_id, apply),
            EntityType::MultiLeader(ml) => Grippable::apply_grip(ml, grip_id, apply),
            EntityType::Dimension(dim) => Grippable::apply_grip(dim, grip_id, apply),
            EntityType::Hatch(hatch) => Grippable::apply_grip(hatch, grip_id, apply),
            EntityType::Underlay(ul) => Grippable::apply_grip(ul, grip_id, apply),
            _ => {}
        }
    }

    fn apply_transform(&mut self, t: &EntityTransform) {
        match self {
            // ── B5d: 5 个简单 entity 直接走 native free function ──────────
            EntityType::Line(l) => {
                let mut s = [l.start.x, l.start.y, l.start.z];
                let mut e = [l.end.x, l.end.y, l.end.z];
                line::apply_transform(&mut s, &mut e, t);
                l.start.x = s[0]; l.start.y = s[1]; l.start.z = s[2];
                l.end.x = e[0]; l.end.y = e[1]; l.end.z = e[2];
            }
            EntityType::Circle(c) => {
                let mut center = [c.center.x, c.center.y, c.center.z];
                let mut radius = c.radius;
                circle::apply_transform(&mut center, &mut radius, t);
                c.center.x = center[0]; c.center.y = center[1]; c.center.z = center[2];
                c.radius = radius;
            }
            EntityType::Arc(a) => {
                let mut center = [a.center.x, a.center.y, a.center.z];
                let mut radius = a.radius;
                let mut sa = a.start_angle.to_radians();
                let mut ea = a.end_angle.to_radians();
                arc::apply_transform(&mut center, &mut radius, &mut sa, &mut ea, t);
                a.center.x = center[0]; a.center.y = center[1]; a.center.z = center[2];
                a.radius = radius;
                a.start_angle = sa.to_degrees();
                a.end_angle = ea.to_degrees();
            }
            EntityType::Ellipse(e) => {
                let mut center = [e.center.x, e.center.y, e.center.z];
                let mut major = [e.major_axis.x, e.major_axis.y, e.major_axis.z];
                ellipse::apply_transform(&mut center, &mut major, t);
                e.center.x = center[0]; e.center.y = center[1]; e.center.z = center[2];
                e.major_axis.x = major[0]; e.major_axis.y = major[1]; e.major_axis.z = major[2];
            }
            EntityType::Point(pt) => {
                let mut p = [pt.location.x, pt.location.y, pt.location.z];
                point::apply_transform(&mut p, t);
                pt.location.x = p[0]; pt.location.y = p[1]; pt.location.z = p[2];
            }
            EntityType::LwPolyline(pline) => {
                let mut verts = lwv_ar_to_nm(&pline.vertices);
                lwpolyline::apply_transform(&mut verts, t);
                lwv_write_back(&mut pline.vertices, &verts);
            }
            // ── B5e: Ray/XLine/Solid/Spline/Shape inline ──────────────────
            EntityType::Ray(r) => {
                let mut o = [r.base_point.x, r.base_point.y, r.base_point.z];
                let mut d = [r.direction.x, r.direction.y, r.direction.z];
                ray::ray_apply_transform(&mut o, &mut d, t);
                r.base_point.x = o[0];
                r.base_point.y = o[1];
                r.base_point.z = o[2];
                r.direction.x = d[0];
                r.direction.y = d[1];
                r.direction.z = d[2];
            }
            EntityType::XLine(xl) => {
                let mut o = [xl.base_point.x, xl.base_point.y, xl.base_point.z];
                let mut d = [xl.direction.x, xl.direction.y, xl.direction.z];
                ray::ray_apply_transform(&mut o, &mut d, t);
                xl.base_point.x = o[0];
                xl.base_point.y = o[1];
                xl.base_point.z = o[2];
                xl.direction.x = d[0];
                xl.direction.y = d[1];
                xl.direction.z = d[2];
            }
            EntityType::Solid(sd) => {
                let mut corners = [
                    [sd.first_corner.x, sd.first_corner.y, sd.first_corner.z],
                    [sd.second_corner.x, sd.second_corner.y, sd.second_corner.z],
                    [sd.third_corner.x, sd.third_corner.y, sd.third_corner.z],
                    [sd.fourth_corner.x, sd.fourth_corner.y, sd.fourth_corner.z],
                ];
                solid::apply_transform(&mut corners, t);
                sd.first_corner.x = corners[0][0];
                sd.first_corner.y = corners[0][1];
                sd.first_corner.z = corners[0][2];
                sd.second_corner.x = corners[1][0];
                sd.second_corner.y = corners[1][1];
                sd.second_corner.z = corners[1][2];
                sd.third_corner.x = corners[2][0];
                sd.third_corner.y = corners[2][1];
                sd.third_corner.z = corners[2][2];
                sd.fourth_corner.x = corners[3][0];
                sd.fourth_corner.y = corners[3][1];
                sd.fourth_corner.z = corners[3][2];
            }
            EntityType::Spline(sp) => {
                let mut cps: Vec<[f64; 3]> =
                    sp.control_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                let mut fps: Vec<[f64; 3]> =
                    sp.fit_points.iter().map(|p| [p.x, p.y, p.z]).collect();
                spline::apply_transform(&mut cps, &mut fps, t);
                for (dst, src) in sp.control_points.iter_mut().zip(cps.iter()) {
                    dst.x = src[0];
                    dst.y = src[1];
                    dst.z = src[2];
                }
                for (dst, src) in sp.fit_points.iter_mut().zip(fps.iter()) {
                    dst.x = src[0];
                    dst.y = src[1];
                    dst.z = src[2];
                }
            }
            EntityType::Shape(shp) => {
                let mut ins = [
                    shp.insertion_point.x,
                    shp.insertion_point.y,
                    shp.insertion_point.z,
                ];
                shape::apply_transform(&mut ins, t);
                shp.insertion_point.x = ins[0];
                shp.insertion_point.y = ins[1];
                shp.insertion_point.z = ins[2];
            }
            // ── 其余类型暂走 acadrust adapter ──────────────────────────────
            EntityType::Hatch(hatch) => Transformable::apply_transform(hatch, t),
            EntityType::Insert(ins) => Transformable::apply_transform(ins, t),
            EntityType::Polyline(pl) => Transformable::apply_transform(pl, t),
            EntityType::Polyline2D(pl) => Transformable::apply_transform(pl, t),
            EntityType::Polyline3D(pl) => Transformable::apply_transform(pl, t),
            EntityType::RasterImage(img) => Transformable::apply_transform(img, t),
            EntityType::Wipeout(wo) => Transformable::apply_transform(wo, t),
            EntityType::AttributeDefinition(a) => Transformable::apply_transform(a, t),
            EntityType::AttributeEntity(a) => Transformable::apply_transform(a, t),
            EntityType::MLine(ml) => Transformable::apply_transform(ml, t),
            EntityType::Tolerance(tol) => Transformable::apply_transform(tol, t),
            EntityType::Face3D(f) => Transformable::apply_transform(f, t),
            EntityType::PolygonMesh(pm) => Transformable::apply_transform(pm, t),
            EntityType::PolyfaceMesh(pfm) => Transformable::apply_transform(pfm, t),
            EntityType::Table(tbl) => Transformable::apply_transform(tbl, t),
            EntityType::MText(text) => Transformable::apply_transform(text, t),
            EntityType::Text(text) => Transformable::apply_transform(text, t),
            EntityType::Viewport(vp) => Transformable::apply_transform(vp, t),
            EntityType::Dimension(dim) => Transformable::apply_transform(dim, t),
            EntityType::Leader(leader) => Transformable::apply_transform(leader, t),
            EntityType::MultiLeader(ml) => Transformable::apply_transform(ml, t),
            EntityType::Underlay(ul) => Transformable::apply_transform(ul, t),
            _ => {}
        }
    }
}
