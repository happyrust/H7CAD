use acadrust::{CadDocument, EntityType};

use crate::command::EntityTransform;
use crate::scene::acad_to_truck::TruckEntity;
use crate::scene::object::{GripApply, GripDef, PropSection};

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
            EntityType::Point(pt) => TruckConvertible::to_truck(pt, document),
            EntityType::Line(line) => TruckConvertible::to_truck(line, document),
            EntityType::Circle(circle) => TruckConvertible::to_truck(circle, document),
            EntityType::Arc(arc) => TruckConvertible::to_truck(arc, document),
            EntityType::Ellipse(ellipse) => TruckConvertible::to_truck(ellipse, document),
            EntityType::Spline(spline) => TruckConvertible::to_truck(spline, document),
            EntityType::LwPolyline(pline) => TruckConvertible::to_truck(pline, document),
            EntityType::Text(text) => TruckConvertible::to_truck(text, document),
            EntityType::MText(text) => TruckConvertible::to_truck(text, document),
            EntityType::Leader(leader) => TruckConvertible::to_truck(leader, document),
            EntityType::MultiLeader(ml) => TruckConvertible::to_truck(ml, document),
            _ => None,
        }
    }

    fn grips(&self) -> Vec<GripDef> {
        match self {
            EntityType::Line(line) => Grippable::grips(line),
            EntityType::Circle(circle) => Grippable::grips(circle),
            EntityType::Arc(arc) => Grippable::grips(arc),
            EntityType::Ellipse(ellipse) => Grippable::grips(ellipse),
            EntityType::LwPolyline(pline) => Grippable::grips(pline),
            EntityType::Point(pt) => Grippable::grips(pt),
            EntityType::Spline(spline) => Grippable::grips(spline),
            EntityType::Text(text) => Grippable::grips(text),
            EntityType::MText(text) => Grippable::grips(text),
            EntityType::Viewport(vp) => Grippable::grips(vp),
            EntityType::Insert(ins) => Grippable::grips(ins),
            EntityType::Leader(leader) => Grippable::grips(leader),
            EntityType::MultiLeader(ml) => Grippable::grips(ml),
            _ => vec![],
        }
    }

    fn geometry_properties(&self, text_style_names: &[String]) -> Option<PropSection> {
        match self {
            EntityType::Line(line) => Some(PropertyEditable::geometry_properties(
                line,
                text_style_names,
            )),
            EntityType::Circle(circle) => Some(PropertyEditable::geometry_properties(
                circle,
                text_style_names,
            )),
            EntityType::Arc(arc) => {
                Some(PropertyEditable::geometry_properties(arc, text_style_names))
            }
            EntityType::Ellipse(ellipse) => Some(PropertyEditable::geometry_properties(
                ellipse,
                text_style_names,
            )),
            EntityType::LwPolyline(pline) => Some(PropertyEditable::geometry_properties(
                pline,
                text_style_names,
            )),
            EntityType::Hatch(hatch) => Some(PropertyEditable::geometry_properties(
                hatch,
                text_style_names,
            )),
            EntityType::Point(pt) => {
                Some(PropertyEditable::geometry_properties(pt, text_style_names))
            }
            EntityType::Spline(spline) => Some(PropertyEditable::geometry_properties(
                spline,
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
            _ => None,
        }
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        match self {
            EntityType::Line(line) => PropertyEditable::apply_geom_prop(line, field, value),
            EntityType::Circle(circle) => PropertyEditable::apply_geom_prop(circle, field, value),
            EntityType::Arc(arc) => PropertyEditable::apply_geom_prop(arc, field, value),
            EntityType::Ellipse(ellipse) => {
                PropertyEditable::apply_geom_prop(ellipse, field, value)
            }
            EntityType::LwPolyline(pline) => PropertyEditable::apply_geom_prop(pline, field, value),
            EntityType::Hatch(hatch) => PropertyEditable::apply_geom_prop(hatch, field, value),
            EntityType::Point(pt) => PropertyEditable::apply_geom_prop(pt, field, value),
            EntityType::Spline(spline) => PropertyEditable::apply_geom_prop(spline, field, value),
            EntityType::Text(text) => PropertyEditable::apply_geom_prop(text, field, value),
            EntityType::MText(text) => PropertyEditable::apply_geom_prop(text, field, value),
            EntityType::Viewport(vp) => PropertyEditable::apply_geom_prop(vp, field, value),
            EntityType::Insert(ins) => PropertyEditable::apply_geom_prop(ins, field, value),
            EntityType::Dimension(dim) => PropertyEditable::apply_geom_prop(dim, field, value),
            EntityType::Leader(leader) => PropertyEditable::apply_geom_prop(leader, field, value),
            EntityType::MultiLeader(ml) => PropertyEditable::apply_geom_prop(ml, field, value),
            _ => {}
        }
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        match self {
            EntityType::Line(line) => Grippable::apply_grip(line, grip_id, apply),
            EntityType::Circle(circle) => Grippable::apply_grip(circle, grip_id, apply),
            EntityType::Arc(arc) => Grippable::apply_grip(arc, grip_id, apply),
            EntityType::Ellipse(ellipse) => Grippable::apply_grip(ellipse, grip_id, apply),
            EntityType::LwPolyline(pline) => Grippable::apply_grip(pline, grip_id, apply),
            EntityType::Point(pt) => Grippable::apply_grip(pt, grip_id, apply),
            EntityType::Spline(spline) => Grippable::apply_grip(spline, grip_id, apply),
            EntityType::Text(text) => Grippable::apply_grip(text, grip_id, apply),
            EntityType::MText(text) => Grippable::apply_grip(text, grip_id, apply),
            EntityType::Viewport(vp) => Grippable::apply_grip(vp, grip_id, apply),
            EntityType::Insert(ins) => Grippable::apply_grip(ins, grip_id, apply),
            EntityType::Leader(leader) => Grippable::apply_grip(leader, grip_id, apply),
            EntityType::MultiLeader(ml) => Grippable::apply_grip(ml, grip_id, apply),
            _ => {}
        }
    }

    fn apply_transform(&mut self, t: &EntityTransform) {
        match self {
            EntityType::Arc(arc) => Transformable::apply_transform(arc, t),
            EntityType::Circle(circle) => Transformable::apply_transform(circle, t),
            EntityType::Ellipse(ellipse) => Transformable::apply_transform(ellipse, t),
            EntityType::Hatch(hatch) => Transformable::apply_transform(hatch, t),
            EntityType::Insert(ins) => Transformable::apply_transform(ins, t),
            EntityType::Line(line) => Transformable::apply_transform(line, t),
            EntityType::LwPolyline(pline) => Transformable::apply_transform(pline, t),
            EntityType::MText(text) => Transformable::apply_transform(text, t),
            EntityType::Point(pt) => Transformable::apply_transform(pt, t),
            EntityType::Spline(spline) => Transformable::apply_transform(spline, t),
            EntityType::Text(text) => Transformable::apply_transform(text, t),
            EntityType::Viewport(vp) => Transformable::apply_transform(vp, t),
            EntityType::Dimension(dim) => Transformable::apply_transform(dim, t),
            EntityType::Leader(leader) => Transformable::apply_transform(leader, t),
            EntityType::MultiLeader(ml) => Transformable::apply_transform(ml, t),
            _ => {}
        }
    }
}
