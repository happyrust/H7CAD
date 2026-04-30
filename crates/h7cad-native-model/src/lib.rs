use std::collections::{BTreeMap, BTreeSet};

pub mod julian;
pub use julian::{
    format_iso8601, julian_date_to_utc, parse_iso8601, utc_to_julian_date, DateTimeUtc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Handle(pub u64);

impl Handle {
    pub const NULL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfClass {
    pub dxf_name: String,
    pub cpp_class_name: String,
    pub application_name: String,
    pub proxy_flags: i32,
    pub instance_count: i32,
    pub was_a_proxy: bool,
    pub is_an_entity: bool,
}

impl DxfClass {
    pub fn new() -> Self {
        Self {
            dxf_name: String::new(),
            cpp_class_name: String::new(),
            application_name: String::new(),
            proxy_flags: 0,
            instance_count: 0,
            was_a_proxy: false,
            is_an_entity: false,
        }
    }
}

impl Default for DxfClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Layer properties parsed from the LAYER table
#[derive(Debug, Clone, PartialEq)]
pub struct LayerProperties {
    pub handle: Handle,
    pub name: String,
    /// ACI color; negative = layer off
    pub color: i16,
    pub linetype_name: String,
    /// 1/100 mm; -1=Default
    pub lineweight: i16,
    pub is_frozen: bool,
    pub is_locked: bool,
    /// True color (code 420), 0 = not set
    pub true_color: i32,
    pub plot: bool,
}

impl LayerProperties {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            name: name.into(),
            color: 7,
            linetype_name: "Continuous".into(),
            lineweight: -1,
            is_frozen: false,
            is_locked: false,
            true_color: 0,
            plot: true,
        }
    }

    pub fn is_on(&self) -> bool {
        self.color >= 0
    }
}

/// Linetype dash-pattern segment: positive = dash, negative = space, zero = dot
#[derive(Debug, Clone, PartialEq)]
pub struct LinetypeSegment {
    pub length: f64,
}

/// Linetype properties parsed from the LTYPE table
#[derive(Debug, Clone, PartialEq)]
pub struct LinetypeProperties {
    pub handle: Handle,
    pub name: String,
    pub description: String,
    pub pattern_length: f64,
    pub segments: Vec<LinetypeSegment>,
}

impl LinetypeProperties {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            name: name.into(),
            description: String::new(),
            pattern_length: 0.0,
            segments: Vec::new(),
        }
    }

    pub fn is_continuous(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Text style properties parsed from the STYLE table
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyleProperties {
    pub handle: Handle,
    pub name: String,
    pub height: f64,
    pub width_factor: f64,
    pub oblique_angle: f64,
    pub font_name: String,
    pub bigfont_name: String,
    /// Bit flags: 2=backward, 4=upside-down
    pub flags: i16,
}

impl TextStyleProperties {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            name: name.into(),
            height: 0.0,
            width_factor: 1.0,
            oblique_angle: 0.0,
            font_name: "txt".into(),
            bigfont_name: String::new(),
            flags: 0,
        }
    }
}

/// Dimension style properties parsed from the DIMSTYLE table
#[derive(Debug, Clone, PartialEq)]
pub struct DimStyleProperties {
    pub handle: Handle,
    pub name: String,
    /// Overall scale factor (DIMSCALE)
    pub dimscale: f64,
    /// Arrow size (DIMASZ)
    pub dimasz: f64,
    /// Extension line offset (DIMEXO)
    pub dimexo: f64,
    /// Dimension line gap (DIMGAP)
    pub dimgap: f64,
    /// Text height (DIMTXT)
    pub dimtxt: f64,
    /// Decimal places (DIMDEC)
    pub dimdec: i16,
    /// Text style name
    pub dimtxsty_name: String,
    /// Linear unit format (DIMLUNIT)
    pub dimlunit: i16,
    /// Angular unit format (DIMAUNIT)
    pub dimaunit: i16,
}

impl DimStyleProperties {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            name: name.into(),
            dimscale: 1.0,
            dimasz: 2.5,
            dimexo: 0.625,
            dimgap: 0.625,
            dimtxt: 2.5,
            dimdec: 4,
            dimtxsty_name: "Standard".into(),
            dimlunit: 2,
            dimaunit: 0,
        }
    }
}

/// Viewport configuration from the VPORT table
#[derive(Debug, Clone, PartialEq)]
pub struct VPortProperties {
    pub handle: Handle,
    pub name: String,
    /// Lower-left corner of viewport (code 10/20)
    pub lower_left: [f64; 2],
    /// Upper-right corner of viewport (code 11/21)
    pub upper_right: [f64; 2],
    /// View center point (code 12/22)
    pub view_center: [f64; 2],
    /// View height (code 40)
    pub view_height: f64,
    /// View width (code 41 — aspect ratio)
    pub aspect_ratio: f64,
    /// View direction (code 16/26/36)
    pub view_direction: [f64; 3],
    /// View target point (code 17/27/37)
    pub view_target: [f64; 3],
}

impl VPortProperties {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handle: Handle::NULL,
            name: name.into(),
            lower_left: [0.0, 0.0],
            upper_right: [1.0, 1.0],
            view_center: [0.0, 0.0],
            view_height: 1.0,
            aspect_ratio: 1.0,
            view_direction: [0.0, 0.0, 1.0],
            view_target: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CadDocument {
    pub header: DocumentHeader,
    pub classes: Vec<DxfClass>,
    pub tables: Tables,
    pub layers: BTreeMap<String, LayerProperties>,
    pub linetypes: BTreeMap<String, LinetypeProperties>,
    pub text_styles: BTreeMap<String, TextStyleProperties>,
    pub dim_styles: BTreeMap<String, DimStyleProperties>,
    pub vports: BTreeMap<String, VPortProperties>,
    pub block_records: BTreeMap<Handle, BlockRecord>,
    pub layouts: BTreeMap<Handle, Layout>,
    pub root_dictionary: RootDictionary,
    pub entities: Vec<Entity>,
    pub objects: Vec<CadObject>,
    next_handle: u64,
}

impl CadDocument {
    pub fn new() -> Self {
        let mut next_handle = 1;
        let model_space = BlockRecord::new_reserved(
            allocate_reserved_handle(&mut next_handle),
            "*Model_Space",
        );
        let paper_space = BlockRecord::new_reserved(
            allocate_reserved_handle(&mut next_handle),
            "*Paper_Space",
        );
        let model_layout = Layout::new_reserved(
            allocate_reserved_handle(&mut next_handle),
            "Model",
            model_space.handle,
        );
        let paper_layout = Layout::new_reserved(
            allocate_reserved_handle(&mut next_handle),
            "Layout1",
            paper_space.handle,
        );
        let model_space_handle = model_space.handle;
        let paper_space_handle = paper_space.handle;

        let mut block_records = BTreeMap::new();
        block_records.insert(
            model_space.handle,
            model_space.with_layout(model_layout.handle),
        );
        block_records.insert(
            paper_space.handle,
            paper_space.with_layout(paper_layout.handle),
        );

        let mut layouts = BTreeMap::new();
        layouts.insert(model_layout.handle, model_layout.clone());
        layouts.insert(paper_layout.handle, paper_layout.clone());

        let mut root_dictionary = RootDictionary::new(allocate_reserved_handle(&mut next_handle));
        root_dictionary.insert("ACAD_GROUP", Handle::NULL);
        root_dictionary.insert("ACAD_LAYOUT", root_dictionary.handle);
        root_dictionary.insert("ACAD_PLOTSETTINGS", Handle::NULL);
        root_dictionary.insert("ACAD_PLOTSTYLENAME", Handle::NULL);

        let mut layers = BTreeMap::new();
        layers.insert("0".to_string(), LayerProperties::new("0"));

        let mut linetypes = BTreeMap::new();
        linetypes.insert("Continuous".into(), LinetypeProperties::new("Continuous"));
        linetypes.insert("ByLayer".into(), LinetypeProperties::new("ByLayer"));
        linetypes.insert("ByBlock".into(), LinetypeProperties::new("ByBlock"));

        let mut text_styles = BTreeMap::new();
        text_styles.insert("Standard".into(), TextStyleProperties::new("Standard"));

        let mut dim_styles = BTreeMap::new();
        dim_styles.insert("Standard".into(), DimStyleProperties::new("Standard"));

        let mut vports = BTreeMap::new();
        vports.insert("*Active".into(), VPortProperties::new("*Active"));

        Self {
            header: DocumentHeader::default(),
            classes: Vec::new(),
            tables: Tables::new(model_space_handle, paper_space_handle),
            layers,
            linetypes,
            text_styles,
            dim_styles,
            vports,
            block_records,
            layouts,
            root_dictionary,
            entities: Vec::new(),
            objects: Vec::new(),
            next_handle,
        }
    }

    pub fn next_handle(&self) -> u64 {
        self.next_handle
    }

    pub fn set_next_handle(&mut self, value: u64) {
        self.next_handle = self.next_handle.max(value);
    }

    pub fn allocate_handle(&mut self) -> Handle {
        let handle = Handle::new(self.next_handle);
        self.next_handle += 1;
        self.header.handseed = self.header.handseed.max(self.next_handle);
        handle
    }

    pub fn model_space_handle(&self) -> Handle {
        self.tables
            .block_record
            .entries
            .get("*Model_Space")
            .copied()
            .unwrap_or(Handle::NULL)
    }

    pub fn paper_space_handle(&self) -> Handle {
        self.tables
            .block_record
            .entries
            .get("*Paper_Space")
            .copied()
            .unwrap_or(Handle::NULL)
    }

    pub fn insert_block_record(&mut self, block_record: BlockRecord) {
        self.tables
            .block_record
            .insert(block_record.name.clone(), block_record.handle);
        self.block_records.insert(block_record.handle, block_record);
    }

    pub fn insert_layout(&mut self, layout: Layout) {
        self.layouts.insert(layout.handle, layout);
    }

    pub fn layout_by_name(&self, name: &str) -> Option<&Layout> {
        self.layouts
            .values()
            .find(|layout| layout.name.eq_ignore_ascii_case(name))
    }

    pub fn get_entity(&self, handle: Handle) -> Option<&Entity> {
        self.entities
            .iter()
            .find(|entity| entity.handle == handle)
            .or_else(|| {
                self.block_records
                    .values()
                    .find_map(|br| br.entities.iter().find(|entity| entity.handle == handle))
            })
    }

    pub fn get_entity_mut(&mut self, handle: Handle) -> Option<&mut Entity> {
        if let Some(entity) = self
            .entities
            .iter_mut()
            .find(|entity| entity.handle == handle)
        {
            return Some(entity);
        }

        self.block_records
            .values_mut()
            .find_map(|br| br.entities.iter_mut().find(|entity| entity.handle == handle))
    }

    pub fn add_entity(&mut self, mut entity: Entity) -> Result<Handle, String> {
        if entity.owner_handle == Handle::NULL {
            entity.owner_handle = self.model_space_handle();
        }
        self.finalize_entity_handles(&mut entity);
        let handle = entity.handle;
        self.store_entity(entity)?;
        Ok(handle)
    }

