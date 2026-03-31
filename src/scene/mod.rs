pub mod acad_to_truck;
mod camera;
pub mod complex_lt;
pub mod cxf;
pub mod dispatch;
pub mod grip;
pub mod hatch_model;
pub mod hatch_patterns;
pub mod hit_test;
pub mod mesh_model;
pub mod object;
pub mod pipeline;
pub mod properties;
mod render;
mod selection;
pub mod tessellate;
pub mod transform;
pub mod truck_tess;
pub mod wire_model;

use camera::Camera;
pub use camera::Projection;
pub use hatch_model::HatchModel;
pub use mesh_model::MeshModel;
pub use object::{GripApply, GripDef};
pub use pipeline::uniforms::Uniforms;
pub use pipeline::viewcube::{
    hit_test, CubeRegion, VIEWCUBE_DRAW_PX, VIEWCUBE_PAD, VIEWCUBE_PX,
};
pub use selection::SelectionState;
pub use wire_model::WireModel;

use crate::command::EntityTransform;
use acadrust::entities::{BoundaryEdge, BoundaryPath, Hatch as DxfHatch, PolylineEdge};
use acadrust::entities::{Block, BlockEnd, Insert as DxfInsert};
use acadrust::objects::ObjectType;
use acadrust::types::Vector2;
use acadrust::{CadDocument, EntityType, Handle, TableEntry};
use glam;

