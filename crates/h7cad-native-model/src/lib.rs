use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug)]
pub struct CadDocument {
    pub header: DocumentHeader,
    pub classes: Vec<DxfClass>,
    pub tables: Tables,
    pub layers: BTreeMap<String, LayerProperties>,
    pub linetypes: BTreeMap<String, LinetypeProperties>,
    pub text_styles: BTreeMap<String, TextStyleProperties>,
    pub dim_styles: BTreeMap<String, DimStyleProperties>,
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

        Self {
            header: DocumentHeader::default(),
            classes: Vec::new(),
            tables: Tables::new(model_space_handle, paper_space_handle),
            layers,
            linetypes,
            text_styles,
            dim_styles,
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
    pub color_index: i16,
    /// True color (code 420) as packed RGB, 0 = not set
    pub true_color: i32,
    /// Line weight in 1/100 mm (code 370), -1=ByLayer, -2=ByBlock, -3=Default
    pub lineweight: i16,
    /// 0=visible, 1=invisible (code 60)
    pub invisible: bool,
    /// Transparency (code 440), 0=fully opaque
    pub transparency: i32,
    pub data: EntityData,
}

impl Entity {
    pub fn new(data: EntityData) -> Self {
        Self {
            handle: Handle::NULL,
            owner_handle: Handle::NULL,
            layer_name: "0".into(),
            linetype_name: String::new(),
            color_index: 256, // BYLAYER
            true_color: 0,
            lineweight: -1, // ByLayer
            invisible: false,
            transparency: 0,
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
    },
    Text {
        insertion: [f64; 3],
        height: f64,
        value: String,
        rotation: f64,
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
    },
    Solid {
        corners: [[f64; 3]; 4],
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
        value: String,
        rotation: f64,
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
    },
    Image {
        insertion: [f64; 3],
        u_vector: [f64; 3],
        v_vector: [f64; 3],
        image_size: [f64; 2],
    },
    Wipeout {
        clip_vertices: Vec<[f64; 2]>,
    },
    Tolerance {
        text: String,
        insertion: [f64; 3],
    },
    Shape {
        insertion: [f64; 3],
        size: f64,
        shape_number: i16,
    },
    Solid3D {
        acis_data: String,
    },
    Region {
        acis_data: String,
    },
    MultiLeader {
        // Simplified — full nested context parsing is TODO
    },
    Table {
        // Simplified — full ACAD_TABLE parsing is TODO
    },
    Mesh {
        vertex_count: i32,
        face_count: i32,
    },
    PdfUnderlay {
        insertion: [f64; 3],
        scale: [f64; 3],
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
        file_name: String,
        image_size: [f64; 2],
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
}