    pub fn add_entity_to_layout(
        &mut self,
        mut entity: Entity,
        layout_name: &str,
    ) -> Result<Handle, String> {
        let owner_handle = if layout_name.eq_ignore_ascii_case("Model") {
            self.model_space_handle()
        } else {
            self.layout_by_name(layout_name)
                .ok_or_else(|| format!("layout not found: {layout_name}"))?
                .block_record_handle
        };

        entity.owner_handle = owner_handle;
        self.finalize_entity_handles(&mut entity);
        let handle = entity.handle;
        self.store_entity(entity)?;
        Ok(handle)
    }

    pub fn remove_entity(&mut self, handle: Handle) -> Option<Entity> {
        if let Some(index) = self.entities.iter().position(|entity| entity.handle == handle) {
            return Some(self.entities.remove(index));
        }

        for br in self.block_records.values_mut() {
            if let Some(index) = br.entities.iter().position(|entity| entity.handle == handle) {
                return Some(br.entities.remove(index));
            }
        }

        None
    }

    fn finalize_entity_handles(&mut self, entity: &mut Entity) {
        if entity.handle == Handle::NULL {
            entity.handle = self.allocate_handle();
        } else {
            self.set_next_handle(entity.handle.value() + 1);
        }

        if let EntityData::Insert { attribs, .. } = &mut entity.data {
            for attrib in attribs.iter_mut() {
                if attrib.handle == Handle::NULL {
                    attrib.handle = self.allocate_handle();
                } else {
                    self.set_next_handle(attrib.handle.value() + 1);
                }
                attrib.owner_handle = entity.handle;
            }
        }
    }

    fn store_entity(&mut self, mut entity: Entity) -> Result<(), String> {
        let owner_handle = entity.owner_handle;
        if owner_handle == Handle::NULL {
            self.entities.push(entity);
            return Ok(());
        }

        let owner_br_handle = self.block_record_by_any_handle(owner_handle).map(|br| br.handle);
        let owner_br_handle = match owner_br_handle {
            Some(handle) => handle,
            None => {
                // R50-LINE-HANDLE-RECOVERY (2026-04-28): native DWG recovery
                // can surface entities whose decoded `owner_handle` does not
                // resolve to any known block record (e.g. when the AC1015
                // recovery pipeline finds 82 LINE entities in
                // `pending.handle_offsets` but only 26 have owner handles
                // pointing at the resolved block-record table). Hard-erroring
                // here used to drop those 56 entities silently, leaving the
                // user with `read_dwg recovered 26 LINE` instead of 82.
                //
                // Graceful fallback: route the orphan into model space so the
                // entity is still rendered/exported. The original
                // `owner_handle` would otherwise alias an invalid block-record
                // handle, so we rewrite it to the model-space handle so later
                // ownership repair / round-trip writes stay consistent.
                entity.owner_handle = self.model_space_handle();
                self.entities.push(entity);
                return Ok(());
            }
        };

        let is_layout_block = self
            .block_records
            .get(&owner_br_handle)
            .and_then(|br| br.layout_handle)
            .is_some();

        if is_layout_block {
            self.entities.push(entity);
            return Ok(());
        }

        if let Some(block_record) = self.block_records.get_mut(&owner_br_handle) {
            block_record.entities.push(entity);
            Ok(())
        } else {
            Err(format!(
                "block record {:X} disappeared while storing entity",
                owner_br_handle.value()
            ))
        }
    }

    pub fn repair_ownership(&mut self) {
        let mut seen_layouts = BTreeSet::new();

        for block_record in self.block_records.values_mut() {
            if let Some(layout_handle) = block_record.layout_handle {
                seen_layouts.insert(layout_handle);
                if let Some(layout) = self.layouts.get_mut(&layout_handle) {
                    layout.block_record_handle = block_record.handle;
                    if layout.owner == Handle::NULL {
                        layout.owner = self.root_dictionary.handle;
                    }
                }
            }
        }

        for layout in self.layouts.values_mut() {
            if layout.owner == Handle::NULL {
                layout.owner = self.root_dictionary.handle;
            }

            if let Some(block_record) = self.block_records.get_mut(&layout.block_record_handle) {
                if block_record.layout_handle.is_none() {
                    block_record.layout_handle = Some(layout.handle);
                }
                seen_layouts.insert(layout.handle);
            }
        }

        for layout_handle in seen_layouts {
            self.root_dictionary.insert(layout_entry_name(layout_handle), layout_handle);
        }
    }

    /// Resolve the effective ACI color for an entity (handles ByLayer=256)
    pub fn resolve_color(&self, entity: &Entity) -> i16 {
        if entity.color_index == 256 {
            self.layers
                .get(&entity.layer_name)
                .map(|l| l.color.abs())
                .unwrap_or(7)
        } else {
            entity.color_index
        }
    }

    /// Resolve the effective linetype name (handles empty or "ByLayer")
    pub fn resolve_linetype<'a>(&'a self, entity: &'a Entity) -> &'a str {
        let lt = &entity.linetype_name;
        if lt.is_empty() || lt.eq_ignore_ascii_case("BYLAYER") {
            self.layers
                .get(&entity.layer_name)
                .map(|l| l.linetype_name.as_str())
                .unwrap_or("Continuous")
        } else {
            lt.as_str()
        }
    }

    /// Resolve lineweight in 1/100mm (handles -1=ByLayer, -2=ByBlock, -3=Default)
    pub fn resolve_lineweight(&self, entity: &Entity) -> i16 {
        match entity.lineweight {
            -1 => {
                self.layers
                    .get(&entity.layer_name)
                    .map(|l| if l.lineweight < 0 { 25 } else { l.lineweight })
                    .unwrap_or(25)
            }
            -3 => 25,
            w => w,
        }
    }

    /// Find block record by BLOCK_RECORD handle (primary key)
    pub fn block_record_by_handle(&self, handle: Handle) -> Option<&BlockRecord> {
        self.block_records.get(&handle)
    }

    /// Find block record by any associated handle (BLOCK_RECORD or BLOCK entity)
    pub fn block_record_by_any_handle(&self, handle: Handle) -> Option<&BlockRecord> {
        if let Some(br) = self.block_records.get(&handle) {
            return Some(br);
        }
        self.block_records
            .values()
            .find(|br| br.block_entity_handle == handle)
    }

    /// Find block record by name
    pub fn block_record_by_name(&self, name: &str) -> Option<&BlockRecord> {
        let handle = self.tables.block_record.entries.get(name).copied()?;
        self.block_records.get(&handle)
    }

    /// Resolve INSERT entity's target block record
    pub fn resolve_insert_block(&self, entity: &Entity) -> Option<&BlockRecord> {
        if let EntityData::Insert { block_name, .. } = &entity.data {
            self.block_record_by_name(block_name)
        } else {
            None
        }
    }

    /// Find the entity's owner block record (via owner_handle, checks both BLOCK_RECORD and BLOCK entity handles)
    pub fn entity_owner_block(&self, entity: &Entity) -> Option<&BlockRecord> {
        if entity.owner_handle == Handle::NULL {
            return None;
        }
        self.block_record_by_any_handle(entity.owner_handle)
    }

    /// Check if entity is in model space
    pub fn is_model_space_entity(&self, entity: &Entity) -> bool {
        if entity.owner_handle == Handle::NULL {
            return false;
        }
        if let Some(br) = self.block_record_by_any_handle(entity.owner_handle) {
            br.name == "*Model_Space"
        } else {
            false
        }
    }

    /// Iterate over model-space entities only
    pub fn model_space_entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter().filter(|e| self.is_model_space_entity(e))
    }

    /// Iterate over paper-space entities only
    pub fn paper_space_entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter().filter(|e| {
            e.owner_handle != Handle::NULL && !self.is_model_space_entity(e)
        })
    }

    /// Compute drawing extents from entity geometry (useful when header EXTMIN/EXTMAX is stale)
    pub fn compute_extents(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        let mut found = false;

        let mut update = |pt: &[f64; 3]| {
            found = true;
            for i in 0..3 {
                min[i] = min[i].min(pt[i]);
                max[i] = max[i].max(pt[i]);
            }
        };

        for entity in self.model_space_entities() {
            match &entity.data {
                EntityData::Line { start, end } => {
                    update(start);
                    update(end);
                }
                EntityData::Circle { center, .. }
                | EntityData::Arc { center, .. } => {
                    update(center);
                }
                EntityData::Point { position } => {
                    update(position);
                }
                EntityData::Ellipse { center, .. } => {
                    update(center);
                }
                EntityData::Insert { insertion, .. } => {
                    update(insertion);
                }
                EntityData::Text { insertion, .. } => {
                    update(insertion);
                }
                EntityData::MText { insertion, .. } => {
                    update(insertion);
                }
                _ => {}
            }
        }

        if found {
            Some((min, max))
        } else {
            None
        }
    }

    /// Count entities by type (for diagnostics)
    pub fn entity_type_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for entity in &self.entities {
            let type_name = entity.data.type_name();
            *counts.entry(type_name).or_insert(0) += 1;
        }
        counts
    }
}

impl Default for CadDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentHeader {
    pub version: DxfVersion,
    pub insbase: [f64; 3],
    pub extmin: [f64; 3],
    pub extmax: [f64; 3],
    pub limmin: [f64; 2],
    pub limmax: [f64; 2],
    pub ltscale: f64,
    pub pdmode: i32,
    pub pdsize: f64,
    pub textsize: f64,
    pub dimscale: f64,
    pub lunits: i16,
    pub luprec: i16,
    pub aunits: i16,
    pub auprec: i16,

    // Drawing mode flags (DXF code 70 / bool)
    /// `$ORTHOMODE`: orthogonal mode on/off.
    pub orthomode: bool,
    /// `$GRIDMODE`: grid display on/off.
    pub gridmode: bool,
    /// `$SNAPMODE`: snap mode on/off.
    pub snapmode: bool,
    /// `$FILLMODE`: fill solid geometry on/off. Default true.
    pub fillmode: bool,
    /// `$MIRRTEXT`: mirror text when mirrored. Default false.
    pub mirrtext: bool,
    /// `$ATTMODE`: attribute visibility tri-state (0=off, 1=normal, 2=on).
    pub attmode: i16,

    // Snap & grid geometry — value side of the `snapmode / gridmode /
    // orthomode` tri-bool above. io layer is a pure passthrough; any
    // semantic validation (e.g. "`snap_style == 1` requires x==y snap
    // spacing") is UI / command layer concern.
    /// `$SNAPBASE` (codes 10/20): snap grid base point in current UCS.
    /// Default `[0.0, 0.0]`.
    pub snap_base: [f64; 2],
    /// `$SNAPUNIT` (codes 10/20): X / Y snap spacing. Default `[0.5, 0.5]`
    /// (AutoCAD imperial template baseline).
    pub snap_unit: [f64; 2],
    /// `$SNAPSTYLE` (code 70): 0 = rectangular, 1 = isometric. Default 0.
    pub snap_style: i16,
    /// `$SNAPANG` (code 50): snap grid rotation, radians. Default 0.0.
    pub snap_ang: f64,
    /// `$SNAPISOPAIR` (code 70): isometric plane selection
    /// (0 = left, 1 = top, 2 = right). Only meaningful when
    /// `snap_style == 1`. Default 0.
    pub snap_iso_pair: i16,
    /// `$GRIDUNIT` (codes 10/20): grid display spacing X / Y. Independent
    /// of `snap_unit` — AutoCAD lets snap and grid use different spacings.
    /// Default `[0.5, 0.5]`.
    pub grid_unit: [f64; 2],