use iced::time::Duration;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Scene {
    pub camera: Rc<RefCell<Camera>>,
    pub selection: Rc<RefCell<SelectionState>>,
    /// The CAD document — single source of truth for all entities.
    pub document: CadDocument,
    /// Currently selected entity handles.
    pub selected: HashSet<Handle>,
    /// In-progress preview wires while a command is active (rubber-band + object ghosts).
    pub preview_wires: Vec<WireModel>,
    /// Committed-segment wire drawn during multi-point commands (normal colour).
    pub interim_wire: Option<WireModel>,
    pub camera_generation: u64,
    /// Active layout name — "Model" or a paper space layout name.
    pub current_layout: String,
    /// GPU render data for hatch fills, keyed by the DXF entity Handle.
    pub hatches: HashMap<Handle, HatchModel>,
    /// GPU render data for solid meshes (truck Shell/Solid tessellation).
    pub meshes: HashMap<Handle, MeshModel>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            camera: Rc::new(RefCell::new(Camera::default())),
            selection: Rc::new(RefCell::new(SelectionState::default())),
            document: CadDocument::new(),
            selected: HashSet::new(),
            preview_wires: vec![],
            interim_wire: None,
            camera_generation: 0,
            current_layout: "Model".to_string(),
            hatches: HashMap::new(),
            meshes: HashMap::new(),
        }
    }

    /// Returns the block-record handle for `current_layout`.
    fn current_layout_block_handle(&self) -> Handle {
        self.document
            .objects
            .values()
            .find_map(|obj| match obj {
                ObjectType::Layout(l) if l.name == self.current_layout => Some(l.block_record),
                _ => None,
            })
            .unwrap_or(Handle::NULL)
    }

    /// Sorted list of layout names: "Model" first, then paper layouts by tab order.
    pub fn layout_names(&self) -> Vec<String> {
        let mut names = vec!["Model".to_string()];
        let mut paper: Vec<(i16, String)> = self
            .document
            .objects
            .values()
            .filter_map(|obj| match obj {
                ObjectType::Layout(l) if l.name != "Model" => Some((l.tab_order, l.name.clone())),
                _ => None,
            })
            .collect();
        paper.sort_by_key(|(order, _)| *order);
        names.extend(paper.into_iter().map(|(_, n)| n));
        names
    }

    /// Collect closed polygon outlines (world XY) from the current layout.
    pub fn closed_outlines(&self) -> Vec<Vec<[f32; 2]>> {
        self.entity_wires()
            .into_iter()
            .filter_map(|wire| {
                let pts = wire.points;
                if pts.len() < 4 {
                    return None;
                }
                let f = pts.first()?;
                let l = pts.last()?;
                let dx = f[0] - l[0];
                let dy = f[1] - l[1];
                if (dx * dx + dy * dy).sqrt() > 1e-2 {
                    return None;
                }
                Some(pts.iter().map(|p| [p[0], p[1]]).collect())
            })
            .collect()
    }

    /// Build WireModels from all document entities + optional preview wire.
    pub fn entity_wires(&self) -> Vec<WireModel> {
        let layout_block = self.current_layout_block_handle();
        let mut wires: Vec<WireModel> = self.wires_for_block(layout_block);
        if self.current_layout != "Model" {
            wires.extend(self.viewport_content_wires(layout_block));
        }
        wires
    }

    /// Tessellate all non-invisible entities owned by `block_handle`.
    fn wires_for_block(&self, block_handle: Handle) -> Vec<WireModel> {
        self.document
            .entities()
            .filter(|e| {
                let c = e.common();
                if c.invisible {
                    return false;
                }
                if self
                    .document
                    .layers
                    .get(&c.layer)
                    .map(|l| l.flags.off || l.flags.frozen)
                    .unwrap_or(false)
                {
                    return false;
                }
                self.belongs_to_visible_block(e.common().handle, c.owner_handle, block_handle)
            })
            .flat_map(|e| self.tessellate_one(e))
            .collect()
    }

    /// Decide whether an entity should be drawn as direct content of `block_handle`.
    ///
    /// Normal case: entity.owner_handle equals the active layout/model block.
    /// Fallback: if owner is null, allow it only when the handle is not listed
    /// under any other block record. This prevents block-definition geometry
    /// from leaking into the viewport when malformed files omit owner handles.
    fn belongs_to_visible_block(
        &self,
        entity_handle: Handle,
        owner_handle: Handle,
        block_handle: Handle,
    ) -> bool {
        if block_handle.is_null() {
            return true;
        }
        if owner_handle == block_handle {
            return true;
        }
        if !owner_handle.is_null() {
            return false;
        }

        !self
            .document
            .block_records
            .iter()
            .filter(|br| br.handle != block_handle)
            .any(|br| br.entity_handles.contains(&entity_handle))
    }

    /// Full tessellation pipeline for one entity.
    fn tessellate_one(&self, e: &EntityType) -> Vec<WireModel> {
        let h = e.common().handle;
        let sel = self.selected.contains(&h);

        if let EntityType::Viewport(vp) = e {
            let color = if vp.id == 1 {
                [0.40, 0.40, 0.40, 1.0]
            } else {
                [0.0, 0.75, 0.75, 1.0]
            };
            return vec![tessellate::tessellate(
                &self.document,
                h,
                e,
                sel,
                color,
                0.0,
                [0.0f32; 8],
                1.0,
            )];
        }

        let (entity_color, pattern_length, pattern, line_weight_px) = self.render_style(e);
        let lt_scale = e.common().linetype_scale as f32;
        let lt_name = self.resolved_linetype_name(e);

        if let EntityType::Dimension(dim) = e {
            return tessellate::tessellate_dimension(
                &self.document,
                h,
                dim,
                sel,
                entity_color,
                line_weight_px,
            );
        }

        if let EntityType::Insert(ins) = e {
            return ins
                .explode_from_document(&self.document)
                .iter()
                .cloned()
                .map(crate::modules::home::modify::explode::normalize_insert_entity)
                .flat_map(|sub| {
                    let (sub_color, sub_pattern_length, sub_pattern, sub_line_weight_px) =
                        self.render_style(&sub);
                    let mut wire = tessellate::tessellate(
                        &self.document,
                        h,
                        &sub,
                        sel,
                        sub_color,
                        sub_pattern_length,
                        sub_pattern,
                        sub_line_weight_px,
                    );
                    wire.name = h.value().to_string();
                    vec![wire]
                })
                .collect();
        }

        let base = tessellate::tessellate(
            &self.document,
            h,
            e,
            sel,
            entity_color,
            pattern_length,
            pattern,
            line_weight_px,
        );

        if let Some(clt) = crate::linetypes::complex_lt(lt_name) {
            let wires = complex_lt::apply_along(
                &base.name,
                &base.points,
                clt,
                lt_scale.max(1e-4),
                entity_color,
                sel,
                base.line_weight_px,
            );
            if !wires.is_empty() {
                return wires;
            }
        }

        vec![base]
    }

    fn model_space_block_handle(&self) -> Handle {
        self.document
            .objects
            .values()
            .find_map(|obj| match obj {
                ObjectType::Layout(l) if l.name == "Model" => Some(l.block_record),
                _ => None,
            })
            .unwrap_or(Handle::NULL)
    }

    fn viewport_content_wires(&self, paper_block: Handle) -> Vec<WireModel> {
        use acadrust::entities::Viewport;

        let viewports: Vec<&Viewport> = self
            .document
            .entities()
            .filter_map(|e| {
                if let EntityType::Viewport(vp) = e {
                    Some(vp)
                } else {
                    None
                }
            })
            .filter(|vp| vp.id > 1 && vp.common.owner_handle == paper_block)
            .collect();

        if viewports.is_empty() {
            return vec![];
        }

        let model_block = self.model_space_block_handle();
        let model_wires: Vec<WireModel> = self
            .document
            .entities()
            .filter(|e| {
                let c = e.common();
                if c.invisible || matches!(e, EntityType::Viewport(_)) {
                    return false;
                }
                if self
                    .document
                    .layers
                    .get(&c.layer)
                    .map(|l| l.flags.off || l.flags.frozen)
                    .unwrap_or(false)
                {
                    return false;
                }
                self.belongs_to_visible_block(c.handle, c.owner_handle, model_block)
            })
            .flat_map(|e| self.tessellate_one(e))
            .collect();

        let mut result = Vec::new();

        for vp in viewports {
            let scale = if vp.custom_scale.abs() > 1e-9 {
                vp.custom_scale as f32
            } else if vp.view_height.abs() > 1e-9 {
                (vp.height / vp.view_height) as f32
            } else {
                1.0
            };

            let tx = vp.view_target.x as f32;
            let ty = vp.view_target.y as f32;
            let pcx = vp.center.x as f32;
            let pcy = vp.center.y as f32;
            let pcz = vp.center.z as f32;
            let hw = (vp.width / 2.0) as f32;
            let hh = (vp.height / 2.0) as f32;

            for wire in &model_wires {
                let pts: Vec<[f32; 3]> = wire
                    .points
                    .iter()
                    .map(|&[mx, my, _mz]| [pcx + (mx - tx) * scale, pcy + (my - ty) * scale, pcz])
                    .collect();

                if !pts.is_empty() {
                    let n = pts.len() as f32;
                    let cx = pts.iter().map(|p| p[0]).sum::<f32>() / n;
                    let cy = pts.iter().map(|p| p[1]).sum::<f32>() / n;
                    if (cx - pcx).abs() > hw * 1.2 || (cy - pcy).abs() > hh * 1.2 {
                        continue;
                    }
                }

                let [r, g, b, a] = wire.color;
                let mut projected = wire.clone();
                projected.points = pts;
                projected.color = [r * 0.80, g * 0.80, b * 0.80, a * 0.85];
                result.push(projected);
            }
        }

        result
    }

    // ── Layout management ─────────────────────────────────────────────────

    /// Rename a paper-space layout.  Updates the Layout object name in the document.
    pub fn rename_layout(&mut self, old_name: &str, new_name: &str) {
        for obj in self.document.objects.values_mut() {
            if let ObjectType::Layout(l) = obj {
                if l.name == old_name {
                    l.name = new_name.to_string();
                    return;
                }
            }
        }
    }

    /// Delete a paper-space layout and all entities owned by it.
    /// Returns `false` if the layout was not found or is "Model".
    pub fn delete_layout(&mut self, name: &str) -> bool {
        if name == "Model" {
            return false;
        }

        let layout_info = self.document.objects.values().find_map(|obj| {
            if let ObjectType::Layout(l) = obj {
                if l.name == name {
                    return Some((l.handle, l.block_record));
                }
            }
            None
        });

        let (layout_handle, block_handle) = match layout_info {
            Some(info) => info,
            None => return false,
        };

        // Remove all entities that belong to this layout's block record.
        let to_remove: Vec<Handle> = self
            .document
            .entities()
            .filter(|e| e.common().owner_handle == block_handle)
            .map(|e| e.common().handle)
            .collect();
        for h in &to_remove {
            self.hatches.remove(h);
            self.meshes.remove(h);
            self.document.remove_entity(*h);
        }

        // Remove the Layout object itself.
        self.document.objects.remove(&layout_handle);

        // If the deleted layout was active, fall back to Model space.
        if self.current_layout == name {
            self.current_layout = "Model".to_string();
        }

        true
    }

    // ── Entity management ─────────────────────────────────────────────────

    pub fn add_entity(&mut self, entity: EntityType) -> Handle {
        let hatch_seed = if let EntityType::Hatch(dxf) = &entity {
            let color = self.render_style(&entity).0;
            Self::hatch_model_from_dxf(dxf, color)
        } else {
            None
        };

        let handle = self.document.add_entity(entity).unwrap_or(Handle::NULL);
        if !handle.is_null() {
            if let Some(model) = hatch_seed {
                self.hatches.insert(handle, model);
            }
        }
        handle
    }

    pub fn custom_block_names(&self) -> Vec<String> {
        self.document
            .block_records
            .iter()
            .filter(|br| !br.is_standard() && !br.is_layout())
            .map(|br| br.name.clone())
            .collect()
    }

    pub fn create_block_from_entities(
        &mut self,
        handles: &[Handle],
        name: &str,
        base: glam::Vec3,
    ) -> Result<Handle, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Block name cannot be empty.".into());
        }
        if name.starts_with('*') {
            return Err("Block name cannot start with '*'.".into());
        }
        if self.document.block_records.get(name).is_some() {
            return Err(format!("Block \"{name}\" already exists."));
        }

        let source_entities: Vec<_> = handles
            .iter()
            .filter_map(|&h| self.document.get_entity(h).cloned().map(|e| (h, e)))
            .collect();
        if source_entities.is_empty() {
            return Err("No valid entities selected for block creation.".into());
        }

        let next = self.document.next_handle();
        let br_handle = Handle::new(next);
        let block_handle = Handle::new(next + 1);
        let end_handle = Handle::new(next + 2);

        let mut block_record = acadrust::tables::BlockRecord::new(name);
        block_record.handle = br_handle;
        block_record.block_entity_handle = block_handle;
        block_record.block_end_handle = end_handle;
        self.document.block_records.add(block_record).map_err(|e| e.to_string())?;

        let mut block = Block::new(
            name,
            acadrust::types::Vector3::ZERO,
        );
        block.common.handle = block_handle;
        block.common.owner_handle = br_handle;
        self.document
            .add_entity(EntityType::Block(block))
            .map_err(|e| e.to_string())?;

        let mut block_end = BlockEnd::new();
        block_end.common.handle = end_handle;
        block_end.common.owner_handle = br_handle;
        self.document
            .add_entity(EntityType::BlockEnd(block_end))
            .map_err(|e| e.to_string())?;

        let local = EntityTransform::Translate(-base);
        for (old_handle, mut entity) in source_entities {
            dispatch::apply_transform(&mut entity, &local);
            entity = crate::modules::home::modify::explode::normalize_entity_for_block(entity);
            entity.common_mut().handle = Handle::NULL;
            entity.common_mut().owner_handle = br_handle;
            self.document
                .add_entity(entity)
                .map_err(|e| e.to_string())?;
            self.erase_entities(&[old_handle]);
        }

        let insert = DxfInsert::new(
            name,
            acadrust::types::Vector3::new(base.x as f64, base.y as f64, base.z as f64),
        );
        Ok(self.add_entity(EntityType::Insert(insert)))
    }

    fn synced_hatch_models(&self) -> Vec<HatchModel> {
        let mut models: Vec<HatchModel> = self
            .hatches
            .iter()
            .map(|(&handle, model)| {
                let mut m = if let Some(EntityType::Hatch(dxf)) = self.document.get_entity(handle) {
                    let mut m = model.clone();
                    match &mut m.pattern {
                        hatch_model::HatchPattern::Pattern(_) => {
                            m.angle_offset = dxf.pattern_angle as f32;
                            m.scale = dxf.pattern_scale as f32;
                        }
                        hatch_model::HatchPattern::Gradient { angle_deg, .. } => {
                            *angle_deg = dxf.pattern_angle.to_degrees() as f32;
                        }
                        hatch_model::HatchPattern::Solid => {}
                    }
                    m
                } else {
                    model.clone()
                };
                if self.selected.contains(&handle) {
                    m.color = [0.15, 0.55, 1.00, m.color[3]];
                }
                m
            })
            .collect();

        for entity in self.document.entities() {
            let EntityType::Insert(ins) = entity else {
                continue;
            };
            let selected = self.selected.contains(&ins.common.handle);
            for sub in ins
                .explode_from_document(&self.document)
                .into_iter()
                .map(crate::modules::home::modify::explode::normalize_insert_entity)
            {
                let EntityType::Hatch(dxf) = sub else {
                    continue;
                };
                let color = self.render_style(&EntityType::Hatch(dxf.clone())).0;
                if let Some(mut model) = Self::hatch_model_from_dxf(&dxf, color) {
                    if selected {
                        model.color = [0.15, 0.55, 1.00, model.color[3]];
                    }
                    models.push(model);
                }
            }
        }

        models
    }

    fn hatch_model_from_dxf(dxf: &DxfHatch, color: [f32; 4]) -> Option<HatchModel> {
        let path = dxf
            .paths
            .iter()
            .find(|p| p.flags.is_external())
            .or_else(|| dxf.paths.first())?;

        let mut boundary: Vec<[f32; 2]> = Vec::new();

        for edge in &path.edges {
            match edge {
                BoundaryEdge::Polyline(poly) => {
                    let verts = &poly.vertices;
                    let count = verts.len();
                    let seg_count = if poly.is_closed {
                        count
                    } else {
                        count.saturating_sub(1)
                    };
                    for i in 0..seg_count {
                        let v0 = &verts[i];
                        let v1 = &verts[(i + 1) % count];
                        let bulge = v0.z;
                        if bulge.abs() < 1e-9 {
                            boundary.push([v0.x as f32, v0.y as f32]);
                        } else {
                            let p0 = [v0.x as f32, v0.y as f32];
                            let p1 = [v1.x as f32, v1.y as f32];
                            let angle = 4.0 * (bulge as f32).atan();
                            let dx = p1[0] - p0[0];
                            let dy = p1[1] - p0[1];
                            let d = (dx * dx + dy * dy).sqrt();
                            let r = (d / 2.0) / (angle / 2.0).sin().abs();
                            let mx = (p0[0] + p1[0]) * 0.5;
                            let my = (p0[1] + p1[1]) * 0.5;
                            let len = d.max(1e-9);
                            let px = -dy / len;
                            let py = dx / len;
                            let sign = if bulge > 0.0 { 1.0_f32 } else { -1.0_f32 };
                            let h = r - (r * r - d * d / 4.0).max(0.0).sqrt();
                            let cx = mx - sign * px * (r - h);
                            let cy = my - sign * py * (r - h);
                            let a0 = (p0[1] - cy).atan2(p0[0] - cx);
                            let a1 = (p1[1] - cy).atan2(p1[0] - cx);
                            let (sa, mut ea) = if bulge > 0.0 { (a0, a1) } else { (a1, a0) };
                            if ea < sa {
                                ea += std::f32::consts::TAU;
                            }
                            let span = ea - sa;
                            let segs = ((span.abs() / std::f32::consts::TAU) * 16.0)
                                .ceil()
                                .max(4.0) as u32;
                            for j in 0..segs {
                                let t = sa + span * (j as f32 / segs as f32);
                                boundary.push([cx + r * t.cos(), cy + r * t.sin()]);
                            }
                        }
                    }
                    if poly.is_closed {
                        if let Some(&first) = boundary.first() {
                            boundary.push(first);
                        }
                    }
                }
                BoundaryEdge::Line(line) => {
                    boundary.push([line.start.x as f32, line.start.y as f32]);
                    boundary.push([line.end.x as f32, line.end.y as f32]);
                }
                BoundaryEdge::CircularArc(arc) => {
                    let cx = arc.center.x as f32;
                    let cy = arc.center.y as f32;
                    let r = arc.radius as f32;
                    let (sa, ea) = if arc.counter_clockwise {
                        (arc.start_angle as f32, arc.end_angle as f32)
                    } else {
                        (arc.end_angle as f32, arc.start_angle as f32)
                    };
                    let mut end = ea;
                    if end < sa {
                        end += std::f32::consts::TAU;
                    }
                    let span = end - sa;
                    let segs = ((span / std::f32::consts::TAU) * 32.0).ceil().max(4.0) as u32;
                    for i in 0..=segs {
                        let t = sa + span * (i as f32 / segs as f32);
                        boundary.push([cx + r * t.cos(), cy + r * t.sin()]);
                    }
                }
                BoundaryEdge::EllipticArc(ell) => {
                    let cx = ell.center.x as f32;
                    let cy = ell.center.y as f32;
                    let maj_x = ell.major_axis_endpoint.x as f32;
                    let maj_y = ell.major_axis_endpoint.y as f32;
                    let r_maj = (maj_x * maj_x + maj_y * maj_y).sqrt();
                    let r_min = r_maj * ell.minor_axis_ratio as f32;
                    let rot = maj_y.atan2(maj_x);
                    let (sa, ea) = if ell.counter_clockwise {
                        (ell.start_angle as f32, ell.end_angle as f32)
                    } else {
                        (ell.end_angle as f32, ell.start_angle as f32)
                    };
                    let mut end = ea;
                    if end < sa {
                        end += std::f32::consts::TAU;
                    }
                    let span = end - sa;
                    let segs = ((span / std::f32::consts::TAU) * 32.0).ceil().max(4.0) as u32;
                    for i in 0..=segs {
                        let t = sa + span * (i as f32 / segs as f32);
                        let lx = r_maj * t.cos();
                        let ly = r_min * t.sin();
                        boundary.push([
                            cx + lx * rot.cos() - ly * rot.sin(),
                            cy + lx * rot.sin() + ly * rot.cos(),
                        ]);
                    }
                }
                BoundaryEdge::Spline(spline) => {
                    for cp in &spline.control_points {
                        boundary.push([cp.x as f32, cp.y as f32]);
                    }
                    if boundary.len() > 1 {
                        if let Some(&first) = boundary.first() {
                            boundary.push(first);
                        }
                    }
                }
            }
        }

        if boundary.is_empty() {
            return None;
        }
        boundary.truncate(64);

        let pattern = if dxf.gradient_color.is_enabled() {
            let color2 = dxf
                .gradient_color
                .colors
                .get(1)
                .and_then(|e| e.color.rgb())
                .map(|(r, g, b)| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
                .unwrap_or(color);
            let angle_deg = dxf.pattern_angle.to_degrees() as f32;
            hatch_model::HatchPattern::Gradient { angle_deg, color2 }
        } else if dxf.is_solid {
            hatch_model::HatchPattern::Solid
        } else {
            let pat_name = &dxf.pattern.name;
            if let Some(entry) = crate::scene::hatch_patterns::find(pat_name) {
                entry.gpu.clone()
            } else {
                hatch_model::HatchPattern::Pattern(vec![hatch_model::PatFamily {
                    angle_deg: 0.0,
                    x0: 0.0,
                    y0: 0.0,
                    dx: 0.0,
                    dy: 5.0 * dxf.pattern_scale as f32,
                    dashes: vec![],
                }])
            }
        };

        let name = if dxf.gradient_color.is_enabled() {
            dxf.gradient_color.name.clone()
        } else if dxf.is_solid {
            "SOLID".into()
        } else {
            dxf.pattern.name.clone()
        };
        Some(HatchModel {
            boundary,
            pattern,
            name,
            color,
            angle_offset: 0.0,
            scale: 1.0,
        })
    }

    pub fn populate_hatches_from_document(&mut self) {
        self.hatches.clear();

        let entries: Vec<(Handle, [f32; 4])> = self
            .document
            .entities()
            .filter_map(|e| {
                if let EntityType::Hatch(dxf) = e {
                    let color = tessellate::aci_to_rgba(&dxf.common.color);
                    Some((dxf.common.handle, color))
                } else {
                    None
                }
            })
            .collect();

        for (handle, color) in entries {
            if let Some(EntityType::Hatch(dxf)) = self.document.get_entity(handle) {
                if let Some(model) = Self::hatch_model_from_dxf(dxf, color) {
                    self.hatches.insert(handle, model);
                }
            }
        }
    }

    pub fn add_hatch(&mut self, model: HatchModel) -> Handle {
        let mut dxf = DxfHatch::new();
        dxf.is_solid = matches!(
            model.pattern,
            crate::scene::hatch_model::HatchPattern::Solid
        );
        let verts: Vec<Vector2> = model
            .boundary
            .iter()
            .map(|&[x, y]| Vector2::new(x as f64, y as f64))
            .collect();
        let edge = PolylineEdge::new(verts, true);
        let mut path = BoundaryPath::external();
        path.add_edge(BoundaryEdge::Polyline(edge));
        dxf.paths.push(path);
        if let Some(entry) = crate::scene::hatch_patterns::find(&model.name) {
            dxf.pattern = crate::scene::hatch_patterns::build_dxf_pattern(entry);
        }
        dxf.pattern_angle = model.angle_offset as f64;
        dxf.pattern_scale = if model.scale.abs() > 1e-6 {
            model.scale as f64
        } else {
            1.0
        };

        let handle = self.add_entity(EntityType::Hatch(dxf));
        if !handle.is_null() {
            self.hatches.insert(handle, model);
        }
        handle
    }

    pub fn clear(&mut self) {
        self.document = CadDocument::new();
        self.selected = HashSet::new();
        self.preview_wires = vec![];
        self.current_layout = "Model".to_string();
        self.hatches = HashMap::new();
        self.meshes = HashMap::new();
        *self.camera.borrow_mut() = Camera::default();
        self.camera_generation += 1;
    }

    // ── Preview wire ──────────────────────────────────────────────────────

    pub fn set_preview_wires(&mut self, wires: Vec<WireModel>) {
        self.preview_wires = wires;
    }

    pub fn clear_preview_wire(&mut self) {
        self.preview_wires = vec![];
        self.interim_wire = None;
    }

    pub fn wire_models_for(&self, handles: &[acadrust::Handle]) -> Vec<WireModel> {
        handles
            .iter()
            .flat_map(|h| {
                self.document
                    .entities()
                    .find(|e| e.common().handle == *h)
                    .map(|e| self.tessellate_one(e))
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Build wire models for an arbitrary slice of entities (e.g. clipboard contents).
    /// Entities need not be in the document — they are tessellated directly.
    pub fn wires_for_entities(&self, entities: &[acadrust::EntityType]) -> Vec<WireModel> {
        entities
            .iter()
            .flat_map(|e| self.tessellate_one(e))
            .collect()
    }

    pub fn set_interim_wire(&mut self, w: WireModel) {
        self.interim_wire = Some(w);
    }

    // ── Selection ─────────────────────────────────────────────────────────

    pub fn select_entity(&mut self, handle: Handle, exclusive: bool) {
        if exclusive {
            self.selected.clear();
        }
        self.selected.insert(handle);
    }

    pub fn deselect_all(&mut self) {
        self.selected.clear();
    }

    pub fn selected_entities(&self) -> Vec<(Handle, &EntityType)> {
        self.selected
            .iter()
            .filter_map(|&h| self.document.get_entity(h).map(|e| (h, e)))
            .collect()
    }

    // ── Erase ─────────────────────────────────────────────────────────────

    pub fn erase_entities(&mut self, handles: &[Handle]) {
        for &h in handles {
            self.document.remove_entity(h);
            self.selected.remove(&h);
            self.hatches.remove(&h);
        }
        // Remove erased handles from all groups; delete groups that become empty.
        let group_dict_handle = self.document.header.acad_group_dict_handle;
        let to_remove: Vec<Handle> = self
            .document
            .objects
            .values_mut()
            .filter_map(|obj| match obj {
                ObjectType::Group(g) => {
                    g.entities.retain(|h| !handles.contains(h));
                    if g.entities.is_empty() { Some(g.handle) } else { None }
                }
                _ => None,
            })
            .collect();
        for gh in &to_remove {
            if let Some(ObjectType::Dictionary(dict)) =
                self.document.objects.get_mut(&group_dict_handle)
            {
                dict.entries.retain(|(_, h)| h != gh);
            }
            self.document.objects.remove(gh);
        }
    }

    // ── Group helpers ──────────────────────────────────────────────────────

    pub fn groups(&self) -> impl Iterator<Item = &acadrust::objects::Group> {
        self.document.objects.values().filter_map(|obj| match obj {
            ObjectType::Group(g) => Some(g),
            _ => None,
        })
    }

    /// Returns the names of all groups that contain `handle`.
    pub fn group_names_for_entity(&self, handle: Handle) -> Vec<String> {
        self.groups()
            .filter(|g| g.contains(handle))
            .map(|g| g.name.clone())
            .collect()
    }

    /// Creates a named group from the given handles and registers it in the group dictionary.
    pub fn create_group(&mut self, name: String, handles: Vec<Handle>) -> Handle {
        let group_dict_handle = self.document.header.acad_group_dict_handle;
        let mut group = acadrust::objects::Group::new(&name);
        group.handle = self.document.allocate_handle();
        group.owner = group_dict_handle;
        group.add_entities(handles);
        let gh = group.handle;
        self.document.objects.insert(gh, ObjectType::Group(group));
        if let Some(ObjectType::Dictionary(dict)) =
            self.document.objects.get_mut(&group_dict_handle)
        {
            dict.add_entry(&name, gh);
        }
        gh
    }

    /// Dissolves all groups that contain any of the given handles.
    /// Returns the number of groups removed.
    pub fn delete_groups_containing(&mut self, handles: &[Handle]) -> usize {
        let group_dict_handle = self.document.header.acad_group_dict_handle;
        let to_delete: Vec<Handle> = self
            .document
            .objects
            .values()
            .filter_map(|obj| match obj {
                ObjectType::Group(g) if handles.iter().any(|h| g.contains(*h)) => {
                    Some(g.handle)
                }
                _ => None,
            })
            .collect();
        let count = to_delete.len();
        for gh in &to_delete {
            if let Some(ObjectType::Dictionary(dict)) =
                self.document.objects.get_mut(&group_dict_handle)
            {
                dict.entries.retain(|(_, h)| h != gh);
            }
            self.document.objects.remove(gh);
        }
        count
    }

    /// If `handle` belongs to any selectable groups, also select all other members of those groups.
    pub fn expand_selection_for_groups(&mut self, handles: &[Handle]) {
        let to_add: Vec<Handle> = self
            .document
            .objects
            .values()
            .filter_map(|obj| match obj {
                ObjectType::Group(g)
                    if g.selectable && handles.iter().any(|h| g.contains(*h)) =>
                {
                    Some(g.entities.clone())
                }
                _ => None,
            })
            .flatten()
            .collect();
        for h in to_add {
            self.selected.insert(h);
        }
    }

    // ── Layer helpers ──────────────────────────────────────────────────────

    pub fn toggle_layer_visibility(&mut self, name: &str) {
        if let Some(layer) = self.document.layers.get_mut(name) {
            layer.flags.off = !layer.flags.off;
        }
    }

    pub fn toggle_layer_lock(&mut self, name: &str) {
        if let Some(layer) = self.document.layers.get_mut(name) {
            layer.flags.locked = !layer.flags.locked;
        }
    }

    // ── Modify (transform / copy) ─────────────────────────────────────────

    pub fn transform_entities(&mut self, handles: &[Handle], t: &EntityTransform) {
        for &h in handles {
            if let Some(entity) = self.document.get_entity_mut(h) {
                dispatch::apply_transform(entity, t);
            }
            if self.hatches.contains_key(&h) {
                let existing_color = self.hatches[&h].color;
                let new_model = if let Some(EntityType::Hatch(dxf)) = self.document.get_entity(h) {
                    Self::hatch_model_from_dxf(dxf, existing_color)
                } else {
                    None
                };
                if let Some(model) = new_model {
                    self.hatches.insert(h, model);
                }
            }
        }
    }

    pub fn copy_entities(&mut self, handles: &[Handle], t: &EntityTransform) -> Vec<Handle> {
        let clones: Vec<EntityType> = handles
            .iter()
            .filter_map(|&h| self.document.get_entity(h).cloned())
            .collect();
        let mut new_handles = Vec::with_capacity(clones.len());
        for mut entity in clones {
            dispatch::apply_transform(&mut entity, t);
            entity.common_mut().handle = Handle::NULL;
            let h = self.document.add_entity(entity).unwrap_or(Handle::NULL);
            if !h.is_null() {
                let new_model = if let Some(EntityType::Hatch(dxf)) = self.document.get_entity(h) {
                    let color = tessellate::aci_to_rgba(&dxf.common.color);
                    Self::hatch_model_from_dxf(dxf, color)
                } else {
                    None
                };
                if let Some(model) = new_model {
                    self.hatches.insert(h, model);
                }
            }
            new_handles.push(h);
        }
        new_handles
    }

    // ── Grip editing ──────────────────────────────────────────────────────

    pub fn apply_grip(&mut self, handle: Handle, grip_id: usize, apply: GripApply) {
        if let Some(entity) = self.document.get_entity_mut(handle) {
            dispatch::apply_grip(entity, grip_id, apply);
        }
    }

    // ── Hit-test convenience: wire name → Handle ──────────────────────────

    pub fn handle_from_wire_name(name: &str) -> Option<Handle> {
        name.parse::<u64>().ok().map(Handle::new)
    }

    pub fn fit_all(&mut self) {
        let wires = self.entity_wires();
        if wires.is_empty() {
            return;
        }

        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        for wire in &wires {
            for &[x, y, z] in &wire.points {
                min = min.min(glam::Vec3::new(x, y, z));
                max = max.max(glam::Vec3::new(x, y, z));
            }
        }
        if min == max {
            max += glam::Vec3::splat(1.0);
        }
        self.camera.borrow_mut().fit_to_bounds(min, max);
        self.camera_generation += 1;
    }

    pub fn update(&mut self, _dt: Duration) {}
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
