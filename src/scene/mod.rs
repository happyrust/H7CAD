pub mod acad_to_truck;
mod camera;
pub mod complex_lt;
pub mod cxf;
pub mod dispatch;
pub mod grip;
pub mod hatch_model;
pub mod hatch_patterns;
pub mod hit_test;
pub mod image_model;
pub mod mesh_model;
pub mod object;
pub mod pipeline;
pub mod properties;
mod render;
mod selection;
pub mod solid3d_tess;
pub mod tessellate;
pub mod transform;
pub mod truck_tess;
pub mod wire_model;

use camera::Camera;
pub use camera::Projection;
pub use hatch_model::HatchModel;
pub use image_model::ImageModel;
pub use mesh_model::MeshModel;
pub use object::GripDef;
pub use pipeline::uniforms::Uniforms;
pub use pipeline::viewcube::{
    hit_test, CubeRegion, VIEWCUBE_DRAW_PX, VIEWCUBE_PAD, VIEWCUBE_PX,
};
pub use selection::SelectionState;
pub use wire_model::WireModel;
use wire_model::TangentGeom;

use crate::command::EntityTransform;
use crate::store::NativeStore;
use acadrust::entities::{BoundaryEdge, BoundaryPath, Hatch as DxfHatch, PolylineEdge, Solid as DxfSolid};
use acadrust::entities::{Block, BlockEnd, Insert as DxfInsert};
use acadrust::objects::ObjectType;
use crate::types::Vector2;
use acadrust::{CadDocument, EntityType, Handle, TableEntry};
use h7cad_native_model as nm;
use glam;