    // Display & render flags — value side of the default 3D viewport /
    // shading behaviour. All stored as `i16` for family consistency
    // (AutoCAD stores all five at code 70). io layer is a pure
    // passthrough; semantic clamping (e.g. `shadedif ∈ 0..=100`) is the
    // UI's concern.
    /// `$DISPSILH` (code 70): display silhouette edges on 3D solids in
    /// wireframe views. 0 = off (default), 1 = on.
    pub dispsilh: i16,
    /// `$DRAGMODE` (code 70): interactive drag preview.
    /// 0 = off, 1 = on, 2 = auto (default — AutoCAD picks).
    pub dragmode: i16,
    /// `$REGENMODE` (code 70): automatic geometry regeneration on zoom
    /// / view change. 0 = manual REGEN required, 1 = auto (default).
    pub regenmode: i16,
    /// `$SHADEDGE` (code 70): SHADE command edge / face combination.
    /// 0 = faces shaded, no edges;
    /// 1 = faces shaded + edges drawn;
    /// 2 = faces hidden-line;
    /// 3 = faces wireframe (default).
    pub shadedge: i16,
    /// `$SHADEDIF` (code 70): diffuse-to-ambient light ratio during
    /// SHADE, as a percentage 0..=100. AutoCAD default 70. io layer
    /// stores the raw i16 — UI is responsible for clamping on input.
    pub shadedif: i16,

    // Current drawing attributes
    /// `$CLAYER` (code 8): current layer name.
    pub clayer: String,
    /// `$CECOLOR` (code 62): current ACI color; 256 = BYLAYER, 0 = BYBLOCK.
    pub cecolor: i16,
    /// `$CELTYPE` (code 6): current linetype name.
    pub celtype: String,
    /// `$CELWEIGHT` (code 370): current lineweight in 1/100 mm. -1 = ByLayer,
    /// -2 = ByBlock, -3 = Default.
    pub celweight: i16,
    /// `$CELTSCALE` (code 40): current linetype scale factor.
    pub celtscale: f64,
    /// `$CETRANSPARENCY` (code 440): current transparency (0 = opaque).
    pub cetransparency: i32,

    // Angular conventions
    /// `$ANGBASE` (code 50): angle zero-direction, radians.
    pub angbase: f64,
    /// `$ANGDIR` (code 70): angle direction; false = counter-clockwise
    /// (default), true = clockwise.
    pub angdir: bool,

    // Linetype-space scaling
    /// `$PSLTSCALE` (code 70): paper-space linetype scaling on/off.
    /// Default true.
    pub psltscale: bool,

    // UCS (User Coordinate System) metadata.
    /// `$UCSBASE` (code 2): name of UCS defining origin/orientation of
    /// orthographic UCS settings. Default empty.
    pub ucsbase: String,
    /// `$UCSNAME` (code 2): name of the current UCS. Default empty
    /// (i.e. current UCS equals WCS).
    pub ucsname: String,
    /// `$UCSORG` (codes 10/20/30): UCS origin point in WCS coords.
    pub ucsorg: [f64; 3],
    /// `$UCSXDIR` (codes 10/20/30): UCS X-axis direction in WCS.
    pub ucsxdir: [f64; 3],
    /// `$UCSYDIR` (codes 10/20/30): UCS Y-axis direction in WCS.
    pub ucsydir: [f64; 3],

    // Timestamp metadata. All four values are raw f64 Julian dates or
    // fractional days; H7CAD does not do a Julian-date → `DateTime`
    // conversion in-core (keeps `chrono` out of the dependency tree)
    // and expects the UI layer to format for display.
    /// `$TDCREATE` (code 40): drawing creation time as Julian date.
    pub tdcreate: f64,
    /// `$TDUPDATE` (code 40): drawing last-update time as Julian date.
    pub tdupdate: f64,
    /// `$TDINDWG` (code 40): cumulative editing time in fractional days.
    pub tdindwg: f64,
    /// `$TDUSRTIMER` (code 40): user-elapsed timer in fractional days.
    pub tdusrtimer: f64,

    // Active-view metadata.
    /// `$VIEWCTR` (codes 10/20): current view center point (WCS).
    pub viewctr: [f64; 2],
    /// `$VIEWSIZE` (code 40): current view height in world units.
    /// Default 1.0.
    pub viewsize: f64,
    /// `$VIEWDIR` (codes 10/20/30): current view direction, from view
    /// target toward the eye (WCS). Default `[0, 0, 1]` (top-down plan).
    pub viewdir: [f64; 3],

    // Default dimension style — Tier 1 subset (most common 8 of 100+
    // AutoCAD `$DIM*` HEADER variables). Defaults match AutoCAD new
    // imperial drawings.
    /// `$DIMTXT` (code 40): dimension text height. Default 0.18.
    pub dimtxt: f64,
    /// `$DIMASZ` (code 40): arrow size. Default 0.18.
    pub dimasz: f64,
    /// `$DIMEXO` (code 40): extension-line origin offset. Default 0.0625.
    pub dimexo: f64,
    /// `$DIMEXE` (code 40): extension-line extension. Default 0.18.
    pub dimexe: f64,
    /// `$DIMGAP` (code 40): dimension-text gap. Default 0.09.
    pub dimgap: f64,
    /// `$DIMDEC` (code 70): decimal places for linear dims. Default 4.
    pub dimdec: i16,
    /// `$DIMADEC` (code 70): decimal places for angular dims. Default 0.
    pub dimadec: i16,
    /// `$DIMTOFL` (code 70): force dim text inside extension lines.
    /// Default false.
    pub dimtofl: bool,
    /// `$DIMSTYLE` (code 2): current dimension style name.
    /// Default `"Standard"`.
    pub dimstyle: String,
    /// `$DIMTXSTY` (code 7): current dimension text style name.
    /// Default `"Standard"`.
    pub dimtxsty: String,

    // Tier-2 dim numerics (measurement text formatting).
    /// `$DIMRND` (code 40): rounding value for dim measurements.
    /// `0.0` means no rounding (raw measured value). Default 0.0.
    pub dimrnd: f64,
    /// `$DIMLFAC` (code 40): linear measurement scale factor. All linear
    /// dims are multiplied by this before display. Default 1.0. Negative
    /// values mean "apply only in paper-space viewports" — stored as
    /// raw f64 passthrough, semantics left to the renderer.
    pub dimlfac: f64,
    /// `$DIMTDEC` (code 70): decimal places for tolerance text (distinct
    /// from `$DIMDEC` which governs the main dim text). Default 4.
    pub dimtdec: i16,
    /// `$DIMFRAC` (code 70): fraction format: 0 = horizontal stacked,
    /// 1 = diagonal stacked, 2 = not stacked. Only meaningful when
    /// `$DIMLUNIT` selects a fractional unit. Default 0.
    pub dimfrac: i16,
    /// `$DIMDSEP` (code 70): decimal separator as an ASCII code point.
    /// 46 = `.` (default, US); 44 = `,` (European). This is the
    /// character's **ASCII value** — unrelated to file IO encoding.
    pub dimdsep: i16,
    /// `$DIMZIN` (code 70): zero-suppression bitfield for dim text.
    /// bit 1 = suppress leading zero, bit 2 = suppress trailing zero,
    /// bit 4 = suppress 0-feet, bit 8 = suppress 0-inches. Bits may
    /// combine; value range is 0–15. Default 0.
    pub dimzin: i16,

    // Dimension alternate units (DIMALT*) — 9-var family driving the
    // "[metric]"-in-brackets parallel display alongside the primary
    // imperial value. io layer is pure passthrough; semantic decoding
    // (enum meaning of `dim_altu`, bit unpacking of the two `*tz / *z`
    // bitfields, validation of `dim_apost`'s "<>" placeholder) is all
    // a UI / dim-renderer concern.
    /// `$DIMALT` (code 70): master on/off for alternate units.
    /// 0 = disabled (default), 1 = enabled.
    pub dim_alt: i16,
    /// `$DIMALTD` (code 70): decimal places for the alt value.
    /// Default 2.
    pub dim_altd: i16,
    /// `$DIMALTF` (code 40): primary → alt conversion factor. Default
    /// 25.4 (inch → mm — AutoCAD's historical factory default for
    /// mixed imperial / metric dimensioning).
    pub dim_altf: f64,
    /// `$DIMALTRND` (code 40): round-off applied to the alt value.
    /// 0.0 = no rounding (default).
    pub dim_altrnd: f64,
    /// `$DIMALTTD` (code 70): decimal places for the alt **tolerance**
    /// text (distinct from `dim_altd` which governs the main alt
    /// value). Default 2.
    pub dim_alttd: i16,
    /// `$DIMALTTZ` (code 70): alt tolerance zero-suppression bitfield.
    /// bit 1 = suppress leading zero, bit 2 = suppress trailing zero,
    /// bit 4 = suppress 0-feet, bit 8 = suppress 0-inches. Bits may
    /// combine; 0..=15. Default 0.
    pub dim_alttz: i16,
    /// `$DIMALTU` (code 70): alt-unit format enum.
    /// 1 = scientific, 2 = decimal (default), 3 = engineering,
    /// 4 = architectural stacked, 5 = fractional stacked,
    /// 6 = architectural, 7 = fractional, 8 = Windows desktop.
    pub dim_altu: i16,
    /// `$DIMALTZ` (code 70): alt-value zero-suppression bitfield.
    /// Same bit layout as `dim_alttz`. Default 0.
    pub dim_altz: i16,
    /// `$DIMAPOST` (code 1): alt-unit text prefix / suffix. `"<>"` is
    /// the placeholder for the numeric value (e.g. `"<> mm"` appends
    /// " mm" after the value). Default empty — no pre/suffix.
    pub dim_apost: String,

    // Dimension arrow / symbol names.
    /// `$DIMBLK` (code 1): global arrowhead block name.
    /// Empty = standard filled arrowhead.
    pub dim_blk: String,
    /// `$DIMBLK1` (code 1): first arrowhead block name (overrides dim_blk).
    pub dim_blk1: String,
    /// `$DIMBLK2` (code 1): second arrowhead block name (overrides dim_blk).
    pub dim_blk2: String,
    /// `$DIMLDRBLK` (code 1): leader arrowhead block name.
    pub dim_ldrblk: String,
    /// `$DIMARCSYM` (code 70): arc length symbol display
    /// (0=before text, 1=above text, 2=none). Default 0.
    pub dim_arcsym: i16,
    /// `$DIMJOGANG` (code 40): jog angle for jogged dimension lines
    /// (radians). Default π/4 ≈ 0.7854.
    pub dim_jogang: f64,

