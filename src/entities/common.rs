use glam::Vec3;

use crate::scene::object::{GripDef, GripShape, PropValue, Property};

pub fn square_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Square,
    }
}

pub fn diamond_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: true,
        shape: GripShape::Diamond,
    }
}

pub fn triangle_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Triangle,
    }
}

pub fn edit_prop(label: &'static str, field: &'static str, value: f64) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::EditText(format!("{value:.4}")),
    }
}

pub fn ro_prop(label: &'static str, field: &'static str, value: impl Into<String>) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::ReadOnly(value.into()),
    }
}

pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}
