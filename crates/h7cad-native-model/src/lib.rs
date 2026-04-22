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

    fn store_entity(&mut self, entity: Entity) -> Result<(), String> {
        let owner_handle = entity.owner_handle;
        if owner_handle == Handle::NULL {
            self.entities.push(entity);
            return Ok(());
        }

        let owner_br_handle = self.block_record_by_any_handle(owner_handle).map(|br| br.handle);
        let Some(owner_br_handle) = owner_br_handle else {
            return Err(format!(
                "owner handle {:X} does not resolve to a block record",
                owner_handle.value()
            ));
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