    // Dimension visual control.
    /// `$DIMJUST` (code 70): text horizontal justification.
    /// 0=centered, 1=at first ext line, 2=at second, 3=over first, 4=over second.
    pub dim_just: i16,
    /// `$DIMSD1` (code 70): suppress first dimension line. 0=show, 1=hide.
    pub dim_sd1: i16,
    /// `$DIMSD2` (code 70): suppress second dimension line.
    pub dim_sd2: i16,
    /// `$DIMSE1` (code 70): suppress first extension line.
    pub dim_se1: i16,
    /// `$DIMSE2` (code 70): suppress second extension line.
    pub dim_se2: i16,
    /// `$DIMSOXD` (code 70): suppress outside-extension dimension lines.
    pub dim_soxd: i16,
    /// `$DIMATFIT` (code 70): text & arrows fit method when space is tight.
    /// 0=text+arrows, 1=arrows only, 2=text only, 3=best fit. Default 3.
    pub dim_atfit: i16,
    /// `$DIMAZIN` (code 70): angular zero suppression bitfield.
    pub dim_azin: i16,
    /// `$DIMTIX` (code 70): force text inside extension lines.
    /// 0=no, 1=yes.
    pub dim_tix: i16,

    // Dimension rendering attributes.
    /// `$DIMCLRD` (code 70): dimension line color. Default 0 (BYBLOCK).
    pub dim_clrd: i16,
    /// `$DIMCLRE` (code 70): extension line color. Default 0 (BYBLOCK).
    pub dim_clre: i16,
    /// `$DIMCLRT` (code 70): dimension text color. Default 0 (BYBLOCK).
    pub dim_clrt: i16,
    /// `$DIMLWD` (code 70): dimension line weight. Default -2 (BYBLOCK).
    pub dim_lwd: i16,
    /// `$DIMLWE` (code 70): extension line weight. Default -2 (BYBLOCK).
    pub dim_lwe: i16,
    /// `$DIMTAD` (code 70): text placement above dim line. 0=centered, 1=above. Default 0.
    pub dim_tad: i16,
    /// `$DIMTIH` (code 70): text inside horizontal. Default 1.
    pub dim_tih: i16,
    /// `$DIMTOH` (code 70): text outside horizontal. Default 1.
    pub dim_toh: i16,
    /// `$DIMDLE` (code 40): dim line extension beyond extension lines. Default 0.0.
    pub dim_dle: f64,
    /// `$DIMCEN` (code 40): center mark size. Default 2.5.
    pub dim_cen: f64,
    /// `$DIMTSZ` (code 40): tick size (0=use arrows). Default 0.0.
    pub dim_tsz: f64,

    // Paper space control.
    /// `$PSTYLEMODE` (code 70): plot style type. 0=color-dependent, 1=named. Default 1.
    pub pstylemode: i16,
    /// `$TILEMODE` (code 70): 1=Model space active, 0=Paper space active. Default 1.
    pub tilemode: i16,
    /// `$MAXACTVP` (code 70): maximum active viewports. Default 64.
    pub maxactvp: i16,
    /// `$PSVPSCALE` (code 40): paper space viewport scale factor. Default 0.0.
    pub psvpscale: f64,

    // Miscellaneous flags.
    /// `$TREEDEPTH` (code 70): spatial index tree depth. Default 3020.
    pub treedepth: i16,
    /// `$VISRETAIN` (code 70): retain xref visibility settings. Default 1.
    pub visretain: i16,
    /// `$DELOBJ` (code 70): delete source objects after explode / etc. Default 1.
    pub delobj: i16,
    /// `$PROXYGRAPHICS` (code 70): save proxy entity graphics. Default 1.
    pub proxygraphics: i16,

    // 3D Surface defaults.
    /// `$SURFTAB1` (code 70): surface tabulations in first direction. Default 6.
    pub surftab1: i16,
    /// `$SURFTAB2` (code 70): surface tabulations in second direction. Default 6.
    pub surftab2: i16,
    /// `$SURFTYPE` (code 70): PEDIT smooth surface type. Default 6 (cubic B-spline).
    pub surftype: i16,
    /// `$SURFU` (code 70): surface density in M direction. Default 6.
    pub surfu: i16,
    /// `$SURFV` (code 70): surface density in N direction. Default 6.
    pub surfv: i16,
    /// `$PFACEVMAX` (code 70): max vertices per polyface mesh face. Default 4.
    pub pfacevmax: i16,

    // Additional common variables.
    /// `$MEASUREMENT` (code 70): drawing units. 0=Imperial, 1=Metric. Default 0.
    pub measurement: i16,
    /// `$EXTNAMES` (code 290): use extended symbol names. Default true for AC1018+.
    pub extnames: bool,
    /// `$WORLDVIEW` (code 70): 1=UCS follows when entering new viewpoint. Default 1.
    pub worldview: i16,
    /// `$UNITMODE` (code 70): unit display format mode. Default 0.
    pub unitmode: i16,
    /// `$SPLMAXDEG` (code 70): maximum NURBS spline degree. Default 5 (quintic).
    pub splmaxdeg: i16,

    // Paper space UCS.
    /// `$PUCSBASE` (code 2): paper space UCS base. Default "".
    pub pucsbase: String,
    /// `$PUCSNAME` (code 2): paper space UCS name. Default "".
    pub pucsname: String,
    /// `$PUCSORG` (codes 10/20/30): paper space UCS origin.
    pub pucsorg: [f64; 3],
    /// `$PUCSXDIR` (codes 10/20/30): paper space UCS X direction.
    pub pucsxdir: [f64; 3],
    /// `$PUCSYDIR` (codes 10/20/30): paper space UCS Y direction.
    pub pucsydir: [f64; 3],

    // Additional DIM controls.
    /// `$DIMPOST` (code 1): primary dim text prefix/suffix. Default "".
    pub dim_post: String,
    /// `$DIMLUNIT` (code 70): linear unit format. Default 2 (decimal).
    pub dim_lunit: i16,

    // Object snap / selection.
    /// `$OSMODE` (code 70): object snap modes bitfield. Default 4133.
    pub osmode: i16,
    /// `$PICKSTYLE` (code 70): group/hatch selection mode. Default 1.
    pub pickstyle: i16,
    /// `$LIMCHECK` (code 70): limits checking. Default 0 (off).
    pub limcheck: i16,

    // Rendering / display / metadata.
    /// `$PELEVATION` (code 40): paper space default elevation. Default 0.0.
    pub pelevation: f64,
    /// `$FACETRES` (code 40): facet resolution for ACIS solids. Default 0.5.
    pub facetres: f64,
    /// `$ISOLINES` (code 70): isolines on ACIS surfaces. Default 4.
    pub isolines: i16,
    /// `$TEXTQLTY` (code 70): text quality for TrueType fonts. Default 50.
    pub textqlty: i16,
    /// `$TSTACKALIGN` (code 70): MText stack alignment (1=bottom, 2=center, 3=top). Default 1.
    pub tstackalign: i16,
    /// `$TSTACKSIZE` (code 70): MText stack text size percentage. Default 70.
    pub tstacksize: i16,
    /// `$ACADMAINTVER` (code 70): maintenance version number. Default 0.
    pub acadmaintver: i16,
    /// `$CDATE` (code 40): calendar date/time (Julian date format). Default 0.0.
    pub cdate: f64,
    /// `$LASTSAVEDBY` (code 1): user who last saved the file. Default "".
    pub lastsavedby: String,
    /// `$MENU` (code 1): menu file name. Default ".".
    pub menu: String,

    // Dimension tolerance.
    /// `$DIMTP` (code 40): plus tolerance value. Default 0.0.
    pub dim_tp: f64,
    /// `$DIMTM` (code 40): minus tolerance value. Default 0.0.
    pub dim_tm: f64,
    /// `$DIMTOL` (code 70): generate dimension tolerances. Default 0.
    pub dim_tol: i16,
    /// `$DIMLIM` (code 70): generate dimension limits. Default 0.
    pub dim_lim: i16,
    /// `$DIMTVP` (code 40): text vertical position factor. Default 0.0.
    pub dim_tvp: f64,
    /// `$DIMTFAC` (code 40): tolerance text size scaling factor. Default 1.0.
    pub dim_tfac: f64,
    /// `$DIMTOLJ` (code 70): tolerance vertical justification. Default 1 (middle).
    pub dim_tolj: i16,

    // Additional UI / legacy.
    /// `$COORDS` (code 70): coordinate display mode. Default 1.
    pub coords: i16,
    /// `$SPLTKNOTS` (code 70): spline knot parametrization method. Default 0.
    pub spltknots: i16,
    /// `$BLIPMODE` (code 70): blip display mode. Default 0.
    pub blipmode: i16,

    // User variables.
    pub useri1: i16,
    pub useri2: i16,
    pub useri3: i16,
    pub useri4: i16,
    pub useri5: i16,
    pub userr1: f64,
    pub userr2: f64,
    pub userr3: f64,
    pub userr4: f64,
    pub userr5: f64,

    // Geolocation / 3D walk / misc.
    /// `$LATITUDE` (code 40): site latitude. Default 37.795.
    pub latitude: f64,
    /// `$LONGITUDE` (code 40): site longitude. Default -122.394.
    pub longitude: f64,
    /// `$TIMEZONE` (code 70): timezone enum (IANA offset * 1000). Default -8000 (PST).
    pub timezone: i16,
    /// `$STEPSPERSEC` (code 40): 3D walk steps per second. Default 2.0.
    pub stepspersec: f64,
    /// `$STEPSIZE` (code 40): 3D walk step size. Default 6.0.
    pub stepsize: f64,
    /// `$LENSLENGTH` (code 40): lens focal length (mm). Default 50.0.
    pub lenslength: f64,
    /// `$SKETCHINC` (code 40): sketch record increment. Default 0.1.
    pub sketchinc: f64,

    // Spline defaults.
    /// `$SPLFRAME` (code 70): show spline control polygon. Default false.
    pub splframe: bool,
    /// `$SPLINESEGS` (code 70): line segments per spline patch.
    /// Default 8.
    pub splinesegs: i16,
    /// `$SPLINETYPE` (code 70): default spline curve type
    /// (5 = quadratic B-spline, 6 = cubic B-spline). Default 6.
    pub splinetype: i16,

    // Multi-line (MLINE) defaults.
    /// `$CMLSTYLE` (code 2): current MLine style name. Default `"Standard"`.
    pub cmlstyle: String,
    /// `$CMLJUST` (code 70): current MLine justification
    /// (0 = top, 1 = middle, 2 = bottom). Default 0.
    pub cmljust: i16,
    /// `$CMLSCALE` (code 40): current MLine scale factor. Default 1.0.
    pub cmlscale: f64,

    // Insertion / display / edit miscellany.
    /// `$INSUNITS` (code 70): default insertion units for blocks.
    /// AutoCAD values: 0=unspec, 1=in, 2=ft, 3=mi, 4=mm, 5=cm, 6=m,
    /// 7=km, 8=μin, 9=mil, 10=yd, 11=Å, 12=nm, 13=μm, 14=dm, 15=dam,
    /// 16=hm, 17=Gm, 18=AU, 19=ly, 20=pc. Default 0 (unspecified).
    pub insunits: i16,
    /// `$INSUNITSDEFSOURCE` (code 70): source content units when source
    /// drawing unit is "unspecified". Default 0.
    pub insunits_def_source: i16,
    /// `$INSUNITSDEFTARGET` (code 70): target drawing units when target
    /// is "unspecified". Default 0.
    pub insunits_def_target: i16,
    /// `$LWDISPLAY` (code 290): lineweight display on/off. Default false.
    pub lwdisplay: bool,
    /// `$XEDIT` (code 290): allow external edits to this drawing when
    /// referenced as XREF. Default true.
    pub xedit: bool,

