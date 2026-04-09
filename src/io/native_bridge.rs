//! Bridge between h7cad-native-model and acadrust type systems.

use acadrust::entities as ar;
use acadrust::types::{Color, Handle, LineWeight, Vector3};
use h7cad_native_model as nm;

pub fn native_doc_to_acadrust(native: &nm::CadDocument) -> acadrust::CadDocument {
    let mut doc = acadrust::CadDocument::new();

    for entity in &native.entities {
        if let Some(ar_entity) = convert_entity(entity) {
            doc.add_entity(ar_entity);
        }
    }

    for (name, layer) in &native.layers {
        let mut ar_layer = acadrust::tables::Layer::new(name.clone());
        ar_layer.color = Color::from_index(layer.color);
        doc.layers.add_or_replace(ar_layer);
    }

    doc
}

fn apply_common(common: &mut ar::EntityCommon, entity: &nm::Entity) {
    common.handle = Handle::new(entity.handle.value());
    common.layer = entity.layer_name.clone();
    common.color = if entity.true_color != 0 {
        Color::from_rgb(
            ((entity.true_color >> 16) & 0xFF) as u8,
            ((entity.true_color >> 8) & 0xFF) as u8,
            (entity.true_color & 0xFF) as u8,
        )
    } else {
        Color::from_index(entity.color_index)
    };
    common.line_weight = LineWeight::from_value(entity.lineweight);
    common.invisible = entity.invisible;
}

fn v3(arr: &[f64; 3]) -> Vector3 {
    Vector3::new(arr[0], arr[1], arr[2])
}

fn convert_entity(entity: &nm::Entity) -> Option<ar::EntityType> {
    match &entity.data {
        nm::EntityData::Line { start, end } => {
            let mut e = ar::Line::from_points(v3(start), v3(end));
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Line(e))
        }
        nm::EntityData::Circle { center, radius } => {
            let mut e = ar::Circle::new();
            e.center = v3(center);
            e.radius = *radius;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Circle(e))
        }
        nm::EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let mut e = ar::Arc::new();
            e.center = v3(center);
            e.radius = *radius;
            e.start_angle = start_angle.to_radians();
            e.end_angle = end_angle.to_radians();
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Arc(e))
        }
        nm::EntityData::Point { position, .. } => {
            let mut e = ar::Point::new();
            e.location = v3(position);
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Point(e))
        }
        nm::EntityData::Text {
            insertion,
            height,
            value,
            rotation,
            ..
        } => {
            let mut e = ar::Text::new();
            e.insertion_point = v3(insertion);
            e.height = *height;
            e.value = value.clone();
            e.rotation = *rotation;
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::Text(e))
        }
        nm::EntityData::MText {
            insertion,
            height,
            value,
            ..
        } => {
            let mut e = ar::MText::new();
            e.insertion_point = v3(insertion);
            e.height = *height;
            e.value = value.clone();
            apply_common(&mut e.common, entity);
            Some(ar::EntityType::MText(e))
        }
        _ => None,
    }
}
