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

#[derive(Debug, Clone, PartialEq)]
pub struct CadDocument {
    pub header: DocumentHeader,
    pub classes: Vec<DxfClass>,
    pub tables: Tables,
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

        Self {
            header: DocumentHeader::default(),
            classes: Vec::new(),
            tables: Tables::new(model_space_handle, paper_space_handle),
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
}

impl Default for CadDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentHeader {
    pub version: DxfVersion,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRecord {
    pub handle: Handle,
    pub name: String,
    pub layout_handle: Option<Handle>,
}

impl BlockRecord {
    pub fn new(handle: Handle, name: impl Into<String>) -> Self {
        Self {
            handle,
            name: name.into(),
            layout_handle: None,
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
    pub layer_name: String,
    pub linetype_name: String,
    pub color_index: i16,
    pub data: EntityData,
}

impl Entity {
    pub fn new(data: EntityData) -> Self {
        Self {
            handle: Handle::NULL,
            layer_name: "0".into(),
            linetype_name: String::new(),
            color_index: 256, // BYLAYER
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
        fit_points: Vec<[f64; 3]>,
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
        dim_type: i16,
        block_name: String,
        definition_point: [f64; 3],
        text_midpoint: [f64; 3],
        text_override: String,
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