    // Drawing identity and render metadata.
    /// `$FINGERPRINTGUID` (code 2): permanent drawing GUID stamped at
    /// creation time; unchanged across save / copy / rename. Default
    /// empty — io layer only passes the value through. Generating a
    /// fresh GUID for brand-new drawings is a command-layer concern.
    pub fingerprint_guid: String,
    /// `$VERSIONGUID` (code 2): per-save GUID — updated every time the
    /// drawing is written. Default empty (same passthrough policy as
    /// `$FINGERPRINTGUID`).
    pub version_guid: String,
    /// `$DWGCODEPAGE` (code 3): drawing character code page. Legacy
    /// field from R2000–R2006 (ANSI_* families); AutoCAD R2007+ writes
    /// UTF-8 on disk but still emits this for backward compatibility
    /// (commonly `"ANSI_1252"`). Default empty.
    pub dwg_codepage: String,
    /// `$CSHADOW` (code 280): current-entity shadow mode.
    /// 0 = casts and receives shadows (AutoCAD default);
    /// 1 = casts only; 2 = receives only; 3 = ignores shadows.
    pub cshadow: i16,
    /// `$REQUIREDVERSIONS` (code 160): R2018+ required-feature
    /// bitfield. Each bit selects an AutoCAD feature / entity type a
    /// reader must support. H7CAD treats this as an opaque `i64`
    /// passthrough — bit-to-feature mapping is documented by AutoCAD
    /// and is not interpreted at the io layer. Default 0.
    pub required_versions: i64,

    /// `$PROJECTNAME` (code 1): project name for this drawing. AutoCAD
    /// uses it to pick a `ProjectFilePath` subdir when resolving XREF /
    /// raster image paths. Default empty — io layer only passes the
    /// value through; path resolution is a command-layer concern.
    pub project_name: String,
    /// `$HYPERLINKBASE` (code 1): base URL / path for all relative
    /// hyperlinks embedded in the drawing. Default empty (no base).
    pub hyperlink_base: String,
    /// `$INDEXCTL` (code 70): layer / spatial index creation bitfield.
    /// bit 0 = layer index, bit 1 = spatial index. Default 0 (no
    /// indexes created — the most compact drawing). io stores the raw
    /// `i16`; decoding individual bits is a UI / command-layer concern.
    pub indexctl: i16,
    /// `$OLESTARTUP` (code 290): on-open behaviour for OLE objects.
    /// `false` = don't start OLE application when opening drawing
    /// (default — faster); `true` = pre-start. No effect on drawing
    /// content itself — purely a startup hint.
    pub olestartup: bool,

    // Loft 3D defaults — R2007+ LOFT command driver (4 × f64 draft
    // params + 2 × i16 normals / flags). io layer is pure passthrough;
    // semantic meaning of `loft_normals` enum values and `loft_param`
    // bit flags is AutoCAD-documented and UI-decoded.
    /// `$LOFTANG1` (code 40): start cross-section draft angle,
    /// radians. Default 0.0.
    pub loft_ang1: f64,
    /// `$LOFTANG2` (code 40): end cross-section draft angle, radians.
    /// Default 0.0.
    pub loft_ang2: f64,
    /// `$LOFTMAG1` (code 40): start cross-section draft magnitude.
    /// Default 0.0.
    pub loft_mag1: f64,
    /// `$LOFTMAG2` (code 40): end cross-section draft magnitude.
    /// Default 0.0.
    pub loft_mag2: f64,
    /// `$LOFTNORMALS` (code 70): lofted surface normals source.
    /// 0 = ruled, 1 = smooth fit (default), 2 = start cross-section,
    /// 3 = end cross-section, 4 = start and end, 5 = all
    /// cross-sections, 6 = path.
    pub loft_normals: i16,
    /// `$LOFTPARAM` (code 70): lofted surface option bitfield.
    /// bit 1 = no twist, bit 2 = align directions, bit 4 = simple
    /// surfaces, bit 8 = closed / periodic. AutoCAD default 7
    /// (1 + 2 + 4 = three flags on, not closed).
    pub loft_param: i16,

    // Interactive geometry command defaults.
    /// `$CHAMFERA` (code 40): first chamfer distance. Default 0.0.
    pub chamfera: f64,
    /// `$CHAMFERB` (code 40): second chamfer distance. Default 0.0.
    pub chamferb: f64,
    /// `$CHAMFERC` (code 40): chamfer length (distance-angle mode).
    /// Default 0.0.
    pub chamferc: f64,
    /// `$CHAMFERD` (code 40): chamfer angle (distance-angle mode).
    /// Stored as AutoCAD stores it (raw f64 passthrough). Default 0.0.
    pub chamferd: f64,
    /// `$CHAMMODE` (code 70): interactive chamfer input mode.
    /// 0 = distance-distance (uses `$CHAMFERA` / `$CHAMFERB`).
    /// 1 = length-angle (uses `$CHAMFERC` / `$CHAMFERD`).
    /// Stored as `i16` (not `bool`) to leave room for future AutoCAD
    /// tri-state extensions; current spec defines 0 / 1 only. Default 0.
    pub chammode: i16,
    /// `$FILLETRAD` (code 40): default fillet radius. Default 0.0.
    pub filletrad: f64,

    // 2.5-D default attachment for freshly-created entities.
    /// `$ELEVATION` (code 40): default Z value for new entities in the
    /// current UCS. Default 0.0.
    pub elevation: f64,
    /// `$THICKNESS` (code 40): default extrusion thickness for new
    /// entities (LINE / CIRCLE / ARC / TEXT). Default 0.0. Independent
    /// of entity-level `thickness` — this is the per-drawing default,
    /// each entity carries its own override.
    pub thickness: f64,

    pub handseed: u64,
}

