// acadrust -> truck topology conversion layer.

use acadrust::{CadDocument, EntityType};
use glam::Vec3;
use truck_modeling::{Edge, Solid, Vertex, Wire};

use crate::entities::traits::EntityTypeOps;
use crate::scene::wire_model::{SnapHint, TangentGeom};

#[allow(dead_code)]
pub enum TruckObject {
    Point(Vertex),
    Curve(Edge),
    Contour(Wire),
    Text(Vec<Vec<[f32; 2]>>),
    /// Pre-computed NaN-separated 3-D point list (leader lines, arrowheads, etc.).
    Lines(Vec<[f32; 3]>),
    Volume(Solid),
}

pub struct TruckEntity {
    pub object: TruckObject,
    pub snap_pts: Vec<(Vec3, SnapHint)>,
    pub tangent_geoms: Vec<TangentGeom>,
    pub key_vertices: Vec<[f32; 3]>,
}

pub fn convert(entity: &EntityType, document: &CadDocument) -> Option<TruckEntity> {
    entity.to_truck_entity(document)
}