use iced::time::Duration;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Scene {
    pub camera: Rc<RefCell<Camera>>,
    pub selection: Rc<RefCell<SelectionState>>,
    /// The CAD document — compat projection kept during migration.
    pub document: CadDocument,
    /// Native document store — the forward-looking single source of truth.
    pub native_store: Option<NativeStore>,
    /// Temporary runtime switch for the native render debug path.
    pub native_render_enabled: bool,
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
    /// GPU render data for raster images (RasterImage entities), keyed by handle.
    pub images: HashMap<Handle, ImageModel>,
    /// The viewport that is currently "entered" (MSPACE mode).
    /// `None` = paper space editing (PSPACE).  Only meaningful when
    /// `current_layout != "Model"`.
    pub active_viewport: Option<Handle>,
    /// Custom model-space background fill color for Wipeout entities.
    /// Set from the active tab's `bg_color`; defaults to dark grey.
    pub bg_color: [f32; 4],
    /// Custom paper-space background fill color for Wipeout entities.
    pub paper_bg_color: [f32; 4],
    /// Whether the ViewCube overlay is rendered (mirrors `H7CAD.show_viewcube`).
    pub show_viewcube: bool,
    /// Underlay frame visibility: 0 = hidden, 1 = on, 2 = on + print.
    /// (FRAMES0 / FRAMES1 / FRAMES2 commands, mirrors `H7CAD.frames_mode`.)
    pub underlay_frames_mode: u8,
    /// Whether object snap targets Underlay entities (UOSNAP command,
    /// mirrors `H7CAD.uosnap`).
    pub underlay_snap_enabled: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            camera: Rc::new(RefCell::new(Camera::default())),
            selection: Rc::new(RefCell::new(SelectionState::default())),
            document: CadDocument::new(),
            native_store: None,
            native_render_enabled: false,
            selected: HashSet::new(),
            preview_wires: vec![],
            interim_wire: None,
            camera_generation: 0,
            current_layout: "Model".to_string(),
            hatches: HashMap::new(),
            meshes: HashMap::new(),
            images: HashMap::new(),
            active_viewport: None,
            bg_color: [0.11, 0.11, 0.11, 1.0],
            paper_bg_color: [0.22, 0.24, 0.28, 1.0],
            show_viewcube: true,
            underlay_frames_mode: 1,
            underlay_snap_enabled: true,
        }
    }

    pub fn native_doc(&self) -> Option<&nm::CadDocument> {
        self.native_store.as_ref().map(|s| s.inner())
    }

    pub fn native_doc_mut(&mut self) -> Option<&mut nm::CadDocument> {
        self.native_store.as_mut().map(|s| s.inner_mut())
    }

    pub fn set_native_doc(&mut self, doc: Option<nm::CadDocument>) {
        self.native_store = doc.map(NativeStore::new);
    }

    /// Public accessor for the block-record handle of the current layout.
    /// Used by external callers (e.g. `commit_entity`) that need the handle
    /// without going through private API.
    pub fn current_layout_block_handle_pub(&self) -> Handle {
        self.current_layout_block_handle()
    }

    /// Returns the block-record handle for `current_layout`.
    ///
    /// Primary path: the Layout object's `block_record` field (set correctly
    /// by the DWG reader).
    ///
    /// Fallback for DXF files: the DXF reader never reads group code 340
    /// (block_record handle), so `block_record` is NULL after loading DXF.
    /// In that case we derive the block-record name from the DXF convention:
    ///   Model            → "*Model_Space"
    ///   first paper tab  → "*Paper_Space"
    ///   second paper tab → "*Paper_Space0"
    ///   Nth paper tab    → "*Paper_Space{N-2}"
    fn current_layout_block_handle(&self) -> Handle {
        // Locate the Layout object for the active layout name.
        let layout = self.document.objects.values().find_map(|obj| {
            if let ObjectType::Layout(l) = obj {
                if l.name == self.current_layout { Some(l) } else { None }
            } else {
                None
            }
        });

        if let Some(l) = layout {
            // Fast path: block_record already set (DWG reader).
            if !l.block_record.is_null() {
                return l.block_record;
            }

            // Fallback: resolve via conventional DXF block-record name.
            let br_name: String = if self.current_layout == "Model" {
                "*Model_Space".into()
            } else {
                // tab_order 1 → "*Paper_Space",  2 → "*Paper_Space0", etc.
                let tab = l.tab_order;
                if tab <= 1 {
                    "*Paper_Space".into()
                } else {
                    format!("*Paper_Space{}", tab - 2)
                }
            };

            if let Some(br) = self.document.block_records.get(&br_name) {
                return br.handle;
            }

            // Last resort: match by position among paper layouts when tab_order
            // is unreliable (some exporters set it to 0 for all layouts).
            if self.current_layout != "Model" {
                let mut ps_brs: Vec<_> = self
                    .document
                    .block_records
                    .iter()
                    .filter(|br| br.is_paper_space())
                    .collect();
                ps_brs.sort_by(|a, b| a.name.cmp(&b.name));

                let mut paper_layouts: Vec<(i16, &str)> = self
                    .document
                    .objects
                    .values()
                    .filter_map(|obj| {
                        if let ObjectType::Layout(l) = obj {
                            if l.name != "Model" { Some((l.tab_order, l.name.as_str())) }
                            else { None }
                        } else {
                            None
                        }
                    })
                    .collect();
                paper_layouts.sort_by_key(|(o, n)| (*o, *n));

                if let Some(pos) = paper_layouts.iter().position(|(_, n)| *n == self.current_layout) {
                    if let Some(br) = ps_brs.get(pos) {
                        return br.handle;
                    }
                }
            } else if let Some(br) = self.document.block_records.get("*Model_Space") {
                return br.handle;
            }
        }

        Handle::NULL
    }

    /// Returns `(min, max)` paper-space limits for the current layout, or `None`
    /// when in Model space.  Falls back to `(0,0)-(12,9)` if the layout has
    /// zero-size limits (common in freshly-created layouts).
    pub fn paper_limits(&self) -> Option<((f64, f64), (f64, f64))> {
        if self.current_layout == "Model" {
            return None;
        }
        self.document.objects.values().find_map(|obj| {
            if let ObjectType::Layout(l) = obj {
                if l.name == self.current_layout {
                    let (min, max) = (l.min_limits, l.max_limits);
                    // Guard against degenerate limits.
                    let w = (max.0 - min.0).abs();
                    let h = (max.1 - min.1).abs();
                    if w < 1e-6 || h < 1e-6 {
                        // Default to A4 landscape (mm).
                        return Some(((0.0, 0.0), (297.0, 210.0)));
                    }
                    return Some((min, max));
                }
            }
            None
        })
    }

    /// Scale of the first user viewport (id > 1) in the current paper layout,
    /// used for the status-bar display.  Returns `None` in Model space or if
    /// no user viewport exists.
    pub fn first_viewport_scale(&self) -> Option<f64> {
        if self.current_layout == "Model" {
            return None;
        }
        let layout_block = self.current_layout_block_handle();
        if layout_block.is_null() {
            return None;
        }
        self.document.entities().find_map(|e| {
            if let EntityType::Viewport(vp) = e {
                if vp.id > 1 && vp.common.owner_handle == layout_block {
                    let scale = if vp.custom_scale.abs() > 1e-9 {
                        vp.custom_scale
                    } else if vp.view_height.abs() > 1e-9 {
                        vp.height / vp.view_height
                    } else {
                        1.0
                    };
                    return Some(scale);
                }
            }
            None
        })
    }

    /// List of user viewports in the current layout: (handle, label, frozen_layer_handles).
    pub fn viewport_list(&self) -> Vec<(acadrust::Handle, String, Vec<acadrust::Handle>)> {
        if self.current_layout == "Model" {
            return vec![];
        }
        let layout_block = self.current_layout_block_handle();
        if layout_block.is_null() {
            return vec![];
        }
        let mut result: Vec<(acadrust::Handle, String, Vec<acadrust::Handle>)> = self
            .document
            .entities()
            .filter_map(|e| {
                if let EntityType::Viewport(vp) = e {
                    if vp.id > 1 && vp.common.owner_handle == layout_block {
                        Some((vp.common.handle, vp.id, vp.frozen_layers.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(h, id, frozen)| (h, format!("VP {}", id - 1), frozen))
            .collect();
        result.sort_by_key(|(_, label, _)| label.clone());
        result
    }

    /// Count of user viewports (id > 1) in the current layout.
    pub fn viewport_count(&self) -> usize {
        if self.current_layout == "Model" {
            return 0;
        }
        let layout_block = self.current_layout_block_handle();
        if layout_block.is_null() {
            return 0;
        }
        self.document.entities().filter(|e| {
            if let EntityType::Viewport(vp) = e {
                vp.id > 1 && vp.common.owner_handle == layout_block
            } else {
                false
            }
        }).count()
    }

    /// Sorted list of layout names: "Model" first, then paper layouts by tab order.
    pub fn layout_names(&self) -> Vec<String> {
        let mut names = vec!["Model".to_string()];
        // Deduplicate by name: prefer the entry with a non-null block_record (the
        // real layout from the file) over the default placeholder created by
        // CadDocument::new().
        let mut by_name: std::collections::HashMap<String, (i16, Handle)> =
            Default::default();
        for obj in self.document.objects.values() {
            if let ObjectType::Layout(l) = obj {
                if l.name == "Model" || l.name.is_empty() {
                    continue;
                }
                let entry = by_name
                    .entry(l.name.clone())
                    .or_insert((l.tab_order, l.block_record));
                if entry.1.is_null() && !l.block_record.is_null() {
                    *entry = (l.tab_order, l.block_record);
                }
            }
        }
        let mut paper: Vec<(i16, String)> = by_name
            .into_iter()
            .map(|(name, (order, _))| (order, name))
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
            // Draw the paper boundary rectangle first (rendered beneath everything else).
            if let Some(((x0, y0), (x1, y1))) = self.paper_limits() {
                wires.insert(0, paper_boundary_wire(x0 as f32, y0 as f32, x1 as f32, y1 as f32));
            }
            wires.extend(self.viewport_content_wires(layout_block, None));
        }
        wires
    }

    /// Wires that should participate in hit-testing, snapping, and selection.
    ///
    /// - Model layout: all entity wires (same as entity_wires).
    /// - PSPACE (paper layout, no active viewport): paper-space entities only —
    ///   viewport content is NOT interactive.
    /// - MSPACE (active viewport set): model-space content of the active viewport
    ///   only — paper-space entities are NOT interactive.
    pub fn hit_test_wires(&self) -> Vec<WireModel> {
        if self.current_layout == "Model" {
            return self.entity_wires();
        }
        let layout_block = self.current_layout_block_handle();
        match self.active_viewport {
            None => {
                // PSPACE: only paper-space entities (viewport borders, title blocks…)
                self.wires_for_block(layout_block)
            }
            Some(vp_handle) => {
                // MSPACE: only model content visible through the active viewport
                self.viewport_content_wires(layout_block, Some(vp_handle))
            }
        }
    }

    /// Tessellate all non-invisible entities owned by `block_handle`.
    fn wires_for_block(&self, block_handle: Handle) -> Vec<WireModel> {
        use acadrust::objects::ObjectType;

        // Find the SortEntitiesTable for this block (if any).
        let sort_table = self.document.objects.values().find_map(|obj| {
            if let ObjectType::SortEntitiesTable(t) = obj {
                if t.block_owner_handle == block_handle { Some(t) } else { None }
            } else {
                None
            }
        });

        let (mut native_wires, native_handles) = if self.native_render_active_for_block(block_handle)
        {
            self.native_wires_for_model_space()
        } else {
            (Vec::new(), HashSet::new())
        };

        let mut wires: Vec<WireModel> = self.document
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
                if native_handles.contains(&c.handle.value()) {
                    return false;
                }
                // FRAMES0: hide all Underlay frames/boundaries.
                if self.underlay_frames_mode == 0
                    && matches!(e, acadrust::EntityType::Underlay(_))
                {
                    return false;
                }
                self.belongs_to_visible_block(e.common().handle, c.owner_handle, block_handle)
            })
            .flat_map(|e| {
                let mut tessellated = self.tessellate_one(e);
                // UOSNAP OFF: strip snap points from Underlay tessellation so
                // object snap never targets Underlay geometry (the frame stays
                // visible for rendering, just non-snappable).
                if !self.underlay_snap_enabled
                    && matches!(e, acadrust::EntityType::Underlay(_))
                {
                    for w in &mut tessellated {
                        w.snap_pts.clear();
                    }
                }
                tessellated
            })
            .collect();

        native_wires.append(&mut wires);
        let mut wires = native_wires;

        // Apply draw order if a SortEntitiesTable exists and has entries.
        if let Some(table) = sort_table {
            if !table.is_empty() {
                wires.sort_by_key(|w| {
                    let handle = Self::handle_from_wire_name(&w.name)
                        .unwrap_or(Handle::NULL);
                    table.get_sort_handle(handle)
                        .map(|sh| sh.value())
                        .unwrap_or(u64::MAX / 2) // unsorted entities draw in the middle
                });
            }
        }
        wires
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

    fn native_render_active_for_block(&self, block_handle: Handle) -> bool {
        self.native_render_enabled
            && self.current_layout == "Model"
            && self.native_store.is_some()
            && block_handle == self.model_space_block_handle()
    }

    fn native_render_supported_entity(entity: &nm::Entity) -> bool {
        matches!(
            entity.data,
            nm::EntityData::Point { .. }
                | nm::EntityData::Line { .. }
                | nm::EntityData::Circle { .. }
                | nm::EntityData::Arc { .. }
                | nm::EntityData::LwPolyline { .. }
                | nm::EntityData::Text { .. }
                | nm::EntityData::MText { .. }
                | nm::EntityData::Insert { .. }
                | nm::EntityData::Dimension { .. }
                | nm::EntityData::MultiLeader { .. }
        )
    }

    fn native_entity_visible(document: &nm::CadDocument, entity: &nm::Entity) -> bool {
        !entity.invisible
            && !document
                .layers
                .get(&entity.layer_name)
                .map(|layer| !layer.is_on() || layer.is_frozen)
                .unwrap_or(false)
    }

    fn native_wires_for_model_space(&self) -> (Vec<WireModel>, HashSet<u64>) {
        let Some(document) = self.native_doc() else {
            return (Vec::new(), HashSet::new());
        };

        let selected_handles: HashSet<u64> = self.selected.iter().map(|h| h.value()).collect();
        let mut native_handles = HashSet::new();
        let mut wires = Vec::new();
        for entity in document.model_space_entities() {
            if !Self::native_entity_visible(document, entity)
                || !Self::native_render_supported_entity(entity)
            {
                continue;
            }

            let mut visited_blocks = HashSet::new();
            let Some(entity_wires) = self.native_render_entity_wires(
                document,
                entity,
                entity.handle,
                selected_handles.contains(&entity.handle.value()),
                &mut visited_blocks,
            ) else {
                continue;
            };
            native_handles.insert(entity.handle.value());
            wires.extend(entity_wires);
        }

        (wires, native_handles)
    }

    fn native_render_entity_wires(
        &self,
        document: &nm::CadDocument,
        entity: &nm::Entity,
        display_handle: nm::Handle,
        selected: bool,
        visited_blocks: &mut HashSet<u64>,
    ) -> Option<Vec<WireModel>> {
        if !Self::native_entity_visible(document, entity) {
            return Some(Vec::new());
        }

        match &entity.data {
            nm::EntityData::Insert { .. } => {
                self.native_insert_wires(document, entity, display_handle, selected, visited_blocks)
            }
            nm::EntityData::Dimension { .. } => {
                let (entity_color, _, _, line_weight_px, _) =
                    render::render_style_native(document, entity);
                tessellate::tessellate_native_dimension(
                    document,
                    display_handle,
                    entity,
                    selected,
                    entity_color,
                    line_weight_px,
                )
            }
            nm::EntityData::MultiLeader { .. } => {
                let (entity_color, _, _, line_weight_px, _) =
                    render::render_style_native(document, entity);
                tessellate::tessellate_native_multileader(
                    document,
                    display_handle,
                    entity,
                    selected,
                    entity_color,
                    line_weight_px,
                )
            }
            _ => {
                let (entity_color, pattern_length, pattern, line_weight_px, aci) =
                    render::render_style_native(document, entity);
                let mut wire = tessellate::tessellate_native(
                    document,
                    display_handle,
                    entity,
                    selected,
                    entity_color,
                    pattern_length,
                    pattern,
                    line_weight_px,
                );
                wire.aci = aci;
                Some(vec![wire])
            }
        }
    }

    fn native_insert_wires(
        &self,
        document: &nm::CadDocument,
        entity: &nm::Entity,
        display_handle: nm::Handle,
        selected: bool,
        visited_blocks: &mut HashSet<u64>,
    ) -> Option<Vec<WireModel>> {
        let nm::EntityData::Insert {
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
            ..
        } = &entity.data else {
            return None;
        };

        if *has_attribs || !attribs.is_empty() {
            return None;
        }

        let block_record = document.resolve_insert_block(entity)?;
        if !visited_blocks.insert(block_record.handle.value()) {
            return None;
        }

        let mut wires = Vec::new();
        for child in &block_record.entities {
            if !Self::native_entity_visible(document, child) {
                continue;
            }
            if matches!(child.data, nm::EntityData::Hatch { .. }) {
                continue;
            }
            if !Self::native_render_supported_entity(child) {
                visited_blocks.remove(&block_record.handle.value());
                return None;
            }

            let Some(child_wires) =
                self.native_render_entity_wires(document, child, display_handle, selected, visited_blocks)
            else {
                visited_blocks.remove(&block_record.handle.value());
                return None;
            };

            for mut wire in child_wires {
                Self::apply_insert_transform_to_wire(
                    &mut wire,
                    block_record.base_point,
                    *insertion,
                    *scale,
                    *rotation,
                );
                wires.push(wire);
            }
        }

        visited_blocks.remove(&block_record.handle.value());
        Some(wires)
    }

    fn native_insert_hatch_models(
        &self,
        document: &nm::CadDocument,
        entity: &nm::Entity,
        selected: bool,
        visited_blocks: &mut HashSet<u64>,
    ) -> Option<Vec<HatchModel>> {
        let nm::EntityData::Insert {
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
            ..
        } = &entity.data else {
            return None;
        };

        if *has_attribs || !attribs.is_empty() {
            return None;
        }

        let block_record = document.resolve_insert_block(entity)?;
        if !visited_blocks.insert(block_record.handle.value()) {
            return None;
        }

        let mut models = Vec::new();
        for child in &block_record.entities {
            if !Self::native_entity_visible(document, child) {
                continue;
            }

            match &child.data {
                nm::EntityData::Hatch { .. } => {
                    let color = render::render_style_native(document, child).0;
                    let mut model = Self::hatch_model_from_native(child, color)?;
                    Self::apply_insert_transform_to_hatch_model(
                        &mut model,
                        block_record.base_point,
                        *insertion,
                        *scale,
                        *rotation,
                    );
                    if selected {
                        model.color = [0.15, 0.55, 1.00, model.color[3]];
                    }
                    models.push(model);
                }
                nm::EntityData::Insert { .. } => {
                    let nested =
                        self.native_insert_hatch_models(document, child, selected, visited_blocks)?;
                    for mut model in nested {
                        Self::apply_insert_transform_to_hatch_model(
                            &mut model,
                            block_record.base_point,
                            *insertion,
                            *scale,
                            *rotation,
                        );
                        models.push(model);
                    }
                }
                _ if Self::native_render_supported_entity(child) => {}
                _ => {
                    visited_blocks.remove(&block_record.handle.value());
                    return None;
                }
            }
        }

        visited_blocks.remove(&block_record.handle.value());
        Some(models)
    }

    fn apply_insert_transform_to_wire(
        wire: &mut WireModel,
        base_point: [f64; 3],
        insertion: [f64; 3],
        scale: [f64; 3],
        rotation_deg: f64,
    ) {
        let transform = |point: [f32; 3]| -> [f32; 3] {
            if point[0].is_nan() || point[1].is_nan() || point[2].is_nan() {
                return point;
            }

            let local_x = (point[0] - base_point[0] as f32) * scale[0] as f32;
            let local_y = (point[1] - base_point[1] as f32) * scale[1] as f32;
            let local_z = (point[2] - base_point[2] as f32) * scale[2] as f32;
            let rotation = rotation_deg.to_radians() as f32;
            let (sin_r, cos_r) = rotation.sin_cos();

            [
                insertion[0] as f32 + local_x * cos_r - local_y * sin_r,
                insertion[1] as f32 + local_x * sin_r + local_y * cos_r,
                insertion[2] as f32 + local_z,
            ]
        };

        for point in &mut wire.points {
            *point = transform(*point);
        }
        for (snap_point, _) in &mut wire.snap_pts {
            let transformed = transform([snap_point.x, snap_point.y, snap_point.z]);
            *snap_point = glam::Vec3::new(transformed[0], transformed[1], transformed[2]);
        }
        for vertex in &mut wire.key_vertices {
            *vertex = transform(*vertex);
        }

        let avg_xy_scale = ((scale[0].abs() + scale[1].abs()) as f32 * 0.5).max(0.0001);
        wire.pattern_length *= avg_xy_scale;
        for part in &mut wire.pattern {
            *part *= avg_xy_scale;
        }

        let uniform_xy = (scale[0] - scale[1]).abs() < 1e-6;
        wire.tangent_geoms = wire
            .tangent_geoms
            .iter()
            .filter_map(|geom| match geom {
                TangentGeom::Line { p1, p2 } => Some(TangentGeom::Line {
                    p1: transform(*p1),
                    p2: transform(*p2),
                }),
                TangentGeom::Circle { center, radius } if uniform_xy => Some(TangentGeom::Circle {
                    center: transform(*center),
                    radius: *radius * scale[0].abs() as f32,
                }),
                TangentGeom::Circle { .. } => None,
            })
            .collect();
    }

    fn apply_insert_transform_to_hatch_model(
        model: &mut HatchModel,
        base_point: [f64; 3],
        insertion: [f64; 3],
        scale: [f64; 3],
        rotation_deg: f64,
    ) {
        let transform = |point: [f32; 2]| -> [f32; 2] {
            let local_x = (point[0] - base_point[0] as f32) * scale[0] as f32;
            let local_y = (point[1] - base_point[1] as f32) * scale[1] as f32;
            let rotation = rotation_deg.to_radians() as f32;
            let (sin_r, cos_r) = rotation.sin_cos();
            [
                insertion[0] as f32 + local_x * cos_r - local_y * sin_r,
                insertion[1] as f32 + local_x * sin_r + local_y * cos_r,
            ]
        };

        for point in &mut model.boundary {
            *point = transform(*point);
        }

        let avg_xy_scale = ((scale[0].abs() + scale[1].abs()) as f32 * 0.5).max(0.0001);
        match &mut model.pattern {
            hatch_model::HatchPattern::Solid => {}
            hatch_model::HatchPattern::Pattern(_) => {
                model.angle_offset += rotation_deg.to_radians() as f32;
                model.scale *= avg_xy_scale;
            }
            hatch_model::HatchPattern::Gradient { angle_deg, .. } => {
                *angle_deg += rotation_deg as f32;
                model.scale *= avg_xy_scale;
            }
        }
    }

    fn native_model_hatch_entries(&self, native_doc: &nm::CadDocument) -> Vec<(Handle, HatchModel)> {
        let mut models = Vec::new();
        for entity in native_doc.model_space_entities() {
            match &entity.data {
                nm::EntityData::Hatch { .. } => {
                    let handle = Handle::new(entity.handle.value());
                    if !Self::native_entity_visible(native_doc, entity) {
                        continue;
                    }
                    let color = render::render_style_native(native_doc, entity).0;
                    if let Some(mut model) = Self::hatch_model_from_native(entity, color) {
                        if self.selected.contains(&handle) {
                            model.color = [0.15, 0.55, 1.00, model.color[3]];
                        }
                        models.push((handle, model));
                    }
                }
                nm::EntityData::Insert { .. } => {
                    let handle = Handle::new(entity.handle.value());
                    if !Self::native_entity_visible(native_doc, entity) {
                        continue;
                    }
                    if let Some(insert_models) = self.native_insert_hatch_models(
                        native_doc,
                        entity,
                        self.selected.contains(&handle),
                        &mut HashSet::new(),
                    ) {
                        models.extend(insert_models.into_iter().map(|model| (handle, model)));
                    }
                }
                _ => {}
            }
        }
        models
    }

    fn projected_native_hatch_entries_for_paper_space(
        &self,
        paper_block: Handle,
        only_vp: Option<Handle>,
    ) -> Vec<(Handle, HatchModel)> {
        use acadrust::entities::Viewport;

        let Some(native_doc) = self.native_doc() else {
            return vec![];
        };

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
            .filter(|vp| {
                vp.id > 1
                    && vp.common.owner_handle == paper_block
                    && vp.status.is_on
                    && only_vp.map_or(true, |h| vp.common.handle == h)
            })
            .collect();

        if viewports.is_empty() {
            return vec![];
        }

        let model_hatches = self.native_model_hatch_entries(native_doc);
        let mut result = Vec::new();

        for vp in viewports {
            let vd = glam::Vec3::new(
                vp.view_direction.x as f32,
                vp.view_direction.y as f32,
                vp.view_direction.z as f32,
            )
            .normalize_or(glam::Vec3::Z);
            let world_z = glam::Vec3::Z;
            let view_right = if (vd.dot(world_z)).abs() > 0.99 {
                glam::Vec3::X
            } else {
                world_z.cross(vd).normalize()
            };
            let view_up = vd.cross(view_right).normalize();
            let scale = if vp.custom_scale.abs() > 1e-9 {
                vp.custom_scale as f32
            } else if vp.view_height.abs() > 1e-9 {
                (vp.height / vp.view_height) as f32
            } else {
                1.0
            };
            let target = glam::Vec3::new(
                vp.view_target.x as f32,
                vp.view_target.y as f32,
                vp.view_target.z as f32,
            );
            let pcx = vp.center.x as f32;
            let pcy = vp.center.y as f32;
            let hw = (vp.width / 2.0) as f32;
            let hh = (vp.height / 2.0) as f32;
            let vp_x0 = pcx - hw;
            let vp_x1 = pcx + hw;
            let vp_y0 = pcy - hh;
            let vp_y1 = pcy + hh;
            let use_perspective = vp.status.perspective && vp.lens_length > 1.0;
            let camera_dist = if use_perspective {
                (vp.view_height as f32 * vp.lens_length as f32 / 24.0).max(0.001)
            } else {
                0.0
            };

            for (handle, hatch) in &model_hatches {
                let projected: Vec<[f32; 2]> = hatch
                    .boundary
                    .iter()
                    .map(|&[mx, my]| {
                        let mp = glam::Vec3::new(mx, my, 0.0) - target;
                        let u = mp.dot(view_right);
                        let v = mp.dot(view_up);
                        if use_perspective {
                            let d_vd = mp.dot(vd);
                            let fwd = camera_dist - d_vd;
                            if fwd <= 0.001 {
                                return [f32::NAN, f32::NAN];
                            }
                            let factor = camera_dist / fwd;
                            [pcx + u * factor * scale, pcy + v * factor * scale]
                        } else {
                            [pcx + u * scale, pcy + v * scale]
                        }
                    })
                    .filter(|p| p[0].is_finite() && p[1].is_finite())
                    .collect();

                if projected.len() < 3 {
                    continue;
                }

                let mut model = hatch.clone();
                model.boundary = clip_polygon_to_rect(&projected, vp_x0, vp_y0, vp_x1, vp_y1);
                if model.boundary.len() < 3 {
                    continue;
                }
                match &mut model.pattern {
                    hatch_model::HatchPattern::Solid => {}
                    hatch_model::HatchPattern::Pattern(_) => {
                        model.scale *= scale.abs().max(0.0001);
                    }
                    hatch_model::HatchPattern::Gradient { .. } => {}
                }
                result.push((*handle, model));
            }
        }

        result
    }

    /// Full tessellation pipeline for one entity.
    fn tessellate_one(&self, e: &EntityType) -> Vec<WireModel> {
        let h = e.common().handle;
        let sel = self.selected.contains(&h);

        if let EntityType::Viewport(vp) = e {
            let is_active = self.active_viewport == Some(h);
            let is_locked = vp.status.locked;
            let color = if sel && vp.id != 1 {
                // Selected viewport — bright white highlight.
                [1.0, 1.0, 1.0, 1.0]
            } else if vp.id == 1 {
                // Overall paper-space viewport — subtle grey.
                [0.40, 0.40, 0.40, 1.0]
            } else if is_active {
                // Active (entered) viewport — bright yellow.
                [1.0, 0.90, 0.20, 1.0]
            } else if is_locked {
                // Locked viewport — orange tint to indicate scale is frozen.
                [0.90, 0.55, 0.10, 1.0]
            } else {
                // Normal user viewport — cyan.
                [0.0, 0.75, 0.75, 1.0]
            };
            // Active viewport gets a dashed border to visually indicate MSPACE.
            let (pattern_length, pattern) = if is_active {
                (1.5_f32, [0.8, -0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0_f32])
            } else {
                (0.0_f32, [0.0f32; 8])
            };
            return vec![tessellate::tessellate(
                &self.document,
                h,
                e,
                sel,
                color,
                pattern_length,
                pattern,
                1.5,
            )];
        }

        let (entity_color, pattern_length, pattern, line_weight_px, aci) = self.render_style(e);
        let lt_scale = e.common().linetype_scale as f32;
        let lt_name = self.resolved_linetype_name(e);

        if let EntityType::Dimension(dim) = e {
            let mut wires = tessellate::tessellate_dimension(
                &self.document,
                h,
                dim,
                sel,
                entity_color,
                line_weight_px,
            );
            for w in &mut wires { w.aci = aci; }
            return wires;
        }

        if let EntityType::Insert(ins) = e {
            return ins
                .explode_from_document(&self.document)
                .iter()
                .cloned()
                .map(crate::modules::home::modify::explode::normalize_insert_entity)
                .flat_map(|sub| {
                    let (sub_color, sub_pattern_length, sub_pattern, sub_line_weight_px, sub_aci) =
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
                    wire.aci = sub_aci;
                    vec![wire]
                })
                .collect();
        }

        let mut base = tessellate::tessellate(
            &self.document,
            h,
            e,
            sel,
            entity_color,
            pattern_length,
            pattern,
            line_weight_px,
        );
        base.aci = aci;

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
        // Primary: Layout object's block_record (DWG reader sets this).
        if let Some(h) = self.document.objects.values().find_map(|obj| {
            if let ObjectType::Layout(l) = obj {
                if l.name == "Model" && !l.block_record.is_null() {
                    Some(l.block_record)
                } else {
                    None
                }
            } else {
                None
            }
        }) {
            return h;
        }
        // Fallback for DXF files: conventional block-record name.
        self.document
            .block_records
            .get("*Model_Space")
            .map(|br| br.handle)
            .unwrap_or(Handle::NULL)
    }

    /// Compute the axis-aligned bounding box of all model-space entities by
    /// collecting their `key_vertices`.  Returns `None` when there are no
    /// vertices (empty drawing).
    pub fn model_space_extents(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let model_block = self.model_space_block_handle();
        if model_block.is_null() {
            return None;
        }
        let mut min = glam::Vec3::splat(f32::INFINITY);
        let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
        let mut any = false;
        for entity in self.document.entities() {
            let c = entity.common();
            if c.owner_handle != model_block || c.invisible {
                continue;
            }
            for wire in self.tessellate_one(entity) {
                for &[x, y, z] in &wire.key_vertices {
                    if x.is_finite() && y.is_finite() && z.is_finite() {
                        min = min.min(glam::Vec3::new(x, y, z));
                        max = max.max(glam::Vec3::new(x, y, z));
                        any = true;
                    }
                }
            }
        }
        if any { Some((min, max)) } else { None }
    }

    /// Set a newly created viewport's `view_target` and `view_height` so that
    /// all model-space content is visible at a reasonable scale.
    pub fn auto_fit_viewport(&mut self, vp_handle: Handle) {
        let extents = self.model_space_extents();
        let (min, max) = match extents {
            Some(e) => e,
            None => return,
        };
        let center = (min + max) * 0.5;
        let content_w = (max.x - min.x).max(1e-3);
        let content_h = (max.y - min.y).max(1e-3);

        let vp = match self.document.get_entity_mut(vp_handle) {
            Some(acadrust::EntityType::Viewport(vp)) => vp,
            _ => return,
        };
        // Set the view target to the model-space centroid (XY plane, z=0).
        vp.view_target.x = center.x as f64;
        vp.view_target.y = center.y as f64;
        vp.view_target.z = 0.0;

        // Choose the scale that fits both dimensions with a small margin.
        let margin = 1.1_f64;
        let scale_w = vp.width / (content_w as f64 * margin);
        let scale_h = vp.height / (content_h as f64 * margin);
        let fit_scale = scale_w.min(scale_h).min(1000.0).max(1e-6);

        vp.custom_scale = fit_scale;
        vp.view_height = vp.height / fit_scale;
    }

    /// Collect model-space wires projected into paper space for all (or one specific)
    /// user viewports.  `only_vp = Some(h)` restricts output to that viewport.
    fn viewport_content_wires(&self, paper_block: Handle, only_vp: Option<Handle>) -> Vec<WireModel> {
        use acadrust::entities::Viewport;
        use std::collections::HashSet as HSet;

        let viewports: Vec<&Viewport> = self
            .document
            .entities()
            .filter_map(|e| {
                if let EntityType::Viewport(vp) = e { Some(vp) } else { None }
            })
            .filter(|vp| {
                vp.id > 1
                    && vp.common.owner_handle == paper_block
                    && vp.status.is_on
                    && only_vp.map_or(true, |h| vp.common.handle == h)
            })
            .collect();

        if viewports.is_empty() {
            return vec![];
        }

        let model_block = self.model_space_block_handle();
        let mut result = Vec::new();

        for vp in viewports {
            // ── Per-viewport frozen layer set ─────────────────────────────
            let frozen: HSet<Handle> = vp.frozen_layers.iter().cloned().collect();

            // ── View direction coordinate frame ───────────────────────────
            let vd = glam::Vec3::new(
                vp.view_direction.x as f32,
                vp.view_direction.y as f32,
                vp.view_direction.z as f32,
            ).normalize_or(glam::Vec3::Z);

            // Compute view_right and view_up from the direction vector.
            let world_z = glam::Vec3::Z;
            let view_right = if (vd.dot(world_z)).abs() > 0.99 {
                // Looking straight up/down: use X as right.
                glam::Vec3::X
            } else {
                world_z.cross(vd).normalize()
            };
            let view_up = vd.cross(view_right).normalize();

            // ── Scale & viewport parameters ───────────────────────────────
            let scale = if vp.custom_scale.abs() > 1e-9 {
                vp.custom_scale as f32
            } else if vp.view_height.abs() > 1e-9 {
                (vp.height / vp.view_height) as f32
            } else {
                1.0
            };

            let target = glam::Vec3::new(
                vp.view_target.x as f32,
                vp.view_target.y as f32,
                vp.view_target.z as f32,
            );
            let pcx = vp.center.x as f32;
            let pcy = vp.center.y as f32;
            let pcz = vp.center.z as f32;
            let hw = (vp.width / 2.0) as f32;
            let hh = (vp.height / 2.0) as f32;

            let frozen_layer_names: HSet<String> = vp
                .frozen_layers
                .iter()
                .filter_map(|&handle| {
                    self.document
                        .layers
                        .iter()
                        .find(|layer| layer.handle == handle)
                        .map(|layer| layer.name.clone())
                })
                .collect();

            // ── Collect model wires with per-vp layer freeze ──────────────
            let model_wires: Vec<WireModel> =
                if self.native_render_enabled && self.native_store.is_some() {
                    let native_doc = self.native_doc().expect("checked");
                    let selected_handles: HSet<u64> =
                        self.selected.iter().map(|h| h.value()).collect();
                    native_doc
                        .model_space_entities()
                        .filter(|entity| {
                            Self::native_entity_visible(native_doc, entity)
                                && !frozen_layer_names.contains(&entity.layer_name)
                        })
                        .flat_map(|entity| {
                            self.native_render_entity_wires(
                                native_doc,
                                entity,
                                entity.handle,
                                selected_handles.contains(&entity.handle.value()),
                                &mut HashSet::new(),
                            )
                            .unwrap_or_default()
                        })
                        .collect()
                } else {
                    self.document
                        .entities()
                        .filter(|e| {
                            let c = e.common();
                            if c.invisible || matches!(e, EntityType::Viewport(_)) {
                                return false;
                            }
                            // Global layer visibility.
                            if self.document.layers.get(&c.layer)
                                .map(|l| l.flags.off || l.flags.frozen)
                                .unwrap_or(false)
                            {
                                return false;
                            }
                            // Per-viewport frozen layers.
                            if !frozen.is_empty() {
                                if let Some(lh) = self.document.layers.get(&c.layer).map(|l| l.handle) {
                                    if frozen.contains(&lh) {
                                        return false;
                                    }
                                }
                            }
                            self.belongs_to_visible_block(c.handle, c.owner_handle, model_block)
                        })
                        .flat_map(|e| self.tessellate_one(e))
                        .collect()
                };

            // ── Project and clip wires into viewport ──────────────────────
            let vp_x0 = pcx - hw;
            let vp_x1 = pcx + hw;
            let vp_y0 = pcy - hh;
            let vp_y1 = pcy + hh;

            // ── Perspective setup ─────────────────────────────────────────
            // camera_dist: how far the camera is from the target plane.
            // Derived from lens_length (mm, 35mm-film equiv.) and view_height:
            //   tan(fov_v/2) = 12 / lens_length  →  camera_dist = view_height * lens_length / 24
            let use_perspective = vp.status.perspective && vp.lens_length > 1.0;
            let camera_dist = if use_perspective {
                (vp.view_height as f32 * vp.lens_length as f32 / 24.0).max(0.001)
            } else {
                0.0
            };

            for wire in &model_wires {
                // Project 3-D model points onto view plane → paper space.
                let projected_pts: Vec<[f32; 3]> = wire.points.iter().map(|&[mx, my, mz]| {
                    if mx.is_nan() || my.is_nan() || mz.is_nan() {
                        return [f32::NAN; 3];
                    }
                    let mp = glam::Vec3::new(mx, my, mz) - target;
                    let u = mp.dot(view_right);
                    let v = mp.dot(view_up);
                    if use_perspective {
                        // vd points from target toward camera, so depth along vd is the
                        // distance from the target plane toward the camera.
                        let d_vd = mp.dot(vd);
                        // Forward distance from camera (positive = in front of camera).
                        let fwd = camera_dist - d_vd;
                        if fwd <= 0.001 {
                            // Point is behind or at the camera — discard.
                            return [f32::NAN; 3];
                        }
                        let factor = camera_dist / fwd;
                        [pcx + u * factor * scale, pcy + v * factor * scale, pcz]
                    } else {
                        [pcx + u * scale, pcy + v * scale, pcz]
                    }
                }).collect();

                // Fast AABB pre-reject: skip entirely if no finite point is
                // anywhere near the viewport.
                let any_near = projected_pts.iter().any(|&[x, y, _]| {
                    x.is_finite() && y.is_finite()
                        && x >= vp_x0 - 1.0 && x <= vp_x1 + 1.0
                        && y >= vp_y0 - 1.0 && y <= vp_y1 + 1.0
                });
                // Also keep wires whose AABB overlaps the viewport (partial overlap).
                let (min_x, max_x, min_y, max_y) = projected_pts.iter()
                    .filter(|p| p[0].is_finite())
                    .fold(
                        (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY),
                        |(mnx, mxx, mny, mxy), &[x, y, _]| {
                            (mnx.min(x), mxx.max(x), mny.min(y), mxy.max(y))
                        },
                    );
                let aabb_hits = max_x >= vp_x0 && min_x <= vp_x1
                             && max_y >= vp_y0 && min_y <= vp_y1;
                if !any_near && !aabb_hits {
                    continue;
                }

                // Cohen-Sutherland clipping: clip every segment to the viewport.
                let clipped = clip_polyline_to_rect(
                    &projected_pts, vp_x0, vp_y0, vp_x1, vp_y1, pcz,
                );
                if clipped.is_empty() {
                    continue;
                }

                let [r, g, b, a] = wire.color;
                let mut out = wire.clone();
                out.points = clipped;
                out.color = [r * 0.80, g * 0.80, b * 0.80, a * 0.85];
                // Line weights are paper-space pen widths — independent of viewport scale.
                out.line_weight_px = wire.line_weight_px;
                result.push(out);
            }
        }

        result
    }

    // ── MSPACE helpers ───────────────────────────────────────────────────

    /// Convert a **paper-space** world coordinate to **model-space** using the
    /// geometry of the currently active viewport.  Returns the input unchanged
    /// when there is no active viewport.
    pub fn paper_to_model(&self, paper_pt: glam::Vec3) -> glam::Vec3 {
        let vp_handle = match self.active_viewport {
            Some(h) => h,
            None => return paper_pt,
        };
        let vp = match self.document.get_entity(vp_handle) {
            Some(acadrust::EntityType::Viewport(vp)) => vp,
            _ => return paper_pt,
        };
        let scale = if vp.custom_scale.abs() > 1e-9 {
            vp.custom_scale as f32
        } else if vp.view_height.abs() > 1e-9 {
            (vp.height / vp.view_height) as f32
        } else {
            1.0
        };
        if scale.abs() < 1e-9 {
            return paper_pt;
        }
        let tx = vp.view_target.x as f32;
        let ty = vp.view_target.y as f32;
        let pcx = vp.center.x as f32;
        let pcy = vp.center.y as f32;
        glam::Vec3::new(
            (paper_pt.x - pcx) / scale + tx,
            (paper_pt.y - pcy) / scale + ty,
            paper_pt.z,
        )
    }

    /// Pan the active viewport's model-space view by `(screen_dx, screen_dy)` pixels.
    /// The delta is converted to model-space units using the camera and viewport scale.
    /// No-op when there is no active viewport.
    pub fn pan_active_viewport(&mut self, screen_dx: f32, screen_dy: f32, bounds: iced::Rectangle) {
        let vp_handle = match self.active_viewport {
            Some(h) => h,
            None => return,
        };
        // Convert screen pixels → paper-space delta using the camera.
        let cam = self.camera.borrow();
        let paper_delta = cam.screen_delta_to_world(screen_dx, screen_dy, bounds);
        drop(cam);

        if let Some(acadrust::EntityType::Viewport(vp)) =
            self.document.get_entity_mut(vp_handle)
        {
            if vp.status.locked { return; }
            let scale = if vp.custom_scale.abs() > 1e-9 {
                vp.custom_scale
            } else if vp.view_height.abs() > 1e-9 {
                vp.height / vp.view_height
            } else {
                1.0
            };
            if scale.abs() < 1e-12 { return; }
            // screen_delta_to_world returns the same delta that cam.pan() ADDS to its
            // target, so we add it here too (dividing by viewport scale to convert from
            // paper-space to model-space).  Using -= would invert the drag direction.
            vp.view_target.x += (paper_delta.x / scale as f32) as f64;
            vp.view_target.y += (paper_delta.y / scale as f32) as f64;
        }
    }

    /// Zoom the active viewport's model-space view by `steps` notches.
    /// Positive = zoom in (increase detail), negative = zoom out.
    /// `cursor_paper`: optional paper-space XY of the cursor; when supplied the
    /// model point under the cursor is kept stationary (AutoCAD-style zoom).
    /// No-op when there is no active viewport.
    pub fn zoom_active_viewport(&mut self, steps: f32, cursor_paper: Option<glam::Vec2>) {
        let vp_handle = match self.active_viewport {
            Some(h) => h,
            None => return,
        };
        if let Some(acadrust::EntityType::Viewport(vp)) =
            self.document.get_entity_mut(vp_handle)
        {
            if vp.status.locked { return; }
            // Zoom in = shrink view_height → higher scale → objects appear larger.
            let factor = (1.0_f64 - 0.15 * steps as f64).clamp(0.1, 10.0);

            if let Some(cp) = cursor_paper {
                // Compute the model-space point under the cursor before zoom.
                let scale_before = if vp.custom_scale.abs() > 1e-9 {
                    vp.custom_scale as f32
                } else if vp.view_height.abs() > 1e-9 {
                    (vp.height / vp.view_height) as f32
                } else {
                    1.0
                };
                let cx = vp.center.x as f32;
                let cy = vp.center.y as f32;
                let tx = vp.view_target.x as f32;
                let ty = vp.view_target.y as f32;
                let mx = (cp.x - cx) / scale_before + tx;
                let my = (cp.y - cy) / scale_before + ty;

                // Apply zoom.
                vp.view_height = (vp.view_height * factor).max(1e-6);
                if vp.view_height.abs() > 1e-9 {
                    vp.custom_scale = vp.height / vp.view_height;
                }
                let scale_after = vp.custom_scale as f32;

                // Adjust view_target so the model point under cursor stays there.
                let mx_after = (cp.x - cx) / scale_after + vp.view_target.x as f32;
                let my_after = (cp.y - cy) / scale_after + vp.view_target.y as f32;
                vp.view_target.x += (mx - mx_after) as f64;
                vp.view_target.y += (my - my_after) as f64;
            } else {
                vp.view_height = (vp.view_height * factor).max(1e-6);
                if vp.view_height.abs() > 1e-9 {
                    vp.custom_scale = vp.height / vp.view_height;
                }
            }
        }
    }

    /// Return the handle of the user viewport whose bounding rectangle contains
    /// the given paper-space point, or `None` if no viewport matches.
    pub fn viewport_at_paper_point(&self, px: f32, py: f32) -> Option<Handle> {
        let layout_block = self.current_layout_block_handle();
        self.document
            .entities()
            .find_map(|e| {
                let EntityType::Viewport(vp) = e else { return None; };
                if vp.id <= 1 || vp.common.owner_handle != layout_block || !vp.status.is_on {
                    return None;
                }
                let hw = (vp.width / 2.0) as f32;
                let hh = (vp.height / 2.0) as f32;
                let cx = vp.center.x as f32;
                let cy = vp.center.y as f32;
                if px >= cx - hw && px <= cx + hw && py >= cy - hh && py <= cy + hh {
                    Some(vp.common.handle)
                } else {
                    None
                }
            })
    }

    /// Return the handle of the first active user viewport in the current layout,
    /// or `None` if there are none.  Used by the MS command.
    pub fn first_user_viewport(&self) -> Option<Handle> {
        let layout_block = self.current_layout_block_handle();
        self.document.entities().find_map(|e| {
            let EntityType::Viewport(vp) = e else { return None; };
            if vp.id > 1 && vp.common.owner_handle == layout_block && vp.status.is_on {
                Some(vp.common.handle)
            } else {
                None
            }
        })
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

    /// Swap the `tab_order` of two paper layouts so they appear in swapped order.
    pub fn swap_layout_order(&mut self, name_a: &str, name_b: &str) {
        let mut order_a: Option<i16> = None;
        let mut order_b: Option<i16> = None;
        for obj in self.document.objects.values() {
            if let ObjectType::Layout(l) = obj {
                if l.name == name_a { order_a = Some(l.tab_order); }
                if l.name == name_b { order_b = Some(l.tab_order); }
            }
        }
        if let (Some(oa), Some(ob)) = (order_a, order_b) {
            for obj in self.document.objects.values_mut() {
                if let ObjectType::Layout(l) = obj {
                    if l.name == name_a { l.tab_order = ob; }
                    else if l.name == name_b { l.tab_order = oa; }
                }
            }
        }
    }

    // ── Entity management ─────────────────────────────────────────────────

    pub fn add_entity(&mut self, mut entity: EntityType) -> Handle {
        let hatch_seed = if let EntityType::Hatch(dxf) = &entity {
            let color = self.render_style(&entity).0;
            Self::hatch_model_from_dxf(dxf, color)
        } else if let EntityType::Solid(solid) = &entity {
            let color = self.render_style(&entity).0;
            Some(Self::solid_hatch_model(solid, color))
        } else {
            None
        };
        let image_seed = if let EntityType::RasterImage(img) = &entity {
            ImageModel::from_raster_image(img)
        } else {
            None
        };
        let mesh_seed = match &entity {
            EntityType::Solid3D(s3d) => {
                let color = self.render_style(&entity).0;
                solid3d_tess::tessellate_solid3d(s3d, color)
            }
            EntityType::Region(r) => {
                let color = self.render_style(&entity).0;
                solid3d_tess::tessellate_region(r, color)
            }
            EntityType::Body(b) => {
                let color = self.render_style(&entity).0;
                solid3d_tess::tessellate_body(b, color)
            }
            _ => None,
        };

        // Auto-create an ImageDefinition object for new RasterImage entities
        // that don't already reference one.
        if let EntityType::RasterImage(ref mut img) = entity {
            if img.definition_handle.is_none() {
                use acadrust::objects::{ImageDefinition, ObjectType};
                let def_handle = Handle::new(self.document.next_handle());
                let mut img_def = ImageDefinition::with_dimensions(
                    &img.file_path,
                    img.size.x as u32,
                    img.size.y as u32,
                );
                img_def.handle = def_handle;
                img_def.is_loaded = true;
                self.document
                    .objects
                    .insert(def_handle, ObjectType::ImageDefinition(img_def));
                img.definition_handle = Some(def_handle);
            }
        }

        // Route to the correct block based on current editing mode:
        //   - PSPACE (paper layout, no active viewport): paper-space layout block.
        //   - MSPACE or model layout: model space (document default).
        let handle = if self.current_layout != "Model" && self.active_viewport.is_none() {
            let layout_name = self.current_layout.clone();
            self.document
                .add_entity_to_layout(entity, &layout_name)
                .unwrap_or(Handle::NULL)
        } else {
            self.document.add_entity(entity).unwrap_or(Handle::NULL)
        };

        if !handle.is_null() {
            if let Some(store) = self.native_store.as_mut() {
                let native_doc = store.inner_mut();
                if let Some(entity) = self.document.get_entity(handle).cloned() {
                    if let Some(mut native_entity) = crate::io::native_bridge::acadrust_entity_to_native(&entity) {
                        let owner_handle = if self.current_layout != "Model" && self.active_viewport.is_none() {
                            native_doc
                                .layout_by_name(&self.current_layout)
                                .map(|layout| layout.block_record_handle)
                                .unwrap_or_else(|| native_doc.model_space_handle())
                        } else {
                            native_doc.model_space_handle()
                        };
                        native_entity.owner_handle = owner_handle;
                        let _ = native_doc.add_entity(native_entity);
                    }
                }
            }
            if let Some(model) = hatch_seed {
                self.hatches.insert(handle, model);
            }
            if let Some(model) = image_seed {
                self.images.insert(handle, model);
            }
            if let Some(model) = mesh_seed {
                self.meshes.insert(handle, model);
            }
        }
        handle
    }

    /// Returns the RGBA color for the given layer name.
    pub fn layer_color(&self, layer: &str) -> [f32; 4] {
        let layer_entry = self.document.layers.get(layer);
        let color = layer_entry.map(|l| &l.color).unwrap_or(&crate::types::Color::WHITE);
        let [r, g, b, _] = crate::scene::tessellate::aci_to_rgba(color);
        [r, g, b, 1.0]
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
            crate::types::Vector3::ZERO,
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
            crate::types::Vector3::new(base.x as f64, base.y as f64, base.z as f64),
        );
        Ok(self.add_entity(EntityType::Insert(insert)))
    }

    pub(crate) fn synced_hatch_entries(&self) -> Vec<(Handle, HatchModel)> {
        let mut models: Vec<(Handle, HatchModel)> = self
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
                (handle, m)
            })
            .collect();

        let mut native_insert_hatch_handles = HashSet::new();

        if self.native_render_enabled {
            if self.current_layout == "Model" {
                if let Some(native_doc) = self.native_doc() {
                    for entity in native_doc.model_space_entities() {
                        match &entity.data {
                            nm::EntityData::Hatch { .. } => {
                                let handle = Handle::new(entity.handle.value());
                                if self.hatches.contains_key(&handle)
                                    || !Self::native_entity_visible(native_doc, entity)
                                {
                                    continue;
                                }
                                let color = render::render_style_native(native_doc, entity).0;
                                if let Some(mut model) = Self::hatch_model_from_native(entity, color) {
                                    if self.selected.contains(&handle) {
                                        model.color = [0.15, 0.55, 1.00, model.color[3]];
                                    }
                                    models.push((handle, model));
                                }
                            }
                            nm::EntityData::Insert { .. } => {
                                let handle = Handle::new(entity.handle.value());
                                let Some(insert_models) = self.native_insert_hatch_models(
                                    native_doc,
                                    entity,
                                    self.selected.contains(&handle),
                                    &mut HashSet::new(),
                                ) else {
                                    continue;
                                };
                                if !insert_models.is_empty() {
                                    native_insert_hatch_handles.insert(handle);
                                    models.extend(insert_models.into_iter().map(|model| (handle, model)));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                let projected = self.projected_native_hatch_entries_for_paper_space(
                    self.current_layout_block_handle(),
                    self.active_viewport,
                );
                models.extend(projected);
            }
        }

        for entity in self.document.entities() {
            let EntityType::Insert(ins) = entity else {
                continue;
            };
            if native_insert_hatch_handles.contains(&ins.common.handle) {
                continue;
            }
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
                    models.push((ins.common.handle, model));
                }
            }
        }

        models
    }

    fn synced_hatch_models(&self) -> Vec<HatchModel> {
        self.synced_hatch_entries()
            .into_iter()
            .map(|(_, model)| model)
            .collect()
    }

    pub(crate) fn hatch_model_for_handle(&self, handle: Handle) -> Option<HatchModel> {
        if let Some(model) = self.hatches.get(&handle) {
            return Some(model.clone());
        }
        self.synced_hatch_entries()
            .into_iter()
            .find_map(|(entry_handle, model)| (entry_handle == handle).then_some(model))
    }

    /// Wipeout fill models — rendered in a separate pass AFTER wires so that
    /// wipeouts correctly mask everything below them in the draw order.
    pub(super) fn wipeout_models(&self) -> Vec<HatchModel> {
        let bg_color: [f32; 4] = if self.current_layout == "Model" {
            self.bg_color
        } else {
            self.paper_bg_color
        };
        let mut models = Vec::new();
        for entity in self.document.entities() {
            let EntityType::Wipeout(wo) = entity else { continue };
            if entity.common().invisible {
                continue;
            }
            if self.document.layers
                .get(&entity.common().layer)
                .map(|l| l.flags.off || l.flags.frozen)
                .unwrap_or(false)
            {
                continue;
            }
            let boundary = Self::wipeout_boundary_2d(wo);
            if boundary.len() >= 3 {
                let mut fill_color = bg_color;
                if self.selected.contains(&wo.common.handle) {
                    fill_color = [0.15, 0.55, 1.00, 0.35];
                }
                models.push(HatchModel {
                    boundary,
                    pattern: hatch_model::HatchPattern::Solid,
                    name: "WIPEOUT_FILL".into(),
                    color: fill_color,
                    angle_offset: 0.0,
                    scale: 1.0,
                });
            }
        }
        models
    }

    /// Compute the 2D (XY) boundary polygon for a Wipeout entity.
    fn wipeout_boundary_2d(wo: &acadrust::entities::Wipeout) -> Vec<[f32; 2]> {
        use acadrust::entities::WipeoutClipType;

        let is_polygon = wo.clipping_enabled
            && wo.clip_boundary_vertices.len() >= 3
            && matches!(wo.clip_type, WipeoutClipType::Polygonal);

        if is_polygon {
            let ox = wo.insertion_point.x as f32;
            let oy = wo.insertion_point.y as f32;
            wo.clip_boundary_vertices.iter().map(|v| {
                let wx = (wo.u_vector.x * v.x * wo.size.x + wo.v_vector.x * v.y * wo.size.y) as f32;
                let wy = (wo.u_vector.y * v.x * wo.size.x + wo.v_vector.y * v.y * wo.size.y) as f32;
                [ox + wx, oy + wy]
            }).collect()
        } else {
            // Rectangular boundary from 4 corners.
            let ox = wo.insertion_point.x as f32;
            let oy = wo.insertion_point.y as f32;
            let oz = wo.insertion_point.z as f32;
            let ux = (wo.u_vector.x * wo.size.x) as f32;
            let uy = (wo.u_vector.y * wo.size.x) as f32;
            let vx = (wo.v_vector.x * wo.size.y) as f32;
            let vy = (wo.v_vector.y * wo.size.y) as f32;
            let _ = oz;
            vec![
                [ox, oy],
                [ox + ux, oy + uy],
                [ox + ux + vx, oy + uy + vy],
                [ox + vx, oy + vy],
            ]
        }
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

    fn hatch_model_from_native(hatch: &nm::Entity, color: [f32; 4]) -> Option<HatchModel> {
        let nm::EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } = &hatch.data else {
            return None;
        };

        let path = boundary_paths.first()?;
        let mut boundary: Vec<[f32; 2]> = Vec::new();

        for edge in &path.edges {
            match edge {
                nm::HatchEdge::Polyline { closed, vertices } => {
                    let count = vertices.len();
                    let seg_count = if *closed {
                        count
                    } else {
                        count.saturating_sub(1)
                    };
                    for i in 0..seg_count {
                        let v0 = vertices[i];
                        let v1 = vertices[(i + 1) % count];
                        let bulge = v0[2] as f32;
                        if bulge.abs() < 1e-9 {
                            boundary.push([v0[0] as f32, v0[1] as f32]);
                        } else {
                            let p0 = [v0[0] as f32, v0[1] as f32];
                            let p1 = [v1[0] as f32, v1[1] as f32];
                            let angle = 4.0 * bulge.atan();
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
                    if *closed {
                        if let Some(&first) = boundary.first() {
                            boundary.push(first);
                        }
                    }
                }
                nm::HatchEdge::Line { start, end } => {
                    boundary.push([start[0] as f32, start[1] as f32]);
                    boundary.push([end[0] as f32, end[1] as f32]);
                }
                nm::HatchEdge::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    is_ccw,
                } => {
                    let cx = center[0] as f32;
                    let cy = center[1] as f32;
                    let r = *radius as f32;
                    let (sa, ea) = if *is_ccw {
                        (*start_angle as f32, *end_angle as f32)
                    } else {
                        (*end_angle as f32, *start_angle as f32)
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
                nm::HatchEdge::EllipticArc {
                    center,
                    major_endpoint,
                    minor_ratio,
                    start_angle,
                    end_angle,
                    is_ccw,
                } => {
                    let cx = center[0] as f32;
                    let cy = center[1] as f32;
                    let maj_x = major_endpoint[0] as f32;
                    let maj_y = major_endpoint[1] as f32;
                    let r_maj = (maj_x * maj_x + maj_y * maj_y).sqrt();
                    let r_min = r_maj * *minor_ratio as f32;
                    let rot = maj_y.atan2(maj_x);
                    let (sa, ea) = if *is_ccw {
                        (*start_angle as f32, *end_angle as f32)
                    } else {
                        (*end_angle as f32, *start_angle as f32)
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
            }
        }

        if boundary.is_empty() {
            return None;
        }
        boundary.truncate(64);

        let pattern = if *solid_fill {
            hatch_model::HatchPattern::Solid
        } else if let Some(entry) = crate::scene::hatch_patterns::find(pattern_name) {
            entry.gpu.clone()
        } else {
            hatch_model::HatchPattern::Pattern(vec![hatch_model::PatFamily {
                angle_deg: 0.0,
                x0: 0.0,
                y0: 0.0,
                dx: 0.0,
                dy: 5.0,
                dashes: vec![],
            }])
        };

        Some(HatchModel {
            boundary,
            pattern,
            name: if *solid_fill {
                "SOLID".into()
            } else {
                pattern_name.clone()
            },
            color,
            angle_offset: 0.0,
            scale: 1.0,
        })
    }

    /// Decode and cache all RasterImage entities from the current document.
    /// Silently skips images whose files cannot be read.
    pub fn populate_images_from_document(&mut self) {
        self.images.clear();
        let entries: Vec<(Handle, acadrust::entities::RasterImage)> = self
            .document
            .entities()
            .filter_map(|e| {
                if let EntityType::RasterImage(img) = e {
                    Some((img.common.handle, img.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (handle, img) in entries {
            if let Some(model) = ImageModel::from_raster_image(&img) {
                self.images.insert(handle, model);
            }
        }
    }

    pub fn populate_hatches_from_document(&mut self) {
        self.hatches.clear();

        let entries: Vec<(Handle, EntityType)> = self
            .document
            .entities()
            .filter_map(|e| match e {
                EntityType::Hatch(h) => Some((h.common.handle, e.clone())),
                EntityType::Solid(s) => Some((s.common.handle, e.clone())),
                _ => None,
            })
            .collect();

        for (handle, kind) in entries {
            let model = match &kind {
                EntityType::Hatch(dxf) => {
                    let color = tessellate::aci_to_rgba(&dxf.common.color);
                    Self::hatch_model_from_dxf(dxf, color)
                }
                EntityType::Solid(solid) => {
                    let color = tessellate::aci_to_rgba(&solid.common.color);
                    Some(Self::solid_hatch_model(solid, color))
                }
                _ => None,
            };
            if let Some(m) = model {
                self.hatches.insert(handle, m);
            }
        }
    }

    /// Tessellate all `Solid3D` entities in the current document into
    /// GPU-ready `MeshModel`s and store them in `self.meshes`.
    ///
    /// Called after loading a document or after undo/redo so that every
    /// `Solid3D` entity is represented in the mesh cache.
    pub fn populate_meshes_from_document(&mut self) {
        self.meshes.clear();
        // Collect all ACIS-bearing entities: Solid3D, Region, Body.
        let entries: Vec<(Handle, EntityType)> = self
            .document
            .entities()
            .filter_map(|e| match e {
                EntityType::Solid3D(_) | EntityType::Region(_) | EntityType::Body(_) =>
                    Some((e.common().handle, e.clone())),
                _ => None,
            })
            .collect();
        for (handle, entity) in entries {
            let color = if let Some(e) = self.document.get_entity(handle) {
                tessellate::aci_to_rgba(&e.common().color)
            } else {
                [0.7, 0.7, 0.7, 1.0]
            };
            let model = match &entity {
                EntityType::Solid3D(s) => solid3d_tess::tessellate_solid3d(s, color),
                EntityType::Region(r)  => solid3d_tess::tessellate_region(r, color),
                EntityType::Body(b)    => solid3d_tess::tessellate_body(b, color),
                _ => None,
            };
            if let Some(m) = model {
                self.meshes.insert(handle, m);
            }
        }
    }

    /// Rebuild hatch / image / mesh caches after the document is modified
    /// outside the normal `add_entity` path (e.g. REFCLOSE SAVE).
    pub fn rebuild_derived_caches(&mut self) {
        self.populate_hatches_from_document();
        self.populate_images_from_document();
        self.populate_meshes_from_document();
    }

    /// Build a solid-fill HatchModel for a DXF Solid entity.
    /// DXF SOLID corners are in "Z-order": p0-p1 top, p2-p3 bottom.
    /// Visual quad is p0→p1→p3→p2 (closed).
    fn solid_hatch_model(solid: &DxfSolid, color: [f32; 4]) -> HatchModel {
        let boundary = vec![
            [solid.first_corner.x as f32,  solid.first_corner.y as f32],
            [solid.second_corner.x as f32, solid.second_corner.y as f32],
            [solid.fourth_corner.x as f32, solid.fourth_corner.y as f32],
            [solid.third_corner.x as f32,  solid.third_corner.y as f32],
        ];
        HatchModel {
            boundary,
            pattern: hatch_model::HatchPattern::Solid,
            name: "SOLID".into(),
            color,
            angle_offset: 0.0,
            scale: 1.0,
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

    pub fn native_entity(&self, handle: Handle) -> Option<&nm::Entity> {
        self.native_doc()
            .and_then(|doc| doc.get_entity(nm::Handle::new(handle.value())))
    }

    pub fn native_entity_mut(&mut self, handle: Handle) -> Option<&mut nm::Entity> {
        self.native_doc_mut()
            .and_then(|doc| doc.get_entity_mut(nm::Handle::new(handle.value())))
    }

    // ── Erase ─────────────────────────────────────────────────────────────

    pub fn erase_entities(&mut self, handles: &[Handle]) {
        for &h in handles {
            self.document.remove_entity(h);
            if let Some(native_doc) = self.native_doc_mut() {
                let _ = native_doc.remove_entity(nm::Handle::new(h.value()));
            }
            self.selected.remove(&h);
            self.hatches.remove(&h);
            self.meshes.remove(&h);
            self.images.remove(&h);
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
            let nh = nm::Handle::new(h.value());
            let native_applied = if let Some(store) = self.native_store.as_mut() {
                if let Some(entity) = store.inner_mut().get_entity_mut(nh) {
                    dispatch::apply_transform_native(entity, t);
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if native_applied {
                if let Some(native_entity) = self.native_doc()
                    .and_then(|doc| doc.get_entity(nh))
                {
                    if let Some(compat) = crate::io::native_bridge::native_entity_to_acadrust(native_entity) {
                        if let Some(existing) = self.document.get_entity_mut(h) {
                            *existing = compat;
                        }
                    }
                }
            } else if let Some(entity) = self.document.get_entity_mut(h) {
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

    /// Rebuild GPU hatch/solid model after a grip edit changed geometry.
    pub fn rebuild_gpu_model_after_grip(&mut self, handle: Handle) {
        match self.document.get_entity(handle) {
            Some(EntityType::Hatch(dxf)) => {
                let color = tessellate::aci_to_rgba(&dxf.common.color);
                if let Some(model) = Self::hatch_model_from_dxf(dxf, color) {
                    self.hatches.insert(handle, model);
                } else {
                    self.hatches.remove(&handle);
                }
            }
            Some(EntityType::Solid(solid)) => {
                let color = tessellate::aci_to_rgba(&solid.common.color);
                self.hatches.insert(handle, Self::solid_hatch_model(solid, color));
            }
            _ => {}
        }
    }

    // ── Hit-test convenience: wire name → Handle ──────────────────────────

    pub fn handle_from_wire_name(name: &str) -> Option<Handle> {
        name.parse::<u64>().ok().map(Handle::new)
    }

    /// Restore camera to a named view from the document view table.
    pub fn restore_named_view(&mut self, view: &acadrust::tables::View) {
        use glam::Vec3;
        let cam = &mut *self.camera.borrow_mut();
        // view.target is the look-at point; view.direction is eye→target direction.
        cam.target = Vec3::new(view.target.x as f32, view.target.y as f32, view.target.z as f32);
        // direction in acadrust = from-target-to-eye (same as AutoCAD convention).
        let eye_dir = Vec3::new(
            view.direction.x as f32,
            view.direction.y as f32,
            view.direction.z as f32,
        );
        let eye_dir = if eye_dir.length_squared() > 1e-10 {
            eye_dir.normalize()
        } else {
            Vec3::Z
        };
        // Build rotation: canonical eye is +Z, rotate to eye_dir.
        cam.rotation = glam::Quat::from_rotation_arc(Vec3::Z, eye_dir);
        // Sync yaw/pitch from new rotation (for ViewCube).
        let pitch = eye_dir.z.clamp(-0.999, 0.999).asin();
        let yaw = eye_dir.x.atan2(eye_dir.y);
        cam.yaw = yaw;
        cam.pitch = pitch;
        // Derive distance from view height and fov.
        let h = view.height as f32;
        cam.distance = if h > 0.0 {
            h / (2.0 * (cam.fov_y * 0.5).tan())
        } else {
            cam.distance
        };
        self.camera_generation += 1;
    }

    /// Save the current camera state into a new named view entry.
    /// Returns the view; caller must push it into document.views.
    pub fn current_as_named_view(&self, name: &str) -> acadrust::tables::View {
        use crate::types::Vector3;
        let cam = self.camera.borrow();
        let eye_dir = cam.rotation * glam::Vec3::Z;
        let height = cam.ortho_size() * 2.0;
        let width = height; // caller can adjust; rough square
        acadrust::tables::View {
            handle: crate::types::Handle::NULL,
            name: name.to_string(),
            center: Vector3 {
                x: cam.target.x as f64,
                y: cam.target.y as f64,
                z: 0.0,
            },
            target: Vector3 {
                x: cam.target.x as f64,
                y: cam.target.y as f64,
                z: cam.target.z as f64,
            },
            direction: Vector3 {
                x: eye_dir.x as f64,
                y: eye_dir.y as f64,
                z: eye_dir.z as f64,
            },
            height: height as f64,
            width: width as f64,
            lens_length: 50.0,
            front_clip: 0.0,
            back_clip: 0.0,
            twist_angle: 0.0,
        }
    }

    /// Zoom the model-space camera in/out by a percentage.
    /// factor > 1 = zoom out, factor < 1 = zoom in.
    pub fn zoom_camera(&mut self, factor: f32) {
        let mut cam = self.camera.borrow_mut();
        cam.distance = (cam.distance * factor).max(0.001);
        drop(cam);
        self.camera_generation += 1;
    }

    /// Fit the camera to a world-space bounding box (corners p1, p2).
    pub fn zoom_to_window(&mut self, p1: glam::Vec3, p2: glam::Vec3) {
        let min = p1.min(p2);
        let max = p1.max(p2);
        if min == max {
            return;
        }
        self.camera.borrow_mut().fit_to_bounds(min, max);
        self.camera_generation += 1;
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

    /// Fit the camera to the bounding box of every entity whose
    /// `layer_name` starts with any of `layer_prefixes`. Returns `true`
    /// when at least one matching entity contributes to the bbox and
    /// the camera was updated; returns `false` (leaving the camera
    /// untouched) when there is no native document or no entity matches.
    ///
    /// Motivation: the PID preview pipeline (`src/io/pid_import.rs`)
    /// emits **real** drawing geometry on layers prefixed with
    /// `PID_OBJECTS_`, `PID_LAYOUT_TEXT`, and `PID_RELATIONSHIPS`, plus
    /// a large ring of **decorative** side panels on `PID_META` /
    /// `PID_FALLBACK` / `PID_CROSSREF` / `PID_UNRESOLVED` /
    /// `PID_STREAMS` / `PID_CLUSTERS` / `PID_SYMBOLS`. `fit_all`
    /// over-weights the panels because they live at far-offset world
    /// coordinates (`SIDE_PANEL_X`, `BOTTOM_PANEL_Y`, …), shrinking
    /// the real drawing to a viewport corner. `fit_layers_matching`
    /// lets `Message::FileOpened`'s PID branch target the main
    /// drawing first and fall back to `fit_all` only when the
    /// preview really has no main geometry.
    pub fn fit_layers_matching(&mut self, layer_prefixes: &[&str]) -> bool {
        let Some(native) = self.native_doc() else {
            return false;
        };

        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        let mut found = false;

        for entity in &native.entities {
            if !layer_prefixes
                .iter()
                .any(|p| entity.layer_name.starts_with(p))
            {
                continue;
            }
            for point in entity_bbox_points(entity) {
                let v = glam::Vec3::new(
                    point[0] as f32,
                    point[1] as f32,
                    point[2] as f32,
                );
                min = min.min(v);
                max = max.max(v);
                found = true;
            }
        }

        if !found {
            return false;
        }
        if min == max {
            max += glam::Vec3::splat(1.0);
        }
        self.camera.borrow_mut().fit_to_bounds(min, max);
        self.camera_generation += 1;
        true
    }

    pub fn update(&mut self, _dt: Duration) {}
}

/// Extract the bbox-contributing points of an entity for
/// `fit_layers_matching`. Covers every entity kind the PID preview
/// pipeline emits plus a few CAD-side kinds so the helper is general
/// enough to reuse outside of PID flows. Entities that return an empty
/// vec are silently ignored (they don't affect the bbox).
fn entity_bbox_points(entity: &nm::Entity) -> Vec<[f64; 3]> {
    use h7cad_native_model::EntityData;
    match &entity.data {
        EntityData::Line { start, end } => vec![*start, *end],
        EntityData::Circle { center, radius } => vec![
            [center[0] - radius, center[1] - radius, center[2]],
            [center[0] + radius, center[1] + radius, center[2]],
        ],
        EntityData::Arc {
            center, radius, ..
        } => vec![
            [center[0] - radius, center[1] - radius, center[2]],
            [center[0] + radius, center[1] + radius, center[2]],
        ],
        EntityData::Ellipse {
            center, major_axis, ..
        } => {
            let r = (major_axis[0].powi(2) + major_axis[1].powi(2) + major_axis[2].powi(2))
                .sqrt();
            vec![
                [center[0] - r, center[1] - r, center[2]],
                [center[0] + r, center[1] + r, center[2]],
            ]
        }
        EntityData::Text { insertion, .. } | EntityData::MText { insertion, .. } => {
            vec![*insertion]
        }
        EntityData::Point { position } => vec![*position],
        EntityData::LwPolyline { vertices, .. } => {
            vertices.iter().map(|v| [v.x, v.y, 0.0]).collect()
        }
        EntityData::Polyline { vertices, .. } => {
            vertices.iter().map(|v| v.position).collect()
        }
        EntityData::Insert { insertion, .. } => vec![*insertion],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::Scene;
    use acadrust::EntityType;
    use acadrust::entities::Viewport;
    use crate::io::native_bridge;
    use h7cad_native_model as nm;
    use std::collections::HashSet;

    fn scene_with_native(native: nm::CadDocument) -> Scene {
        let compat = native_bridge::native_doc_to_acadrust(&native);
        let mut scene = Scene::new();
        scene.document = compat;
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene
    }

    fn line_on_layer(layer: &str, start: [f64; 3], end: [f64; 3]) -> nm::Entity {
        let mut e = nm::Entity::new(nm::EntityData::Line { start, end });
        e.layer_name = layer.into();
        e
    }

    #[test]
    fn fit_layers_matching_returns_true_and_advances_camera_generation_for_matching_layer() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(line_on_layer(
                "PID_OBJECTS_PipeRun",
                [0.0, 0.0, 0.0],
                [100.0, 0.0, 0.0],
            ))
            .expect("primary line");
        native
            .add_entity(line_on_layer(
                "PID_META",
                [5000.0, 5000.0, 0.0],
                [5100.0, 5100.0, 0.0],
            ))
            .expect("decorative line at far offset");

        let mut scene = scene_with_native(native);
        let before = scene.camera_generation;
        let fitted = scene.fit_layers_matching(&["PID_OBJECTS_"]);

        assert!(
            fitted,
            "fit_layers_matching must report success when the primary layer has entities"
        );
        assert_eq!(
            scene.camera_generation,
            before + 1,
            "camera_generation must tick exactly once after a successful fit"
        );
    }

    #[test]
    fn fit_layers_matching_returns_false_without_touching_camera_when_no_layer_matches() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(line_on_layer("0", [0.0, 0.0, 0.0], [1.0, 1.0, 0.0]))
            .expect("cad-layer line");

        let mut scene = scene_with_native(native);
        let before = scene.camera_generation;
        let fitted = scene.fit_layers_matching(&["PID_OBJECTS_"]);

        assert!(
            !fitted,
            "fit_layers_matching must return false when no entity layer matches the prefixes"
        );
        assert_eq!(
            scene.camera_generation, before,
            "camera_generation must NOT tick when fit_layers_matching returns false"
        );
    }

    #[test]
    fn fit_layers_matching_returns_false_without_native_doc() {
        let mut scene = Scene::new();
        let before = scene.camera_generation;
        let fitted = scene.fit_layers_matching(&["PID_OBJECTS_"]);

        assert!(!fitted, "fit_layers_matching must no-op on a scene without a native doc");
        assert_eq!(scene.camera_generation, before);
    }

    #[test]
    fn fit_layers_matching_prefix_semantics_match_any_of_the_prefixes() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(line_on_layer(
                "PID_LAYOUT_TEXT",
                [10.0, 20.0, 0.0],
                [30.0, 40.0, 0.0],
            ))
            .expect("layout-text line");

        let mut scene = scene_with_native(native);
        // First prefix doesn't match; second prefix does. The OR-of-
        // prefixes semantics should let the second one fit.
        let fitted = scene
            .fit_layers_matching(&["PID_OBJECTS_", "PID_LAYOUT_TEXT"]);
        assert!(
            fitted,
            "OR-of-prefixes: second prefix must still trigger a successful fit"
        );
    }

    fn block_with_entities(
        doc: &mut nm::CadDocument,
        name: &str,
        entities: Vec<nm::Entity>,
    ) -> nm::Handle {
        let handle = doc.allocate_handle();
        let mut block = nm::BlockRecord::new(handle, name);
        block.entities = entities;
        doc.insert_block_record(block);
        handle
    }

    #[test]
    fn new_scene_starts_without_native_document() {
        let scene = Scene::new();
        assert!(scene.native_doc().is_none());
    }

    #[test]
    fn nativerender_mixes_supported_native_and_compat_fallback_without_duplicates() {
        let mut native = nm::CadDocument::new();
        let _line_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");

        let compat = native_bridge::native_doc_to_acadrust(&native);
        let mut scene = Scene::new();
        scene.document = compat;
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;

        let mut viewport = Viewport::new();
        viewport.id = 2;
        let viewport_handle = scene.add_entity(EntityType::Viewport(viewport));

        let wires = scene.entity_wires();
        let viewport_matches = wires
            .iter()
            .filter(|wire| wire.name == viewport_handle.value().to_string())
            .count();
        let line_matches = wires
            .iter()
            .filter(|wire| !wire.name.is_empty() && wire.name != viewport_handle.value().to_string())
            .count();

        assert_eq!(viewport_matches, 1, "unsupported compat viewport should remain visible");
        assert_eq!(line_matches, 1, "supported native entity should not be double-rendered");
        assert_eq!(wires.len(), 2, "expected one native wire and one compat fallback wire");
    }

    #[test]
    fn nativerender_insert_uses_native_when_block_is_fully_supported() {
        let mut native = nm::CadDocument::new();
        block_with_entities(
            &mut native,
            "BLOCK_A",
            vec![nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [2.0, 0.0, 0.0],
            })],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_A".into(),
                insertion: [10.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        assert!(wires.iter().any(|wire| wire.name == insert_handle.value().to_string()));
    }

    #[test]
    fn nativerender_insert_falls_back_when_block_contains_unsupported_entity() {
        let mut native = nm::CadDocument::new();
        block_with_entities(
            &mut native,
            "BLOCK_UNSUPPORTED",
            vec![nm::Entity::new(nm::EntityData::Viewport {
                center: [0.0, 0.0, 0.0],
                width: 10.0,
                height: 5.0,
            })],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_UNSUPPORTED".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");

        let scene = scene_with_native(native.clone());
        let entity = native.get_entity(insert_handle).expect("insert entity");
        let rendered = scene.native_render_entity_wires(
            scene.native_doc().expect("native doc"),
            entity,
            insert_handle,
            false,
            &mut HashSet::new(),
        );
        assert!(rendered.is_none());
    }

    #[test]
    fn nativerender_insert_with_hatch_adds_native_hatch_model() {
        let mut native = nm::CadDocument::new();
        block_with_entities(
            &mut native,
            "BLOCK_HATCH",
            vec![
                nm::Entity::new(nm::EntityData::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [2.0, 0.0, 0.0],
                }),
                nm::Entity::new(nm::EntityData::Hatch {
                    pattern_name: "SOLID".into(),
                    solid_fill: true,
                    boundary_paths: vec![nm::HatchBoundaryPath {
                        flags: 2,
                        edges: vec![nm::HatchEdge::Polyline {
                            closed: true,
                            vertices: vec![
                                [0.0, 0.0, 0.0],
                                [2.0, 0.0, 0.0],
                                [2.0, 2.0, 0.0],
                                [0.0, 2.0, 0.0],
                            ],
                        }],
                    }],
                }),
            ],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_HATCH".into(),
                insertion: [10.0, 5.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        let hatches = scene.synced_hatch_models();

        assert!(wires.iter().any(|wire| wire.name == insert_handle.value().to_string()));
        assert_eq!(hatches.len(), 1, "insert-contained hatch should produce one native hatch model");
        assert_eq!(hatches[0].name, "SOLID");
    }

    #[test]
    fn nativerender_selected_insert_tints_nested_native_hatch_model() {
        let mut native = nm::CadDocument::new();
        block_with_entities(
            &mut native,
            "BLOCK_HATCH_SELECTED",
            vec![nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Line {
                        start: [0.0, 0.0],
                        end: [1.0, 0.0],
                    },
                    nm::HatchEdge::Line {
                        start: [1.0, 0.0],
                        end: [1.0, 1.0],
                    },
                    nm::HatchEdge::Line {
                        start: [1.0, 1.0],
                        end: [0.0, 1.0],
                    },
                    nm::HatchEdge::Line {
                        start: [0.0, 1.0],
                        end: [0.0, 0.0],
                    }],
                }],
            })],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_HATCH_SELECTED".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");

        let mut scene = scene_with_native(native);
        scene.selected.insert(acadrust::Handle::new(insert_handle.value()));
        let hatches = scene.synced_hatch_models();

        assert_eq!(hatches.len(), 1);
        assert_eq!(hatches[0].color, [0.15, 0.55, 1.00, hatches[0].color[3]]);
    }

    #[test]
    fn nativerender_projects_native_model_wires_into_paper_viewport() {
        let mut native = nm::CadDocument::new();
        let line_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");

        let mut scene = Scene::new();
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene.current_layout = "Layout1".into();

        let mut viewport = Viewport::new();
        viewport.id = 2;
        viewport.status.is_on = true;
        viewport.center = crate::types::Vector3::new(50.0, 50.0, 0.0);
        viewport.width = 100.0;
        viewport.height = 100.0;
        viewport.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        viewport.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        viewport.view_height = 100.0;
        let viewport_handle = scene.add_entity(EntityType::Viewport(viewport));

        let wires = scene.entity_wires();
        assert!(wires.iter().any(|wire| wire.name == viewport_handle.value().to_string()));
        assert!(
            wires.iter().any(|wire| wire.name == line_handle.value().to_string()),
            "paper viewport should project native model wires",
        );
    }

    #[test]
    fn nativerender_hit_test_wires_in_active_viewport_uses_native_projection() {
        let mut native = nm::CadDocument::new();
        let line_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
            }))
            .expect("native line");

        let mut scene = Scene::new();
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene.current_layout = "Layout1".into();

        let mut viewport = Viewport::new();
        viewport.id = 2;
        viewport.status.is_on = true;
        viewport.center = crate::types::Vector3::new(50.0, 50.0, 0.0);
        viewport.width = 100.0;
        viewport.height = 100.0;
        viewport.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        viewport.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        viewport.view_height = 100.0;
        let viewport_handle = scene.add_entity(EntityType::Viewport(viewport));

        scene.active_viewport = Some(viewport_handle);
        let wires = scene.hit_test_wires();
        assert_eq!(wires.len(), 1, "MSPACE hit-test should only expose viewport content");
        assert_eq!(wires[0].name, line_handle.value().to_string());
    }

    #[test]
    fn nativerender_projects_native_hatch_into_paper_viewport() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![
                            [0.0, 0.0, 0.0],
                            [10.0, 0.0, 0.0],
                            [10.0, 10.0, 0.0],
                            [0.0, 10.0, 0.0],
                        ],
                    }],
                }],
            }))
            .expect("native hatch");

        let mut scene = Scene::new();
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene.current_layout = "Layout1".into();

        let mut viewport = Viewport::new();
        viewport.id = 2;
        viewport.status.is_on = true;
        viewport.center = crate::types::Vector3::new(50.0, 50.0, 0.0);
        viewport.width = 100.0;
        viewport.height = 100.0;
        viewport.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        viewport.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        viewport.view_height = 100.0;
        let _vp_handle = scene.add_entity(EntityType::Viewport(viewport));

        let hatches = scene.synced_hatch_models();
        assert_eq!(hatches.len(), 1);
        assert_eq!(hatches[0].name, "SOLID");
        assert!(hatches[0].boundary.iter().all(|[x, y]| *x >= 0.0 && *y >= 0.0));
    }

    #[test]
    fn nativerender_mspace_hatch_projection_uses_only_active_viewport() {
        let mut native = nm::CadDocument::new();
        native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![
                        nm::HatchEdge::Line {
                            start: [0.0, 0.0],
                            end: [10.0, 0.0],
                        },
                        nm::HatchEdge::Line {
                            start: [10.0, 0.0],
                            end: [10.0, 10.0],
                        },
                        nm::HatchEdge::Line {
                            start: [10.0, 10.0],
                            end: [0.0, 10.0],
                        },
                        nm::HatchEdge::Line {
                            start: [0.0, 10.0],
                            end: [0.0, 0.0],
                        },
                    ],
                }],
            }))
            .expect("native hatch");

        let mut scene = Scene::new();
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene.current_layout = "Layout1".into();

        let mut vp1 = Viewport::new();
        vp1.id = 2;
        vp1.status.is_on = true;
        vp1.center = crate::types::Vector3::new(50.0, 50.0, 0.0);
        vp1.width = 100.0;
        vp1.height = 100.0;
        vp1.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        vp1.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        vp1.view_height = 100.0;
        let vp1_handle = scene.add_entity(EntityType::Viewport(vp1));

        let mut vp2 = Viewport::new();
        vp2.id = 3;
        vp2.status.is_on = true;
        vp2.center = crate::types::Vector3::new(200.0, 200.0, 0.0);
        vp2.width = 100.0;
        vp2.height = 100.0;
        vp2.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        vp2.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        vp2.view_height = 100.0;
        let _vp2_handle = scene.add_entity(EntityType::Viewport(vp2));

        scene.active_viewport = Some(vp1_handle);
        let hatches = scene.synced_hatch_models();
        assert_eq!(hatches.len(), 1, "MSPACE should only project hatch content for the active viewport");
    }

    #[test]
    fn nativerender_paper_viewport_hatch_entries_preserve_native_handle() {
        let mut native = nm::CadDocument::new();
        let hatch_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![
                            [0.0, 0.0, 0.0],
                            [10.0, 0.0, 0.0],
                            [10.0, 10.0, 0.0],
                            [0.0, 10.0, 0.0],
                        ],
                    }],
                }],
            }))
            .expect("native hatch");

        let mut scene = Scene::new();
        scene.set_native_doc(Some(native));
        scene.native_render_enabled = true;
        scene.current_layout = "Layout1".into();

        let mut viewport = Viewport::new();
        viewport.id = 2;
        viewport.status.is_on = true;
        viewport.center = crate::types::Vector3::new(50.0, 50.0, 0.0);
        viewport.width = 100.0;
        viewport.height = 100.0;
        viewport.view_target = crate::types::Vector3::new(0.0, 0.0, 0.0);
        viewport.view_direction = crate::types::Vector3::new(0.0, 0.0, 1.0);
        viewport.view_height = 100.0;
        let _vp_handle = scene.add_entity(EntityType::Viewport(viewport));

        let entries = scene.synced_hatch_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, acadrust::Handle::new(hatch_handle.value()));
    }

    #[test]
    fn nativerender_hatch_model_for_handle_returns_native_model() {
        let mut native = nm::CadDocument::new();
        let hatch_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Line {
                        start: [0.0, 0.0],
                        end: [1.0, 0.0],
                    },
                    nm::HatchEdge::Line {
                        start: [1.0, 0.0],
                        end: [1.0, 1.0],
                    },
                    nm::HatchEdge::Line {
                        start: [1.0, 1.0],
                        end: [0.0, 1.0],
                    },
                    nm::HatchEdge::Line {
                        start: [0.0, 1.0],
                        end: [0.0, 0.0],
                    }],
                }],
            }))
            .expect("native hatch");

        let scene = scene_with_native(native);
        let model = scene.hatch_model_for_handle(acadrust::Handle::new(hatch_handle.value()));
        assert!(model.is_some(), "native hatch should be retrievable by handle");
        assert_eq!(model.unwrap().name, "SOLID");
    }

    #[test]
    fn nativerender_insert_falls_back_when_attribs_present() {
        let mut native = nm::CadDocument::new();
        block_with_entities(&mut native, "BLOCK_ATTR", vec![]);
        let attrib = nm::Entity::new(nm::EntityData::Attrib {
            tag: "TAG".into(),
            value: "VAL".into(),
            insertion: [0.0, 0.0, 0.0],
            height: 1.0,
        });
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_ATTR".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: true,
                attribs: vec![attrib],
            }))
            .expect("insert");

        let scene = scene_with_native(native.clone());
        let entity = native.get_entity(insert_handle).expect("insert entity");
        let rendered = scene.native_render_entity_wires(
            scene.native_doc().expect("native doc"),
            entity,
            insert_handle,
            false,
            &mut HashSet::new(),
        );
        assert!(rendered.is_none());
    }

    #[test]
    fn nativerender_insert_breaks_recursion_with_compat_fallback() {
        let mut native = nm::CadDocument::new();
        let recurse_handle = block_with_entities(
            &mut native,
            "BLOCK_RECURSE",
            vec![nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_RECURSE".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            })],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_RECURSE".into(),
                insertion: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");

        let scene = scene_with_native(native.clone());
        let entity = native.get_entity(insert_handle).expect("insert entity");
        let mut visited = HashSet::from([recurse_handle.value()]);
        let rendered = scene.native_render_entity_wires(
            scene.native_doc().expect("native doc"),
            entity,
            insert_handle,
            false,
            &mut visited,
        );
        assert!(rendered.is_none());
    }

    #[test]
    fn nativerender_dimension_uses_native_adapter_for_linear_or_aligned() {
        let mut native = nm::CadDocument::new();
        let dim_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 0,
                block_name: "*D1".into(),
                style_name: "Standard".into(),
                definition_point: [5.0, 2.0, 0.0],
                text_midpoint: [2.5, 2.0, 0.0],
                text_override: "".into(),
                attachment_point: 0,
                measurement: 5.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [0.0, 0.0, 0.0],
                second_point: [5.0, 0.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [0.0, 0.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("dimension");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        let matches = wires
            .iter()
            .filter(|wire| wire.name == dim_handle.value().to_string())
            .count();
        assert!(matches >= 1, "dimension should render through native adapter");
    }

    #[test]
    fn nativerender_multileader_uses_native_adapter() {
        let mut native = nm::CadDocument::new();
        let mleader_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::MultiLeader {
                content_type: 1,
                text_label: "TAG".into(),
                style_name: "Standard".into(),
                arrowhead_size: 2.5,
                landing_gap: 0.0,
                dogleg_length: 2.5,
                property_override_flags: 0,
                path_type: 1,
                line_color: 256,
                leader_line_weight: -1,
                enable_landing: true,
                enable_dogleg: true,
                enable_annotation_scale: false,
                scale_factor: 1.0,
                text_attachment_direction: 0,
                text_bottom_attachment_type: 9,
                text_top_attachment_type: 9,
                text_location: Some([6.0, 0.0, 4.0]),
                leader_vertices: vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 1.0],
                    [6.0, 0.0, 4.0],
                    [10.0, 0.0, 0.0],
                    [6.0, 0.0, 4.0],
                ],
                leader_root_lengths: vec![3, 2],
            }))
            .expect("multileader");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        let matches = wires
            .iter()
            .filter(|wire| wire.name == mleader_handle.value().to_string() && !wire.points.is_empty())
            .count();
        assert!(matches >= 1, "multileader should render through native adapter");
    }

    #[test]
    fn nativerender_dimension_uses_native_adapter_for_angular2ln() {
        let mut native = nm::CadDocument::new();
        let dim_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 2,
                block_name: "*D2".into(),
                style_name: "Standard".into(),
                definition_point: [0.0, 1.0, 0.0],
                text_midpoint: [1.0, 1.0, 0.0],
                text_override: "".into(),
                attachment_point: 0,
                measurement: 45.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [1.0, 0.0, 0.0],
                second_point: [0.0, 1.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [1.0, 1.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("dimension");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        let matches = wires
            .iter()
            .filter(|wire| wire.name == dim_handle.value().to_string())
            .count();
        assert!(matches >= 1, "Angular2Ln should render through native adapter");
    }

    #[test]
    fn nativerender_dimension_falls_back_for_unsupported_dim_type() {
        let mut native = nm::CadDocument::new();
        let dim_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 7,
                block_name: "*D7".into(),
                style_name: "Standard".into(),
                definition_point: [0.0, 0.0, 0.0],
                text_midpoint: [0.0, 0.0, 0.0],
                text_override: "".into(),
                attachment_point: 0,
                measurement: 45.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [1.0, 0.0, 0.0],
                second_point: [0.0, 1.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [1.0, 1.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("dimension");

        let scene = scene_with_native(native.clone());
        let entity = native.get_entity(dim_handle).expect("dim entity");
        let rendered = scene.native_render_entity_wires(
            scene.native_doc().expect("native doc"),
            entity,
            dim_handle,
            false,
            &mut HashSet::new(),
        );
        assert!(rendered.is_none());
    }

    #[test]
    fn nativerender_mixed_insert_dimension_does_not_duplicate_parent_handle() {
        let mut native = nm::CadDocument::new();
        block_with_entities(
            &mut native,
            "BLOCK_DIM",
            vec![nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [1.0, 0.0, 0.0],
            })],
        );
        let insert_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Insert {
                block_name: "BLOCK_DIM".into(),
                insertion: [10.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                rotation: 0.0,
                has_attribs: false,
                attribs: vec![],
            }))
            .expect("insert");
        let dim_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Dimension {
                dim_type: 1,
                block_name: "*D1".into(),
                style_name: "Standard".into(),
                definition_point: [4.0, 2.0, 0.0],
                text_midpoint: [2.0, 2.0, 0.0],
                text_override: "".into(),
                attachment_point: 0,
                measurement: 4.0,
                text_rotation: 0.0,
                horizontal_direction: 0.0,
                flip_arrow1: false,
                flip_arrow2: false,
                first_point: [0.0, 0.0, 0.0],
                second_point: [4.0, 0.0, 0.0],
                angle_vertex: [0.0, 0.0, 0.0],
                dimension_arc: [0.0, 0.0, 0.0],
                leader_length: 0.0,
                rotation: 0.0,
                ext_line_rotation: 0.0,
            }))
            .expect("dimension");

        let scene = scene_with_native(native);
        let wires = scene.entity_wires();
        let unique_names: std::collections::HashSet<_> =
            wires.iter().map(|wire| wire.name.clone()).collect();

        assert!(wires.iter().any(|wire| wire.name == insert_handle.value().to_string()));
        assert!(wires.iter().any(|wire| wire.name == dim_handle.value().to_string()));
        assert_eq!(unique_names.len(), 2, "insert and dimension should each own one parent handle namespace");
    }

    #[test]
    fn nativerender_hatch_adds_native_model_when_compat_missing() {
        let mut native = nm::CadDocument::new();
        let hatch_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![
                            [0.0, 0.0, 0.0],
                            [2.0, 0.0, 0.0],
                            [2.0, 2.0, 0.0],
                            [0.0, 2.0, 0.0],
                        ],
                    }],
                }],
            }))
            .expect("hatch");

        let scene = scene_with_native(native);
        assert!(scene.hatches.is_empty(), "compat bridge should not have a native hatch cache entry");
        let models = scene.synced_hatch_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "SOLID");
        assert_eq!(models[0].boundary.len(), 5);
        assert!(!models[0].boundary.is_empty());
        let _ = hatch_handle;
    }

    #[test]
    fn nativerender_hatch_selection_tints_native_model() {
        let mut native = nm::CadDocument::new();
        let hatch_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Line {
                        start: [0.0, 0.0],
                        end: [2.0, 0.0],
                    },
                    nm::HatchEdge::Line {
                        start: [2.0, 0.0],
                        end: [2.0, 2.0],
                    },
                    nm::HatchEdge::Line {
                        start: [2.0, 2.0],
                        end: [0.0, 2.0],
                    },
                    nm::HatchEdge::Line {
                        start: [0.0, 2.0],
                        end: [0.0, 0.0],
                    }],
                }],
            }))
            .expect("hatch");

        let mut scene = scene_with_native(native);
        scene.selected.insert(acadrust::Handle::new(hatch_handle.value()));
        let models = scene.synced_hatch_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].color, [0.15, 0.55, 1.00, models[0].color[3]]);
    }

    #[test]
    fn add_entity_syncs_supported_entity_into_native_document() {
        let mut scene = scene_with_native(nm::CadDocument::new());

        let handle = scene.add_entity(EntityType::Line(acadrust::entities::Line::from_points(
            crate::types::Vector3::new(0.0, 0.0, 0.0),
            crate::types::Vector3::new(5.0, 0.0, 0.0),
        )));

        let native_entity = scene
            .native_doc()
            .and_then(|doc| doc.get_entity(nm::Handle::new(handle.value())));
        assert!(native_entity.is_some(), "supported compat add should mirror into native document");
    }

    #[test]
    fn erase_entities_removes_entity_from_native_document() {
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("native line");
        let mut scene = scene_with_native(native);

        scene.erase_entities(&[acadrust::Handle::new(handle.value())]);

        assert!(
            scene
                .native_doc()
                .and_then(|doc| doc.get_entity(handle))
                .is_none(),
            "erase_entities should remove mirrored native entity"
        );
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

// ── Paper boundary wire ────────────────────────────────────────────────────

// ── Cohen-Sutherland line clipping ───────────────────────────────────────

/// Clip a single segment (x0,y0)→(x1,y1) against the axis-aligned rectangle
/// [xmin,xmax]×[ymin,ymax].  Returns the clipped endpoints or `None` if the
/// segment is entirely outside.
fn cs_clip(
    mut x0: f32, mut y0: f32,
    mut x1: f32, mut y1: f32,
    xmin: f32, ymin: f32, xmax: f32, ymax: f32,
) -> Option<(f32, f32, f32, f32)> {
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;

    let code = |x: f32, y: f32| -> u8 {
        let mut c = 0u8;
        if x < xmin { c |= LEFT; }
        else if x > xmax { c |= RIGHT; }
        if y < ymin { c |= BOTTOM; }
        else if y > ymax { c |= TOP; }
        c
    };

    let mut c0 = code(x0, y0);
    let mut c1 = code(x1, y1);

    loop {
        if c0 | c1 == 0 { return Some((x0, y0, x1, y1)); }
        if c0 & c1 != 0 { return None; }
        let cout = if c0 != 0 { c0 } else { c1 };
        let (x, y);
        if cout & TOP != 0 {
            x = x0 + (x1 - x0) * (ymax - y0) / (y1 - y0);
            y = ymax;
        } else if cout & BOTTOM != 0 {
            x = x0 + (x1 - x0) * (ymin - y0) / (y1 - y0);
            y = ymin;
        } else if cout & RIGHT != 0 {
            y = y0 + (y1 - y0) * (xmax - x0) / (x1 - x0);
            x = xmax;
        } else {
            y = y0 + (y1 - y0) * (xmin - x0) / (x1 - x0);
            x = xmin;
        }
        if cout == c0 { x0 = x; y0 = y; c0 = code(x0, y0); }
        else           { x1 = x; y1 = y; c1 = code(x1, y1); }
    }
}

/// Clip a projected polyline (NaN-separated segments) to the viewport rectangle.
/// Returns a new points vec with proper NaN separators at clip boundaries.
fn clip_polyline_to_rect(
    pts: &[[f32; 3]],
    xmin: f32, ymin: f32, xmax: f32, ymax: f32,
    z: f32,
) -> Vec<[f32; 3]> {
    const NAN3: [f32; 3] = [f32::NAN, f32::NAN, f32::NAN];
    let mut result: Vec<[f32; 3]> = Vec::new();
    let mut i = 0;

    while i < pts.len() {
        // Skip NaN separators.
        if pts[i][0].is_nan() || pts[i][1].is_nan() {
            i += 1;
            continue;
        }
        // Gather contiguous run of finite points.
        let start = i;
        while i < pts.len() && pts[i][0].is_finite() && pts[i][1].is_finite() {
            i += 1;
        }
        let seg = &pts[start..i];
        if seg.len() < 2 {
            continue;
        }

        // Clip each edge and track pen state to insert NaN on lift.
        let mut pen_down = false;
        for j in 0..seg.len() - 1 {
            let [x0, y0, _] = seg[j];
            let [x1, y1, _] = seg[j + 1];
            match cs_clip(x0, y0, x1, y1, xmin, ymin, xmax, ymax) {
                None => { pen_down = false; }
                Some((cx0, cy0, cx1, cy1)) => {
                    if !pen_down {
                        if !result.is_empty() { result.push(NAN3); }
                        result.push([cx0, cy0, z]);
                        pen_down = true;
                    } else if let Some(&[lx, ly, _]) = result.last() {
                        if (lx - cx0).abs() > 1e-4 || (ly - cy0).abs() > 1e-4 {
                            result.push(NAN3);
                            result.push([cx0, cy0, z]);
                        }
                    }
                    result.push([cx1, cy1, z]);
                    // If the exit point was clipped, lift pen.
                    if (cx1 - x1).abs() > 1e-4 || (cy1 - y1).abs() > 1e-4 {
                        pen_down = false;
                    }
                }
            }
        }
    }
    // Remove trailing NaN.
    while result.last().map(|p: &[f32; 3]| p[0].is_nan()).unwrap_or(false) {
        result.pop();
    }
    result
}

fn clip_polygon_to_rect(
    poly: &[[f32; 2]],
    xmin: f32,
    ymin: f32,
    xmax: f32,
    ymax: f32,
) -> Vec<[f32; 2]> {
    fn clip_against_edge(
        input: &[[f32; 2]],
        inside: impl Fn([f32; 2]) -> bool,
        intersect: impl Fn([f32; 2], [f32; 2]) -> [f32; 2],
    ) -> Vec<[f32; 2]> {
        if input.is_empty() {
            return vec![];
        }

        let mut output = Vec::new();
        let mut prev = *input.last().expect("non-empty");
        let mut prev_inside = inside(prev);

        for &curr in input {
            let curr_inside = inside(curr);
            match (prev_inside, curr_inside) {
                (true, true) => output.push(curr),
                (true, false) => output.push(intersect(prev, curr)),
                (false, true) => {
                    output.push(intersect(prev, curr));
                    output.push(curr);
                }
                (false, false) => {}
            }
            prev = curr;
            prev_inside = curr_inside;
        }

        output
    }

    let left = clip_against_edge(poly, |p| p[0] >= xmin, |a, b| {
        let t = (xmin - a[0]) / (b[0] - a[0]);
        [xmin, a[1] + (b[1] - a[1]) * t]
    });
    let right = clip_against_edge(&left, |p| p[0] <= xmax, |a, b| {
        let t = (xmax - a[0]) / (b[0] - a[0]);
        [xmax, a[1] + (b[1] - a[1]) * t]
    });
    let bottom = clip_against_edge(&right, |p| p[1] >= ymin, |a, b| {
        let t = (ymin - a[1]) / (b[1] - a[1]);
        [a[0] + (b[0] - a[0]) * t, ymin]
    });
    clip_against_edge(&bottom, |p| p[1] <= ymax, |a, b| {
        let t = (ymax - a[1]) / (b[1] - a[1]);
        [a[0] + (b[0] - a[0]) * t, ymax]
    })
}

/// A thin white rectangle wire that represents the printable-area boundary
/// of the active paper layout.  Rendered beneath all other paper-space
/// geometry so it acts as a visual "page" backdrop.
fn paper_boundary_wire(x0: f32, y0: f32, x1: f32, y1: f32) -> WireModel {
    WireModel {
        name: "__paper_boundary__".to_string(),
        points: vec![
            [x0, y0, 0.0],
            [x1, y0, 0.0],
            [x1, y1, 0.0],
            [x0, y1, 0.0],
            [x0, y0, 0.0],
        ],
        // Near-white so it stands out against the dark paper-space background.
        color: [0.95, 0.95, 0.95, 1.0],
        selected: false,
        pattern_length: 0.0,
        pattern: [0.0; 8],
        line_weight_px: 1.5,
        snap_pts: vec![],
        tangent_geoms: vec![],
        aci: 0,
            key_vertices: vec![],
    }
}