impl Default for DocumentHeader {
    fn default() -> Self {
        Self {
            version: DxfVersion::default(),
            insbase: [0.0; 3],
            extmin: [1e20, 1e20, 1e20],
            extmax: [-1e20, -1e20, -1e20],
            limmin: [0.0, 0.0],
            limmax: [12.0, 9.0],
            ltscale: 1.0,
            pdmode: 0,
            pdsize: 0.0,
            textsize: 2.5,
            dimscale: 1.0,
            lunits: 2,
            luprec: 4,
            aunits: 0,
            auprec: 0,

            orthomode: false,
            gridmode: false,
            snapmode: false,
            fillmode: true,
            mirrtext: false,
            attmode: 1,

            snap_base: [0.0, 0.0],
            snap_unit: [0.5, 0.5],
            snap_style: 0,
            snap_ang: 0.0,
            snap_iso_pair: 0,
            grid_unit: [0.5, 0.5],

            dispsilh: 0,
            dragmode: 2,
            regenmode: 1,
            shadedge: 3,
            shadedif: 70,

            clayer: "0".to_string(),
            cecolor: 256,
            celtype: "ByLayer".to_string(),
            celweight: -1,
            celtscale: 1.0,
            cetransparency: 0,

            angbase: 0.0,
            angdir: false,

            psltscale: true,

            ucsbase: String::new(),
            ucsname: String::new(),
            ucsorg: [0.0, 0.0, 0.0],
            ucsxdir: [1.0, 0.0, 0.0],
            ucsydir: [0.0, 1.0, 0.0],

            tdcreate: 0.0,
            tdupdate: 0.0,
            tdindwg: 0.0,
            tdusrtimer: 0.0,

            viewctr: [0.0, 0.0],
            viewsize: 1.0,
            viewdir: [0.0, 0.0, 1.0],

            dimtxt: 0.18,
            dimasz: 0.18,
            dimexo: 0.0625,
            dimexe: 0.18,
            dimgap: 0.09,
            dimdec: 4,
            dimadec: 0,
            dimtofl: false,
            dimstyle: "Standard".to_string(),
            dimtxsty: "Standard".to_string(),

            dimrnd: 0.0,
            dimlfac: 1.0,
            dimtdec: 4,
            dimfrac: 0,
            dimdsep: 46,
            dimzin: 0,

            dim_alt: 0,
            dim_altd: 2,
            dim_altf: 25.4,
            dim_altrnd: 0.0,
            dim_alttd: 2,
            dim_alttz: 0,
            dim_altu: 2,
            dim_altz: 0,
            dim_apost: String::new(),

            dim_blk: String::new(),
            dim_blk1: String::new(),
            dim_blk2: String::new(),
            dim_ldrblk: String::new(),
            dim_arcsym: 0,
            dim_jogang: std::f64::consts::FRAC_PI_4,

            dim_just: 0,
            dim_sd1: 0,
            dim_sd2: 0,
            dim_se1: 0,
            dim_se2: 0,
            dim_soxd: 0,
            dim_atfit: 3,
            dim_azin: 0,
            dim_tix: 0,

            dim_clrd: 0,
            dim_clre: 0,
            dim_clrt: 0,
            dim_lwd: -2,
            dim_lwe: -2,
            dim_tad: 0,
            dim_tih: 1,
            dim_toh: 1,
            dim_dle: 0.0,
            dim_cen: 2.5,
            dim_tsz: 0.0,

            pstylemode: 1,
            tilemode: 1,
            maxactvp: 64,
            psvpscale: 0.0,

            treedepth: 3020,
            visretain: 1,
            delobj: 1,
            proxygraphics: 1,

            surftab1: 6,
            surftab2: 6,
            surftype: 6,
            surfu: 6,
            surfv: 6,
            pfacevmax: 4,

            measurement: 0,
            extnames: true,
            worldview: 1,
            unitmode: 0,
            splmaxdeg: 5,

            pucsbase: String::new(),
            pucsname: String::new(),
            pucsorg: [0.0, 0.0, 0.0],
            pucsxdir: [1.0, 0.0, 0.0],
            pucsydir: [0.0, 1.0, 0.0],

            dim_post: String::new(),
            dim_lunit: 2,

            osmode: 4133,
            pickstyle: 1,
            limcheck: 0,

            pelevation: 0.0,
            facetres: 0.5,
            isolines: 4,
            textqlty: 50,
            tstackalign: 1,
            tstacksize: 70,
            acadmaintver: 0,
            cdate: 0.0,
            lastsavedby: String::new(),
            menu: ".".into(),

            dim_tp: 0.0,
            dim_tm: 0.0,
            dim_tol: 0,
            dim_lim: 0,
            dim_tvp: 0.0,
            dim_tfac: 1.0,
            dim_tolj: 1,

            coords: 1,
            spltknots: 0,
            blipmode: 0,

            useri1: 0, useri2: 0, useri3: 0, useri4: 0, useri5: 0,
            userr1: 0.0, userr2: 0.0, userr3: 0.0, userr4: 0.0, userr5: 0.0,

            latitude: 37.795,
            longitude: -122.394,
            timezone: -8000,
            stepspersec: 2.0,
            stepsize: 6.0,
            lenslength: 50.0,
            sketchinc: 0.1,

            splframe: false,
            splinesegs: 8,
            splinetype: 6,

            cmlstyle: "Standard".to_string(),
            cmljust: 0,
            cmlscale: 1.0,

            insunits: 0,
            insunits_def_source: 0,
            insunits_def_target: 0,
            lwdisplay: false,
            xedit: true,

            fingerprint_guid: String::new(),
            version_guid: String::new(),
            dwg_codepage: String::new(),
            cshadow: 0,
            required_versions: 0,

            project_name: String::new(),
            hyperlink_base: String::new(),
            indexctl: 0,
            olestartup: false,

            loft_ang1: 0.0,
            loft_ang2: 0.0,
            loft_mag1: 0.0,
            loft_mag2: 0.0,
            loft_normals: 1,
            loft_param: 7,

            chamfera: 0.0,
            chamferb: 0.0,
            chamferc: 0.0,
            chamferd: 0.0,
            chammode: 0,
            filletrad: 0.0,

            elevation: 0.0,
            thickness: 0.0,

            handseed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum DxfVersion {
    Unknown,
    R12,
    R13,
    R14,
    #[default]
    R2000,
    R2004,
    R2007,
    R2010,
    R2013,
    R2018,
}

impl DxfVersion {
    pub fn from_acadver(s: &str) -> Self {
        match s.trim() {
            "AC1009" => Self::R12,
            "AC1012" => Self::R13,
            "AC1014" => Self::R14,
            "AC1015" => Self::R2000,
            "AC1018" => Self::R2004,
            "AC1021" => Self::R2007,
            "AC1024" => Self::R2010,
            "AC1027" => Self::R2013,
            "AC1032" => Self::R2018,
            _ => Self::Unknown,
        }
    }

    pub fn to_dxf(self) -> &'static str {
        match self {
            Self::R12 => "AC1009",
            Self::R13 => "AC1012",
            Self::R14 => "AC1014",
            Self::R2000 => "AC1015",
            Self::R2004 => "AC1018",
            Self::R2007 => "AC1021",
            Self::R2010 => "AC1024",
            Self::R2013 => "AC1027",
            Self::R2018 => "AC1032",
            Self::Unknown => "AC1015",
        }
    }

    pub fn is_utf8(self) -> bool {
        self >= Self::R2007
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    pub layer: SymbolTable,
    pub linetype: SymbolTable,
    pub style: SymbolTable,
    pub view: SymbolTable,
    pub ucs: SymbolTable,
    pub appid: SymbolTable,
    pub dimstyle: SymbolTable,
    pub block_record: SymbolTable,
}

impl Tables {
    pub fn new(model_space: Handle, paper_space: Handle) -> Self {
        let mut block_record = SymbolTable::named("BLOCK_RECORD");
        block_record.insert("*Model_Space", model_space);
        block_record.insert("*Paper_Space", paper_space);

        Self {
            layer: SymbolTable::named("LAYER"),
            linetype: SymbolTable::named("LTYPE"),
            style: SymbolTable::named("STYLE"),
            view: SymbolTable::named("VIEW"),
            ucs: SymbolTable::named("UCS"),
            appid: SymbolTable::named("APPID"),
            dimstyle: SymbolTable::named("DIMSTYLE"),
            block_record,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    pub name: &'static str,
    pub entries: BTreeMap<String, Handle>,
}

impl SymbolTable {
    pub fn named(name: &'static str) -> Self {
        Self {
            name,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, handle: Handle) -> Option<Handle> {
        self.entries.insert(name.into(), handle)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockRecord {
    pub handle: Handle,
    /// Handle of the BLOCK entity in the BLOCKS section
    pub block_entity_handle: Handle,
    pub name: String,
    pub layout_handle: Option<Handle>,
    pub entities: Vec<Entity>,
    /// Base point from BLOCK entity (codes 10/20/30)
    pub base_point: [f64; 3],
}

impl BlockRecord {
    pub fn new(handle: Handle, name: impl Into<String>) -> Self {
        Self {
            handle,
            block_entity_handle: Handle::NULL,
            name: name.into(),
            layout_handle: None,
            entities: Vec::new(),
            base_point: [0.0; 3],
        }
    }

    pub fn with_layout(mut self, layout_handle: Handle) -> Self {
        self.layout_handle = Some(layout_handle);
        self
    }

    fn new_reserved(handle: Handle, name: impl Into<String>) -> Self {
        Self::new(handle, name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub handle: Handle,
    pub name: String,
    pub owner: Handle,
    pub block_record_handle: Handle,
}

impl Layout {
    pub fn new(handle: Handle, name: impl Into<String>, block_record_handle: Handle) -> Self {
        Self {
            handle,
            name: name.into(),
            owner: Handle::NULL,
            block_record_handle,
        }
    }

    fn new_reserved(handle: Handle, name: impl Into<String>, block_record_handle: Handle) -> Self {
        let mut layout = Self::new(handle, name, block_record_handle);
        layout.owner = Handle::new(0);
        layout
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootDictionary {
    pub handle: Handle,
    pub entries: BTreeMap<String, Handle>,
}

impl RootDictionary {
    pub fn new(handle: Handle) -> Self {
        Self {
            handle,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, handle: Handle) -> Option<Handle> {
        self.entries.insert(name.into(), handle)
    }
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub handle: Handle,
    /// Owner block record handle (code 330)
    pub owner_handle: Handle,
    pub layer_name: String,
    pub linetype_name: String,
    /// Linetype scale (code 48), 1.0 = default
    pub linetype_scale: f64,
    pub color_index: i16,
    /// True color (code 420) as packed RGB, 0 = not set
    pub true_color: i32,
    /// Line weight in 1/100 mm (code 370), -1=ByLayer, -2=ByBlock, -3=Default
    pub lineweight: i16,
    /// 0=visible, 1=invisible (code 60)
    pub invisible: bool,
    /// Transparency (code 440), 0=fully opaque
    pub transparency: i32,
    /// Thickness (code 39), 0.0 = no thickness
    pub thickness: f64,
    /// Extrusion direction (codes 210/220/230), default [0,0,1]
    pub extrusion: [f64; 3],
    /// Raw XData keyed by application name (code 1001)
    pub xdata: Vec<(String, Vec<(i16, String)>)>,
    pub data: EntityData,
}

impl Entity {
    pub fn new(data: EntityData) -> Self {
        Self {
            handle: Handle::NULL,
            owner_handle: Handle::NULL,
            layer_name: "0".into(),
            linetype_name: String::new(),
            linetype_scale: 1.0,
            color_index: 256, // BYLAYER
            true_color: 0,
            lineweight: -1, // ByLayer
            invisible: false,
            transparency: 0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            xdata: Vec::new(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityData {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Circle {
        center: [f64; 3],
        radius: f64,
    },
    Arc {
        center: [f64; 3],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Point {
        position: [f64; 3],
    },
    LwPolyline {
        vertices: Vec<LwVertex>,
        closed: bool,
        /// Uniform segment width overriding per-vertex widths when > 0
        /// (DXF code 43). Native-model addition (D3 series).
        constant_width: f64,
    },
    Text {
        insertion: [f64; 3],
        height: f64,
        value: String,
        rotation: f64,
        style_name: String,
        width_factor: f64,
        oblique_angle: f64,
        horizontal_alignment: i16,
        vertical_alignment: i16,
        alignment_point: Option<[f64; 3]>,
    },
    Ellipse {
        center: [f64; 3],
        major_axis: [f64; 3],
        ratio: f64,
        start_param: f64,
        end_param: f64,
    },
    Spline {
        degree: i32,
        closed: bool,
        knots: Vec<f64>,
        control_points: Vec<[f64; 3]>,
        weights: Vec<f64>,
        fit_points: Vec<[f64; 3]>,
        start_tangent: [f64; 3],
        end_tangent: [f64; 3],
    },
    Face3D {
        corners: [[f64; 3]; 4],
        invisible_edges: i16,
    },
    Solid {
        corners: [[f64; 3]; 4],
        normal: [f64; 3],
        thickness: f64,
    },
    Ray {
        origin: [f64; 3],
        direction: [f64; 3],
    },
    XLine {
        origin: [f64; 3],
        direction: [f64; 3],
    },
    MText {
        insertion: [f64; 3],
        height: f64,
        width: f64,
        rectangle_height: Option<f64>,
        value: String,
        rotation: f64,
        style_name: String,
        attachment_point: i16,
        line_spacing_factor: f64,
        drawing_direction: i16,
    },
    Insert {
        block_name: String,
        insertion: [f64; 3],
        scale: [f64; 3],
        rotation: f64,
        has_attribs: bool,
        attribs: Vec<Entity>,
    },
    Dimension {
        /// Low 4 bits of code 70: 0=Linear, 1=Aligned, 2=Angular2Line,
        /// 3=Diameter, 4=Radius, 5=Angular3Pt, 6=Ordinate
        dim_type: i16,
        block_name: String,
        style_name: String,
        /// code 10/20/30
        definition_point: [f64; 3],
        /// code 11/21/31
        text_midpoint: [f64; 3],
        text_override: String,
        attachment_point: i16,
        measurement: f64,
        text_rotation: f64,
        horizontal_direction: f64,
        flip_arrow1: bool,
        flip_arrow2: bool,
        /// code 13/23/33 — Aligned/Linear: FirstPoint; Angular: FirstPoint; Ordinate: FeatureLocation
        first_point: [f64; 3],
        /// code 14/24/34 — Aligned/Linear: SecondPoint; Angular: SecondPoint; Ordinate: LeaderEndpoint
        second_point: [f64; 3],
        /// code 15/25/35 — Radius/Diameter/Angular: AngleVertex
        angle_vertex: [f64; 3],
        /// code 16/26/36 — Angular2Line: DimensionArc
        dimension_arc: [f64; 3],
        /// code 40 — Radius/Diameter: LeaderLength
        leader_length: f64,
        /// code 50 — Linear: Rotation
        rotation: f64,
        /// code 52 — Aligned/Linear: ExtLineRotation (oblique angle)
        ext_line_rotation: f64,
    },
    Hatch {
        pattern_name: String,
        solid_fill: bool,
        boundary_paths: Vec<HatchBoundaryPath>,
    },
    Viewport {
        center: [f64; 3],
        width: f64,
        height: f64,
    },
    Polyline {
        polyline_type: PolylineType,
        vertices: Vec<PolylineVertex>,
        closed: bool,
    },
    Attrib {
        tag: String,
        value: String,
        insertion: [f64; 3],
        height: f64,
    },
    AttDef {
        tag: String,
        prompt: String,
        default_value: String,
        insertion: [f64; 3],
        height: f64,
    },
    Leader {
        vertices: Vec<[f64; 3]>,
        has_arrowhead: bool,
    },
    MLine {
        vertices: Vec<[f64; 3]>,
        style_name: String,
        scale: f64,
        /// MLineFlags::CLOSED (DXF code 71 bit 2). True when the multiline
        /// forms a closed polygon. Native-model addition (D1 series).
        closed: bool,
    },
    Image {
        insertion: [f64; 3],
        u_vector: [f64; 3],
        v_vector: [f64; 3],
        image_size: [f64; 2],
        /// DXF code 340: Hard-pointer to the linked IMAGEDEF object.
        /// `Handle::NULL` means the IMAGE entity is unlinked. When writing
        /// DXF, if this is NULL and `file_path` is non-empty, the writer's
        /// `ensure_image_defs` pre-pass will allocate a new handle, insert
        /// a matching `ObjectData::ImageDef` into `doc.objects`, and fill
        /// this field in-place.
        image_def_handle: Handle,
        /// File path to the raster image. Authoritative when
        /// `image_def_handle` is `Handle::NULL`; otherwise this is a
        /// **cached mirror** of the linked `ImageDef.file_name` populated
        /// by `resolve_image_def_links` after DXF read. Kept as a first-
        /// class field so UI / bridge code can read it directly without
        /// chasing the handle. Writers prefer the IMAGEDEF object as the
        /// source of truth and emit code 340 on IMAGE (not code 1).
        file_path: String,
        /// DXF code 70 image display flags bitfield
        /// (bit 1=SHOW_IMAGE, bit 2=SHOW_WHEN_NOT_ALIGNED_WITH_SCREEN,
        ///  bit 4=USE_CLIPPING_BOUNDARY, bit 8=TRANSPARENCY_IS_ON).
        display_flags: i32,
    },
    Wipeout {
        clip_vertices: Vec<[f64; 2]>,
        /// DXF Z of the wipeout's insertion point (code 30). Native-model
        /// addition (D2 series) — previously dropped by bridge.
        elevation: f64,
    },
    Tolerance {
        text: String,
        insertion: [f64; 3],
    },
    Shape {
        insertion: [f64; 3],
        size: f64,
        shape_number: i16,
        name: String,
        rotation: f64,
        relative_x_scale: f64,
        oblique_angle: f64,
        style_name: String,
        normal: [f64; 3],
        thickness: f64,
    },
    Solid3D {
        acis_data: String,
    },
    Region {
        acis_data: String,
    },
    MultiLeader {
        /// 1=MText, 2=Block, 3=Tolerance
        content_type: i16,
        text_label: String,
        style_name: String,
        arrowhead_size: f64,
        landing_gap: f64,
        dogleg_length: f64,
        /// Property override flags (bitfield)
        property_override_flags: u32,
        /// 1=Straight, 2=Spline
        path_type: i16,
        /// Leader line color (ACI or true color int)
        line_color: i32,
        /// Leader line weight (-3=default, -2=byblock, -1=bylayer, or hundredths of mm)
        leader_line_weight: i16,
        enable_landing: bool,
        enable_dogleg: bool,
        enable_annotation_scale: bool,
        /// Overall scale factor
        scale_factor: f64,
        /// 0=Horizontal, 1=Vertical
        text_attachment_direction: i16,
        /// Bottom text attachment type
        text_bottom_attachment_type: i16,
        /// Top text attachment type
        text_top_attachment_type: i16,
        /// Text location from ContextData (code 12,22,32)
        text_location: Option<[f64; 3]>,
        /// Leader line vertices from ContextData (code 10,20,30 after LEADER_LINE marker)
        leader_vertices: Vec<[f64; 3]>,
        /// Number of vertices for each leader root, in sequence
        leader_root_lengths: Vec<usize>,
    },
    Table {
        num_rows: i32,
        num_cols: i32,
        insertion: [f64; 3],
        horizontal_direction: [f64; 3],
        version: i16,
        value_flag: i32,
        row_heights: Vec<f64>,
        column_widths: Vec<f64>,
    },
    Mesh {
        vertex_count: i32,
        face_count: i32,
        vertices: Vec<[f64; 3]>,
        face_indices: Vec<i32>,
    },
    PdfUnderlay {
        insertion: [f64; 3],
        scale: [f64; 3],
    },
    Helix {
        /// code 10/20/30
        axis_base_point: [f64; 3],
        /// code 11/21/31
        start_point: [f64; 3],
        /// code 12/22/32
        axis_vector: [f64; 3],
        /// code 40
        radius: f64,
        /// code 41
        turns: f64,
        /// code 42
        turn_height: f64,
        /// code 280 — 0=cylindrical, 1=conical
        handedness: i16,
        /// code 290 — true=CCW
        is_ccw: bool,
    },
    ArcDimension {
        /// Shared dimension fields (reuse same semantics as Dimension)
        block_name: String,
        style_name: String,
        definition_point: [f64; 3],
        text_midpoint: [f64; 3],
        text_override: String,
        /// code 13/23/33 — First extension definition point
        first_point: [f64; 3],
        /// code 14/24/34 — Second extension definition point
        second_point: [f64; 3],
        /// code 15/25/35 — Arc center
        arc_center: [f64; 3],
        /// code 40 — Leader length (if any)
        leader_length: f64,
        measurement: f64,
    },
    LargeRadialDimension {
        block_name: String,
        style_name: String,
        definition_point: [f64; 3],
        text_midpoint: [f64; 3],
        text_override: String,
        /// code 15/25/35 — chord override point
        chord_point: [f64; 3],
        /// code 40 — Leader length
        leader_length: f64,
        /// code 50 — Jog angle (radians)
        jog_angle: f64,
        measurement: f64,
    },
    /// Generic NURBS / procedural surface.
    /// `surface_kind` is the original DXF type name
    /// (EXTRUDEDSURFACE / LOFTEDSURFACE / REVOLVEDSURFACE / SWEPTSURFACE /
    ///  PLANESURFACE / NURBSURFACE).
    /// ACIS payload is stored as raw text so bridge/writer can round-trip it.
    Surface {
        surface_kind: String,
        /// code 70 — U iso-line count
        u_isolines: i32,
        /// code 71 — V iso-line count
        v_isolines: i32,
        acis_data: String,
    },
    Light {
        /// code 1
        name: String,
        /// code 70 — Light type: 1=Distant, 2=Point, 3=Spot
        light_type: i16,
        /// code 10/20/30
        position: [f64; 3],
        /// code 11/21/31 — Target (spot/distant)
        target: [f64; 3],
        /// code 40 — Intensity
        intensity: f64,
        /// code 290 — Status
        is_on: bool,
        /// code 63 — Color (shadow color)
        color: i16,
        /// code 50 — Hotspot angle (spot)
        hotspot_angle: f64,
        /// code 51 — Falloff angle (spot)
        falloff_angle: f64,
    },
    /// AutoCAD Camera (view helper entity).
    Camera {
        /// code 10/20/30
        position: [f64; 3],
        /// code 11/21/31
        target: [f64; 3],
        /// code 40 — Lens length
        lens_length: f64,
    },
    /// Section plane entity (SECTION / SECTIONOBJECT).
    Section {
        /// code 1 — Section name
        name: String,
        /// code 70 — State
        state: i32,
        /// code 10/20/30 per vertex — Section plane vertices
        vertices: Vec<[f64; 3]>,
        /// code 40/41/42 — vertical direction (normal)
        vertical_direction: [f64; 3],
    },
    /// ACAD_PROXY_ENTITY — preserves raw DXF codes so the entity can
    /// be written back unchanged even though we don't understand it.
    ProxyEntity {
        /// code 90 — Proxy entity class id
        class_id: i32,
        /// code 91 — Application entity class id
        application_class_id: i32,
        /// Raw (code, value) tuples captured verbatim
        raw_codes: Vec<(i16, String)>,
    },
    /// Placeholder for entity types not yet fully parsed
    Unknown {
        entity_type: String,
    },
}

impl EntityData {
    pub fn type_name(&self) -> String {
        match self {
            Self::Line { .. } => "LINE".into(),
            Self::Circle { .. } => "CIRCLE".into(),
            Self::Arc { .. } => "ARC".into(),
            Self::Point { .. } => "POINT".into(),
            Self::Ellipse { .. } => "ELLIPSE".into(),
            Self::LwPolyline { .. } => "LWPOLYLINE".into(),
            Self::Text { .. } => "TEXT".into(),
            Self::MText { .. } => "MTEXT".into(),
            Self::Insert { .. } => "INSERT".into(),
            Self::Hatch { .. } => "HATCH".into(),
            Self::Dimension { .. } => "DIMENSION".into(),
            Self::Viewport { .. } => "VIEWPORT".into(),
            Self::Spline { .. } => "SPLINE".into(),
            Self::Face3D { .. } => "3DFACE".into(),
            Self::Solid { .. } => "SOLID".into(),
            Self::Ray { .. } => "RAY".into(),
            Self::Polyline { .. } => "POLYLINE".into(),
            Self::Attrib { .. } => "ATTRIB".into(),
            Self::AttDef { .. } => "ATTDEF".into(),
            Self::Leader { .. } => "LEADER".into(),
            Self::MLine { .. } => "MLINE".into(),
            Self::Image { .. } => "IMAGE".into(),
            Self::Wipeout { .. } => "WIPEOUT".into(),
            Self::Tolerance { .. } => "TOLERANCE".into(),
            Self::Shape { .. } => "SHAPE".into(),
            Self::XLine { .. } => "XLINE".into(),
            Self::Solid3D { .. } => "3DSOLID".into(),
            Self::Region { .. } => "REGION".into(),
            Self::Table { .. } => "ACAD_TABLE".into(),
            Self::Mesh { .. } => "MESH".into(),
            Self::PdfUnderlay { .. } => "PDFUNDERLAY".into(),
            Self::MultiLeader { .. } => "MULTILEADER".into(),
            Self::Helix { .. } => "HELIX".into(),
            Self::ArcDimension { .. } => "ARC_DIMENSION".into(),
            Self::LargeRadialDimension { .. } => "LARGE_RADIAL_DIMENSION".into(),
            Self::Surface { surface_kind, .. } => surface_kind.clone(),
            Self::Light { .. } => "LIGHT".into(),
            Self::Camera { .. } => "CAMERA".into(),
            Self::Section { .. } => "SECTION".into(),
            Self::ProxyEntity { .. } => "ACAD_PROXY_ENTITY".into(),
            Self::Unknown { entity_type } => entity_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolylineType {
    Polyline2D,
    Polyline3D,
    PolygonMesh,
    PolyfaceMesh,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolylineVertex {
    pub position: [f64; 3],
    pub bulge: f64,
    pub start_width: f64,
    pub end_width: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LwVertex {
    pub x: f64,
    pub y: f64,
    pub bulge: f64,
    /// Per-vertex starting width (DXF code 40 after 10/20). Native-model
    /// addition (D3 series).
    pub start_width: f64,
    /// Per-vertex ending width (DXF code 41). Native-model addition (D3 series).
    pub end_width: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HatchBoundaryPath {
    pub flags: i32,
    pub edges: Vec<HatchEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HatchEdge {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    CircularArc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        is_ccw: bool,
    },
    EllipticArc {
        center: [f64; 2],
        major_endpoint: [f64; 2],
        minor_ratio: f64,
        start_angle: f64,
        end_angle: f64,
        is_ccw: bool,
    },
    Polyline {
        closed: bool,
        vertices: Vec<[f64; 3]>,
    },
}

// ---------------------------------------------------------------------------
// Objects (non-graphical)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CadObject {
    pub handle: Handle,
    pub owner_handle: Handle,
    pub data: ObjectData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    Dictionary {
        entries: Vec<(String, Handle)>,
    },
    XRecord {
        data_pairs: Vec<(i16, String)>,
    },
    Group {
        description: String,
        entity_handles: Vec<Handle>,
    },
    Layout {
        name: String,
        tab_order: i32,
        block_record_handle: Handle,
        plot_paper_size: [f64; 2],
        plot_origin: [f64; 2],
    },
    DictionaryVar {
        schema: String,
        value: String,
    },
    Scale {
        name: String,
        paper_units: f64,
        drawing_units: f64,
        is_unit_scale: bool,
    },
    VisualStyle {
        description: String,
        style_type: i32,
    },
    Material {
        name: String,
    },
    ImageDef {
        /// DXF code 1: Raster image absolute path.
        file_name: String,
        /// DXF codes 10/20: image size in pixels (U, V).
        image_size: [f64; 2],
        /// DXF codes 11/21: default size of one pixel in AutoCAD
        /// drawing units (U, V). Defaults to [1.0, 1.0] so a freshly
        /// auto-created IMAGEDEF treats 1 pixel = 1 drawing unit.
        pixel_size: [f64; 2],
        /// DXF code 90: class version. Defaults to 0.
        class_version: i32,
        /// DXF code 71: whether the referenced file was loaded at save
        /// time. Defaults to true.
        image_is_loaded: bool,
        /// DXF code 281: resolution unit — 0 = None, 2 = centimeters,
        /// 5 = inches. Defaults to 0 = None.
        resolution_unit: u8,
    },
    ImageDefReactor {
        image_handle: Handle,
    },
    MLineStyle {
        name: String,
        description: String,
        element_count: i16,
    },
    MLeaderStyle {
        name: String,
        content_type: i16,
        text_style_handle: Handle,
    },
    TableStyle {
        name: String,
        description: String,
    },
    SortEntsTable {
        entity_handles: Vec<Handle>,
        sort_handles: Vec<Handle>,
    },
    DimAssoc {
        associativity: i32,
        dimension_handle: Handle,
    },
    PlotSettings {
        page_name: String,
        printer_name: String,
        paper_size: String,
    },
    /// FIELD — parametric text/property field.
    Field {
        /// code 1 — Evaluator id (e.g. "AcDbFormattedTable", "AcVar")
        evaluator_id: String,
        /// code 2 — Field code string
        field_code: String,
    },
    /// IDBUFFER — ordered list of entity handles.
    IdBuffer {
        entity_handles: Vec<Handle>,
    },
    /// LAYER_FILTER — list of layer handles matching a saved filter.
    LayerFilter {
        /// code 1 — Filter name
        name: String,
        /// code 8 — Layer handles referenced by this filter
        layer_handles: Vec<Handle>,
    },
    /// LIGHTLIST — aggregated list of lights.
    LightList {
        /// code 90 — Light count
        count: i32,
        light_handles: Vec<Handle>,
    },
    /// SUNSTUDY — sun study analysis block.
    SunStudy {
        /// code 1 — Sun setup name
        name: String,
        /// code 2 — Description
        description: String,
        /// code 70 — Output type (0=frames, 1=single, 2=range)
        output_type: i16,
    },
    /// DATATABLE — user-defined key/value tabular data.
    DataTable {
        /// code 70 — Flags
        flags: i16,
        /// code 90 — Column count
        column_count: i32,
        /// code 91 — Row count
        row_count: i32,
        /// code 1 — Table name
        name: String,
    },
    /// WIPEOUTVARIABLES — global wipeout frame draw mode.
    WipeoutVariables {
        /// code 70 — Frame setting (0=off, 1=on, 2=on+print)
        frame_mode: i16,
    },
    /// GEODATA — drawing geodata (coordinate system, reference point).
    GeoData {
        /// code 70 — Coordinate type
        coordinate_type: i16,
        /// code 10/20/30 — Reference point (world coords)
        reference_point: [f64; 3],
        /// code 11/21/31 — Reference point (local design coords)
        design_point: [f64; 3],
    },
    /// RENDERENVIRONMENT — background / fog render settings.
    RenderEnvironment {
        /// code 1 — Environment name
        name: String,
        /// code 290 — Fog enabled
        fog_enabled: bool,
        /// code 40 — Fog density near
        fog_density_near: f64,
        /// code 41 — Fog density far
        fog_density_far: f64,
    },
    /// ACAD_PROXY_OBJECT — preserves raw DXF codes so the object can
    /// be written back unchanged.
    ProxyObject {
        /// code 90 — Proxy object class id
        class_id: i32,
        /// code 91 — Application object class id
        application_class_id: i32,
        /// Raw (code, value) tuples captured verbatim
        raw_codes: Vec<(i16, String)>,
    },
    Unknown {
        object_type: String,
    },
}

fn allocate_reserved_handle(next_handle: &mut u64) -> Handle {
    let handle = Handle::new(*next_handle);
    *next_handle += 1;
    handle
}

fn layout_entry_name(handle: Handle) -> String {
    format!("LAYOUT_{:X}", handle.value())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_document_has_required_defaults() {
        let doc = CadDocument::new();

        assert_eq!(doc.header.version, DxfVersion::R2000);
        assert_eq!(doc.model_space_handle(), Handle::new(1));
        assert_eq!(doc.paper_space_handle(), Handle::new(2));
        assert_eq!(doc.root_dictionary.handle, Handle::new(5));
        assert_eq!(doc.next_handle(), 6);
        assert!(doc.block_records.contains_key(&doc.model_space_handle()));
        assert!(doc.block_records.contains_key(&doc.paper_space_handle()));
        assert_eq!(doc.layouts.len(), 2);
    }

    #[test]
    fn minimal_document_seeds_required_root_dictionary_entries() {
        let doc = CadDocument::new();

        assert_eq!(
            doc.root_dictionary.entries.get("ACAD_GROUP"),
            Some(&Handle::NULL)
        );
        assert_eq!(
            doc.root_dictionary.entries.get("ACAD_LAYOUT"),
            Some(&doc.root_dictionary.handle)
        );
        assert_eq!(
            doc.tables.block_record.entries.get("*Model_Space"),
            Some(&doc.model_space_handle())
        );
        assert_eq!(
            doc.tables.block_record.entries.get("*Paper_Space"),
            Some(&doc.paper_space_handle())
        );
        let model_layout = doc
            .block_records
            .get(&doc.model_space_handle())
            .and_then(|record| record.layout_handle)
            .unwrap();
        assert!(doc.layouts.contains_key(&model_layout));
    }

    #[test]
    fn allocated_handles_continue_after_seeded_objects() {
        let mut doc = CadDocument::new();

        let first = doc.allocate_handle();
        let second = doc.allocate_handle();

        assert_eq!(first, Handle::new(6));
        assert_eq!(second, Handle::new(7));
        assert_eq!(doc.next_handle(), 8);
    }

    #[test]
    fn repair_ownership_closes_layout_block_links() {
        let mut doc = CadDocument::new();
        let block = BlockRecord::new(doc.allocate_handle(), "*Paper_Space2");
        let layout = Layout::new(doc.allocate_handle(), "Layout2", block.handle);

        doc.insert_block_record(block);
        doc.insert_layout(layout.clone());
        doc.repair_ownership();

        let repaired_layout = doc.layouts.get(&layout.handle).unwrap();
        let repaired_block = doc.block_records.get(&layout.block_record_handle).unwrap();

        assert_eq!(repaired_layout.owner, doc.root_dictionary.handle);
        assert_eq!(repaired_block.layout_handle, Some(layout.handle));
        assert_eq!(
            doc.root_dictionary
                .entries
                .get(&layout_entry_name(layout.handle)),
            Some(&layout.handle)
        );
    }

    #[test]
    fn clone_snapshot_preserves_entities_and_handles() {
        let mut doc = CadDocument::new();
        let line_handle = doc
            .add_entity(Entity::new(EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("line should be added");

        let snapshot = doc.clone();
        assert_eq!(snapshot.next_handle(), doc.next_handle());
        assert_eq!(snapshot.get_entity(line_handle), doc.get_entity(line_handle));
    }

    #[test]
    fn add_entity_assigns_handle_and_model_owner() {
        let mut doc = CadDocument::new();
        let handle = doc
            .add_entity(Entity::new(EntityData::Circle {
                center: [1.0, 2.0, 0.0],
                radius: 3.0,
            }))
            .expect("circle should be added");

        let entity = doc.get_entity(handle).expect("entity should be queryable");
        assert_ne!(handle, Handle::NULL);
        assert_eq!(entity.owner_handle, doc.model_space_handle());
        assert!(doc.is_model_space_entity(entity));
    }

    #[test]
    fn add_entity_to_layout_routes_to_layout_block() {
        let mut doc = CadDocument::new();
        let handle = doc
            .add_entity_to_layout(
                Entity::new(EntityData::Text {
                    insertion: [0.0, 0.0, 0.0],
                    height: 2.5,
                    value: "Layout note".into(),
                    rotation: 0.0,
                    style_name: "Standard".into(),
                    width_factor: 1.0,
                    oblique_angle: 0.0,
                    horizontal_alignment: 0,
                    vertical_alignment: 0,
                    alignment_point: None,
                }),
                "Layout1",
            )
            .expect("paper-space entity should be added");

        let entity = doc.get_entity(handle).expect("entity should exist");
        let layout = doc.layout_by_name("Layout1").expect("layout should exist");
        assert_eq!(entity.owner_handle, layout.block_record_handle);
        assert!(!doc.is_model_space_entity(entity));
    }

    #[test]
    fn add_entity_with_block_owner_is_stored_inside_block_record() {
        let mut doc = CadDocument::new();
        let block_handle = doc.allocate_handle();
        doc.insert_block_record(BlockRecord::new(block_handle, "MY_BLOCK"));

        let mut entity = Entity::new(EntityData::Point {
            position: [9.0, 9.0, 0.0],
        });
        entity.owner_handle = block_handle;

        let handle = doc.add_entity(entity).expect("block-owned entity should be added");

        assert!(doc.entities.iter().all(|entity| entity.handle != handle));
        let block = doc
            .block_record_by_handle(block_handle)
            .expect("block record should exist");
        assert_eq!(block.entities.len(), 1);
        assert_eq!(block.entities[0].handle, handle);
        assert_eq!(doc.get_entity(handle).expect("entity should be queryable").owner_handle, block_handle);
    }

    #[test]
    fn remove_entity_erases_lookup_entry() {
        let mut doc = CadDocument::new();
        let handle = doc
            .add_entity(Entity::new(EntityData::Arc {
                center: [0.0, 0.0, 0.0],
                radius: 2.0,
                start_angle: 0.0,
                end_angle: 90.0,
            }))
            .expect("arc should be added");

        let removed = doc.remove_entity(handle).expect("entity should be removed");
        assert_eq!(removed.handle, handle);
        assert!(doc.get_entity(handle).is_none());
    }
}
