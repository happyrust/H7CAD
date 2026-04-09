pub mod tokenizer;
pub mod writer;
mod entity_parsers;

use std::fmt;

pub use tokenizer::*;
pub use writer::write_dxf_string;
use h7cad_native_model::CadDocument;

// ---------------------------------------------------------------------------
// DXF Read Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxfReadError {
    Parse(DxfParseError),
    UnexpectedToken {
        expected: String,
        got_code: i16,
        got_value: String,
    },
    UnexpectedEof {
        context: &'static str,
    },
    UnknownSection(String),
    UnsupportedFormat(String),
}

impl From<DxfParseError> for DxfReadError {
    fn from(e: DxfParseError) -> Self {
        Self::Parse(e)
    }
}

impl fmt::Display for DxfReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{e}"),
            Self::UnexpectedToken {
                expected,
                got_code,
                got_value,
            } => write!(
                f,
                "expected {expected}, got ({got_code}, `{got_value}`)"
            ),
            Self::UnexpectedEof { context } => write!(f, "unexpected EOF: {context}"),
            Self::UnknownSection(name) => write!(f, "unknown section `{name}` (skipped)"),
            Self::UnsupportedFormat(msg) => write!(f, "unsupported format: {msg}"),
        }
    }
}

impl std::error::Error for DxfReadError {}

// ---------------------------------------------------------------------------
// Section Names
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxfSectionName {
    Header,
    Classes,
    Tables,
    Blocks,
    Entities,
    Objects,
}

impl DxfSectionName {
    pub fn from_dxf(s: &str) -> Option<Self> {
        match s.trim() {
            "HEADER" => Some(Self::Header),
            "CLASSES" => Some(Self::Classes),
            "TABLES" => Some(Self::Tables),
            "BLOCKS" => Some(Self::Blocks),
            "ENTITIES" => Some(Self::Entities),
            "OBJECTS" => Some(Self::Objects),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Stream Reader — wraps DxfTokenizer with peek / current semantics
// ---------------------------------------------------------------------------

pub struct DxfStreamReader<'a> {
    tokenizer: DxfTokenizer<'a>,
    current: Option<DxfToken>,
}

impl<'a> DxfStreamReader<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            tokenizer: DxfTokenizer::new(input),
            current: None,
        }
    }

    pub fn read_next(&mut self) -> Result<bool, DxfReadError> {
        match self.tokenizer.next() {
            Some(Ok(token)) => {
                self.current = Some(token);
                Ok(true)
            }
            Some(Err(e)) => Err(DxfReadError::Parse(e)),
            None => {
                self.current = None;
                Ok(false)
            }
        }
    }

    pub fn current(&self) -> Option<&DxfToken> {
        self.current.as_ref()
    }

    pub fn current_code(&self) -> i16 {
        self.current.as_ref().map_or(-1, |t| t.code.value())
    }

    pub fn current_value_trimmed(&self) -> &str {
        self.current
            .as_ref()
            .map(|t| t.raw_value.trim())
            .unwrap_or("")
    }

    pub fn find(&mut self, code: i16, value: &str) -> Result<bool, DxfReadError> {
        loop {
            if !self.read_next()? {
                return Ok(false);
            }
            if self.current_code() == code && self.current_value_trimmed() == value {
                return Ok(true);
            }
        }
    }

    pub fn skip_section(&mut self) -> Result<(), DxfReadError> {
        loop {
            if !self.read_next()? {
                return Err(DxfReadError::UnexpectedEof {
                    context: "expected ENDSEC",
                });
            }
            if self.current_code() == 0 && self.current_value_trimmed() == "ENDSEC" {
                return Ok(());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Section readers
// ---------------------------------------------------------------------------

fn read_header_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    use h7cad_native_model::DxfVersion;

    if !stream.read_next()? {
        return Err(DxfReadError::UnexpectedEof {
            context: "expected ENDSEC for HEADER section",
        });
    }

    loop {
        if stream.current().is_none() {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for HEADER section",
            });
        }

        if stream.current_code() == 0 && stream.current_value_trimmed() == "ENDSEC" {
            return Ok(());
        }

        if stream.current_code() != 9 {
            if !stream.read_next()? {
                return Err(DxfReadError::UnexpectedEof {
                    context: "expected ENDSEC for HEADER section",
                });
            }
            continue;
        }

        let var_name = stream.current_value_trimmed().to_string();

        let mut codes: Vec<(i16, String)> = Vec::new();
        while stream.read_next()? {
            if stream.current_code() == 0 || stream.current_code() == 9 {
                break;
            }
            codes.push((
                stream.current_code(),
                stream.current_value_trimmed().to_string(),
            ));
        }

        let f = |c: i16| -> f64 {
            codes
                .iter()
                .find(|(code, _)| *code == c)
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0.0)
        };
        let i16v = |c: i16| -> i16 {
            codes
                .iter()
                .find(|(code, _)| *code == c)
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0)
        };
        let sv = |c: i16| -> &str {
            codes
                .iter()
                .find(|(code, _)| *code == c)
                .map(|(_, v)| v.as_str())
                .unwrap_or("")
        };

        match var_name.as_str() {
            "$ACADVER" => doc.header.version = DxfVersion::from_acadver(sv(1)),
            "$INSBASE" => doc.header.insbase = [f(10), f(20), f(30)],
            "$EXTMIN" => doc.header.extmin = [f(10), f(20), f(30)],
            "$EXTMAX" => doc.header.extmax = [f(10), f(20), f(30)],
            "$LIMMIN" => doc.header.limmin = [f(10), f(20)],
            "$LIMMAX" => doc.header.limmax = [f(10), f(20)],
            "$LTSCALE" => doc.header.ltscale = f(40),
            "$PDMODE" => doc.header.pdmode = i16v(70) as i32,
            "$PDSIZE" => doc.header.pdsize = f(40),
            "$TEXTSIZE" => doc.header.textsize = f(40),
            "$DIMSCALE" => doc.header.dimscale = f(40),
            "$LUNITS" => doc.header.lunits = i16v(70),
            "$LUPREC" => doc.header.luprec = i16v(70),
            "$AUNITS" => doc.header.aunits = i16v(70),
            "$AUPREC" => doc.header.auprec = i16v(70),
            "$HANDSEED" => {
                doc.header.handseed = u64::from_str_radix(sv(5), 16).unwrap_or(0);
            }
            _ => {}
        }
    }
}

fn read_classes_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    use h7cad_native_model::DxfClass;

    while stream.read_next()? {
        if stream.current_code() == 0 {
            match stream.current_value_trimmed() {
                "ENDSEC" => return Ok(()),
                "CLASS" => {
                    let mut cls = DxfClass::new();
                    let mut class_number: i16 = 0;
                    while stream.read_next()? {
                        if stream.current_code() == 0 {
                            if class_number < 500 {
                                class_number = 500 + doc.classes.len() as i16;
                            }
                            let _ = class_number;
                            doc.classes.push(cls);
                            match stream.current_value_trimmed() {
                                "ENDSEC" => return Ok(()),
                                "CLASS" => {
                                    cls = DxfClass::new();
                                    class_number = 0;
                                    continue;
                                }
                                _ => break,
                            }
                        }
                        match stream.current_code() {
                            1 => cls.dxf_name = stream.current_value_trimmed().to_string(),
                            2 => cls.cpp_class_name = stream.current_value_trimmed().to_string(),
                            3 => cls.application_name = stream.current_value_trimmed().to_string(),
                            90 => {
                                cls.proxy_flags = stream
                                    .current_value_trimmed()
                                    .parse()
                                    .unwrap_or(0);
                            }
                            91 => {
                                cls.instance_count = stream
                                    .current_value_trimmed()
                                    .parse()
                                    .unwrap_or(0);
                            }
                            280 => cls.was_a_proxy = stream.current_value_trimmed() == "1",
                            281 => cls.is_an_entity = stream.current_value_trimmed() == "1",
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Err(DxfReadError::UnexpectedEof {
        context: "expected ENDSEC for CLASSES section",
    })
}

fn read_tables_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    while stream.read_next()? {
        if stream.current_code() == 0 {
            match stream.current_value_trimmed() {
                "ENDSEC" => return Ok(()),
                "TABLE" => read_single_table(stream, doc)?,
                _ => {}
            }
        }
    }
    Err(DxfReadError::UnexpectedEof {
        context: "expected ENDSEC for TABLES section",
    })
}

fn read_single_table(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    use h7cad_native_model::{
        DimStyleProperties, Handle, LayerProperties, LinetypeProperties, LinetypeSegment,
        TextStyleProperties, VPortProperties,
    };

    if !stream.read_next()? {
        return Err(DxfReadError::UnexpectedEof {
            context: "expected table name after TABLE",
        });
    }
    let table_name = stream.current_value_trimmed().to_string();

    while stream.read_next()? {
        if stream.current_code() == 0 {
            break;
        }
    }

    let needs_detail = matches!(
        table_name.as_str(),
        "LAYER" | "LTYPE" | "STYLE" | "DIMSTYLE" | "VPORT"
    );

    loop {
        let entry_type = stream.current_value_trimmed().to_string();
        if entry_type == "ENDTAB" {
            return Ok(());
        }

        let mut entry_handle = Handle::NULL;
        let mut entry_name = String::new();
        let mut codes: Vec<(i16, String)> = Vec::new();

        while stream.read_next()? {
            if stream.current_code() == 0 {
                break;
            }
            let code = stream.current_code();
            let val = stream.current_value_trimmed().to_string();
            match code {
                5 | 105 => {
                    entry_handle = Handle::new(
                        u64::from_str_radix(&val, 16).unwrap_or(0),
                    );
                }
                2 => entry_name = val.clone(),
                _ => {}
            }
            if needs_detail {
                codes.push((code, val));
            }
        }

        if !entry_name.is_empty() {
            match table_name.as_str() {
                "LAYER" => {
                    let mut layer = LayerProperties::new(&entry_name);
                    layer.handle = entry_handle;
                    for &(code, ref val) in &codes {
                        match code {
                            62 => layer.color = val.parse().unwrap_or(7),
                            6 => layer.linetype_name = val.clone(),
                            70 => {
                                let flags: i16 = val.parse().unwrap_or(0);
                                layer.is_frozen = flags & 1 != 0;
                                layer.is_locked = flags & 4 != 0;
                            }
                            290 => layer.plot = val.parse::<i16>().unwrap_or(1) != 0,
                            370 => layer.lineweight = val.parse().unwrap_or(-1),
                            420 => layer.true_color = val.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                    doc.layers.insert(entry_name.clone(), layer);
                }
                "LTYPE" => {
                    let mut lt = LinetypeProperties::new(&entry_name);
                    lt.handle = entry_handle;
                    for &(code, ref val) in &codes {
                        match code {
                            3 => lt.description = val.clone(),
                            40 => lt.pattern_length = val.parse().unwrap_or(0.0),
                            49 => {
                                lt.segments.push(LinetypeSegment {
                                    length: val.parse().unwrap_or(0.0),
                                });
                            }
                            _ => {}
                        }
                    }
                    doc.linetypes.insert(entry_name.clone(), lt);
                }
                "STYLE" => {
                    let mut ts = TextStyleProperties::new(&entry_name);
                    ts.handle = entry_handle;
                    for &(code, ref val) in &codes {
                        match code {
                            40 => ts.height = val.parse().unwrap_or(0.0),
                            41 => ts.width_factor = val.parse().unwrap_or(1.0),
                            50 => ts.oblique_angle = val.parse().unwrap_or(0.0),
                            70 => ts.flags = val.parse().unwrap_or(0),
                            3 => ts.font_name = val.clone(),
                            4 => ts.bigfont_name = val.clone(),
                            _ => {}
                        }
                    }
                    doc.text_styles.insert(entry_name.clone(), ts);
                }
                "DIMSTYLE" => {
                    let mut ds = DimStyleProperties::new(&entry_name);
                    ds.handle = entry_handle;
                    for &(code, ref val) in &codes {
                        match code {
                            40 => ds.dimscale = val.parse().unwrap_or(1.0),
                            41 => ds.dimasz = val.parse().unwrap_or(2.5),
                            42 => ds.dimexo = val.parse().unwrap_or(0.625),
                            44 => ds.dimgap = val.parse().unwrap_or(0.625),
                            140 => ds.dimtxt = val.parse().unwrap_or(2.5),
                            271 => ds.dimdec = val.parse().unwrap_or(4),
                            277 => ds.dimlunit = val.parse().unwrap_or(2),
                            275 => ds.dimaunit = val.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                    doc.dim_styles.insert(entry_name.clone(), ds);
                }
                "VPORT" => {
                    let mut vp = VPortProperties::new(&entry_name);
                    vp.handle = entry_handle;
                    for &(code, ref val) in &codes {
                        match code {
                            10 => vp.lower_left[0] = val.parse().unwrap_or(0.0),
                            20 => vp.lower_left[1] = val.parse().unwrap_or(0.0),
                            11 => vp.upper_right[0] = val.parse().unwrap_or(1.0),
                            21 => vp.upper_right[1] = val.parse().unwrap_or(1.0),
                            12 => vp.view_center[0] = val.parse().unwrap_or(0.0),
                            22 => vp.view_center[1] = val.parse().unwrap_or(0.0),
                            40 => vp.view_height = val.parse().unwrap_or(1.0),
                            41 => vp.aspect_ratio = val.parse().unwrap_or(1.0),
                            16 => vp.view_direction[0] = val.parse().unwrap_or(0.0),
                            26 => vp.view_direction[1] = val.parse().unwrap_or(0.0),
                            36 => vp.view_direction[2] = val.parse().unwrap_or(1.0),
                            17 => vp.view_target[0] = val.parse().unwrap_or(0.0),
                            27 => vp.view_target[1] = val.parse().unwrap_or(0.0),
                            37 => vp.view_target[2] = val.parse().unwrap_or(0.0),
                            _ => {}
                        }
                    }
                    doc.vports.insert(entry_name.clone(), vp);
                }
                _ => {}
            }

            let table = match table_name.as_str() {
                "LAYER" => Some(&mut doc.tables.layer),
                "LTYPE" => Some(&mut doc.tables.linetype),
                "STYLE" => Some(&mut doc.tables.style),
                "VIEW" => Some(&mut doc.tables.view),
                "UCS" => Some(&mut doc.tables.ucs),
                "APPID" => Some(&mut doc.tables.appid),
                "DIMSTYLE" => Some(&mut doc.tables.dimstyle),
                "BLOCK_RECORD" => Some(&mut doc.tables.block_record),
                "VPORT" => None,
                _ => None,
            };
            if let Some(tbl) = table {
                tbl.insert(entry_name, entry_handle);
            }
        }

        if stream.current().is_none() {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDTAB",
            });
        }
    }
}

fn read_blocks_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    use h7cad_native_model::{BlockRecord, Handle};

    loop {
        if !stream.read_next()? {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for BLOCKS section",
            });
        }
        if stream.current_code() == 0 {
            break;
        }
    }

    loop {
        match stream.current_value_trimmed() {
            "ENDSEC" => return Ok(()),
            "BLOCK" => {
                let mut block_entity_handle = Handle::NULL;
                let mut owner_handle = Handle::NULL;
                let mut name = String::new();
                let mut base_point = [0.0f64; 3];

                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                    match stream.current_code() {
                        5 => {
                            block_entity_handle = Handle::new(
                                u64::from_str_radix(stream.current_value_trimmed(), 16)
                                    .unwrap_or(0),
                            );
                        }
                        330 => {
                            owner_handle = Handle::new(
                                u64::from_str_radix(stream.current_value_trimmed(), 16)
                                    .unwrap_or(0),
                            );
                        }
                        2 | 3 => {
                            if name.is_empty() {
                                name = stream.current_value_trimmed().to_string();
                            }
                        }
                        10 => base_point[0] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        20 => base_point[1] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        30 => base_point[2] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        _ => {}
                    }
                }

                let mut block_entities = Vec::new();

                loop {
                    if stream.current().is_none() {
                        return Err(DxfReadError::UnexpectedEof {
                            context: "expected ENDBLK",
                        });
                    }
                    let entity_type = stream.current_value_trimmed().to_string();
                    if entity_type == "ENDBLK" {
                        while stream.read_next()? {
                            if stream.current_code() == 0 {
                                break;
                            }
                        }
                        break;
                    }
                    if let Some(ent) = read_entity(stream, &entity_type)? {
                        block_entities.push(ent);
                    }
                }

                if !name.is_empty() {
                    let record_handle = if owner_handle != Handle::NULL {
                        owner_handle
                    } else {
                        block_entity_handle
                    };

                    if let Some(existing) = doc.block_records.get_mut(&record_handle) {
                        existing.block_entity_handle = block_entity_handle;
                        existing.base_point = base_point;
                        existing.entities = block_entities;
                    } else {
                        let mut record = BlockRecord::new(record_handle, &name);
                        record.block_entity_handle = block_entity_handle;
                        record.base_point = base_point;
                        record.entities = block_entities;
                        doc.insert_block_record(record);
                    }
                }
            }
            _ => {
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                }
            }
        }

        if stream.current().is_none() {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for BLOCKS section",
            });
        }
    }
}

fn read_entities_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    loop {
        if !stream.read_next()? {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for ENTITIES section",
            });
        }
        if stream.current_code() == 0 {
            break;
        }
    }

    loop {
        let type_name = stream.current_value_trimmed().to_string();
        if type_name == "ENDSEC" {
            return Ok(());
        }

        if let Some(entity) = read_entity(stream, &type_name)? {
            doc.entities.push(entity);
        }

        if stream.current().is_none() {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for ENTITIES section",
            });
        }
    }
}

fn read_entity(
    stream: &mut DxfStreamReader<'_>,
    type_name: &str,
) -> Result<Option<h7cad_native_model::Entity>, DxfReadError> {
    use h7cad_native_model::{Entity, EntityData, Handle};
    use entity_parsers::*;

    let mut entity = Entity::new(EntityData::Unknown {
        entity_type: type_name.to_string(),
    });

    let mut codes: Vec<(i16, String)> = Vec::new();
    while stream.read_next()? {
        if stream.current_code() == 0 {
            break;
        }
        codes.push((
            stream.current_code(),
            stream.current_value_trimmed().to_string(),
        ));
    }

    let mut xdata_app: Option<String> = None;
    let mut xdata_pairs: Vec<(i16, String)> = Vec::new();

    for &(code, ref val) in &codes {
        if code >= 1000 {
            if code == 1001 {
                if let Some(app) = xdata_app.take() {
                    entity.xdata.push((app, std::mem::take(&mut xdata_pairs)));
                }
                xdata_app = Some(val.clone());
            } else if xdata_app.is_some() {
                xdata_pairs.push((code, val.clone()));
            }
            continue;
        }
        match code {
            5 => {
                entity.handle =
                    Handle::new(u64::from_str_radix(val, 16).unwrap_or(0));
            }
            330 => {
                entity.owner_handle =
                    Handle::new(u64::from_str_radix(val, 16).unwrap_or(0));
            }
            8 => entity.layer_name = val.clone(),
            6 => entity.linetype_name = val.clone(),
            62 => entity.color_index = val.parse().unwrap_or(256),
            420 => entity.true_color = val.parse().unwrap_or(0),
            370 => entity.lineweight = val.parse().unwrap_or(-1),
            60 => entity.invisible = val.parse::<i16>().unwrap_or(0) != 0,
            440 => entity.transparency = val.parse().unwrap_or(0),
            39 => entity.thickness = val.parse().unwrap_or(0.0),
            210 => entity.extrusion[0] = val.parse().unwrap_or(0.0),
            220 => entity.extrusion[1] = val.parse().unwrap_or(0.0),
            230 => entity.extrusion[2] = val.parse().unwrap_or(1.0),
            _ => {}
        }
    }
    if let Some(app) = xdata_app.take() {
        entity.xdata.push((app, xdata_pairs));
    }

    entity.data = match type_name {
        "LINE" => parse_line(&codes),
        "CIRCLE" => parse_circle(&codes),
        "ARC" => parse_arc(&codes),
        "POINT" => parse_point(&codes),
        "LWPOLYLINE" => parse_lwpolyline(&codes),
        "TEXT" => parse_text(&codes),
        "ELLIPSE" => parse_ellipse(&codes),
        "SPLINE" => parse_spline(&codes),
        "3DFACE" => parse_3dface(&codes),
        "SOLID" | "TRACE" => parse_solid(&codes),
        "RAY" => parse_ray_xline(&codes, true),
        "XLINE" => parse_ray_xline(&codes, false),
        "MTEXT" => parse_mtext(&codes),
        "INSERT" => {
            let (data, has_attribs) = parse_insert(&codes);
            if has_attribs {
                entity.data = data;
                return read_insert_attrib_sequence(stream, entity);
            }
            data
        }
        "DIMENSION" => parse_dimension(&codes),
        "HATCH" => parse_hatch(&codes),
        "VIEWPORT" => parse_viewport(&codes),
        "ATTRIB" => parse_attrib(&codes),
        "ATTDEF" => parse_attdef(&codes),
        "LEADER" => parse_leader(&codes),
        "MLINE" => parse_mline(&codes),
        "IMAGE" => parse_image(&codes),
        "WIPEOUT" => parse_wipeout(&codes),
        "TOLERANCE" => parse_tolerance(&codes),
        "SHAPE" => parse_shape(&codes),
        "3DSOLID" | "BODY" => parse_solid3d(&codes),
        "REGION" => parse_region(&codes),
        "MULTILEADER" => parse_multileader(&codes),
        "ACAD_TABLE" => parse_acad_table(&codes),
        "MESH" => parse_mesh(&codes),
        "PDFUNDERLAY" | "DWFUNDERLAY" | "DGNUNDERLAY" => parse_underlay(&codes),
        "SEQEND" => {
            return Ok(None);
        }
        "POLYLINE" => {
            return read_polyline_sequence(stream, entity, &codes);
        }
        _ => EntityData::Unknown {
            entity_type: type_name.to_string(),
        },
    };

    Ok(Some(entity))
}

fn read_polyline_sequence(
    stream: &mut DxfStreamReader<'_>,
    mut entity: h7cad_native_model::Entity,
    header_codes: &[(i16, String)],
) -> Result<Option<h7cad_native_model::Entity>, DxfReadError> {
    use h7cad_native_model::{PolylineType, PolylineVertex};

    let mut flags: i16 = 0;
    for &(code, ref val) in header_codes {
        if code == 70 {
            flags = val.parse().unwrap_or(0);
        }
    }

    let polyline_type = if flags & 16 != 0 {
        PolylineType::PolygonMesh
    } else if flags & 64 != 0 {
        PolylineType::PolyfaceMesh
    } else if flags & 8 != 0 {
        PolylineType::Polyline3D
    } else {
        PolylineType::Polyline2D
    };
    let closed = flags & 1 != 0;

    let mut vertices = Vec::new();

    loop {
        if stream.current().is_none() {
            break;
        }
        let entry_type = stream.current_value_trimmed().to_string();
        match entry_type.as_str() {
            "VERTEX" => {
                let mut pos = [0.0f64; 3];
                let mut bulge = 0.0;
                let mut sw = 0.0;
                let mut ew = 0.0;
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                    match stream.current_code() {
                        10 => pos[0] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        20 => pos[1] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        30 => pos[2] = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        42 => bulge = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        40 => sw = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        41 => ew = stream.current_value_trimmed().parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
                vertices.push(PolylineVertex {
                    position: pos,
                    bulge,
                    start_width: sw,
                    end_width: ew,
                });
            }
            "SEQEND" => {
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                }
                break;
            }
            _ => {
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                }
            }
        }
    }

    entity.data = h7cad_native_model::EntityData::Polyline {
        polyline_type,
        vertices,
        closed,
    };
    Ok(Some(entity))
}

fn read_insert_attrib_sequence(
    stream: &mut DxfStreamReader<'_>,
    mut entity: h7cad_native_model::Entity,
) -> Result<Option<h7cad_native_model::Entity>, DxfReadError> {
    use h7cad_native_model::{Entity, EntityData, Handle};
    use entity_parsers::*;

    let mut attribs = Vec::new();

    loop {
        if stream.current().is_none() {
            break;
        }
        let entry_type = stream.current_value_trimmed().to_string();
        match entry_type.as_str() {
            "ATTRIB" => {
                let mut attr = Entity::new(EntityData::Unknown {
                    entity_type: "ATTRIB".to_string(),
                });
                let mut codes: Vec<(i16, String)> = Vec::new();
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                    codes.push((
                        stream.current_code(),
                        stream.current_value_trimmed().to_string(),
                    ));
                }
                for &(code, ref val) in &codes {
                    match code {
                        5 => attr.handle = Handle::new(u64::from_str_radix(val, 16).unwrap_or(0)),
                        8 => attr.layer_name = val.clone(),
                        6 => attr.linetype_name = val.clone(),
                        62 => attr.color_index = val.parse().unwrap_or(256),
                        _ => {}
                    }
                }
                attr.data = parse_attrib(&codes);
                attribs.push(attr);
            }
            "SEQEND" => {
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                }
                break;
            }
            _ => {
                while stream.read_next()? {
                    if stream.current_code() == 0 {
                        break;
                    }
                }
                break;
            }
        }
    }

    if let EntityData::Insert { attribs: ref mut existing, .. } = entity.data {
        *existing = attribs;
    }
    Ok(Some(entity))
}

fn read_objects_section(
    stream: &mut DxfStreamReader<'_>,
    doc: &mut CadDocument,
) -> Result<(), DxfReadError> {
    use h7cad_native_model::{CadObject, Handle, ObjectData};

    loop {
        if !stream.read_next()? {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for OBJECTS section",
            });
        }
        if stream.current_code() == 0 {
            break;
        }
    }

    loop {
        let type_name = stream.current_value_trimmed().to_string();
        if type_name == "ENDSEC" {
            return Ok(());
        }

        let mut handle = Handle::NULL;
        let mut owner_handle = Handle::NULL;
        let mut codes: Vec<(i16, String)> = Vec::new();

        while stream.read_next()? {
            if stream.current_code() == 0 {
                break;
            }
            let code = stream.current_code();
            let val = stream.current_value_trimmed().to_string();
            match code {
                5 => {
                    handle =
                        Handle::new(u64::from_str_radix(&val, 16).unwrap_or(0));
                }
                330 => {
                    owner_handle =
                        Handle::new(u64::from_str_radix(&val, 16).unwrap_or(0));
                }
                _ => {}
            }
            codes.push((code, val));
        }

        let data = match type_name.as_str() {
            "DICTIONARY" | "ACDBDICTIONARYWDFLT" => {
                let mut entries = Vec::new();
                let mut current_key = String::new();
                for &(code, ref val) in &codes {
                    match code {
                        3 => current_key = val.clone(),
                        350 | 360 => {
                            let h = Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            );
                            entries.push((std::mem::take(&mut current_key), h));
                        }
                        _ => {}
                    }
                }
                ObjectData::Dictionary { entries }
            }
            "XRECORD" => {
                let data_pairs = codes
                    .iter()
                    .filter(|&&(c, _)| c != 5 && c != 330 && c != 100 && c != 102)
                    .cloned()
                    .collect();
                ObjectData::XRecord { data_pairs }
            }
            "GROUP" => {
                let mut description = String::new();
                let mut entity_handles = Vec::new();
                for &(code, ref val) in &codes {
                    match code {
                        300 => description = val.clone(),
                        340 => {
                            entity_handles.push(Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            ));
                        }
                        _ => {}
                    }
                }
                ObjectData::Group {
                    description,
                    entity_handles,
                }
            }
            "LAYOUT" => {
                let mut name = String::new();
                let mut tab_order: i32 = 0;
                let mut block_record_handle = Handle::NULL;
                let (mut pw, mut ph) = (0.0, 0.0);
                let (mut ox, mut oy) = (0.0, 0.0);
                for &(code, ref val) in &codes {
                    match code {
                        1 => name = val.clone(),
                        71 => tab_order = val.parse().unwrap_or(0),
                        330 => {} // already parsed as owner
                        340 => {
                            block_record_handle = Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            );
                        }
                        44 => pw = val.parse().unwrap_or(0.0),
                        45 => ph = val.parse().unwrap_or(0.0),
                        46 => ox = val.parse().unwrap_or(0.0),
                        47 => oy = val.parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
                ObjectData::Layout {
                    name,
                    tab_order,
                    block_record_handle,
                    plot_paper_size: [pw, ph],
                    plot_origin: [ox, oy],
                }
            }
            "PLOTSETTINGS" => {
                let mut page_name = String::new();
                let mut printer_name = String::new();
                let mut paper_size = String::new();
                for &(code, ref val) in &codes {
                    match code {
                        1 => page_name = val.clone(),
                        2 => printer_name = val.clone(),
                        4 => paper_size = val.clone(),
                        _ => {}
                    }
                }
                ObjectData::PlotSettings {
                    page_name,
                    printer_name,
                    paper_size,
                }
            }
            "DICTIONARYVAR" => {
                let mut schema = String::new();
                let mut value = String::new();
                for &(code, ref val) in &codes {
                    match code {
                        280 => schema = val.clone(),
                        1 => value = val.clone(),
                        _ => {}
                    }
                }
                ObjectData::DictionaryVar { schema, value }
            }
            "SCALE" => {
                let mut name = String::new();
                let mut paper_units: f64 = 1.0;
                let mut drawing_units: f64 = 1.0;
                let mut is_unit_scale = false;
                for &(code, ref val) in &codes {
                    match code {
                        300 => name = val.clone(),
                        140 => paper_units = val.parse().unwrap_or(1.0),
                        141 => drawing_units = val.parse().unwrap_or(1.0),
                        290 => is_unit_scale = val.parse::<i16>().unwrap_or(0) != 0,
                        _ => {}
                    }
                }
                ObjectData::Scale {
                    name,
                    paper_units,
                    drawing_units,
                    is_unit_scale,
                }
            }
            "VISUALSTYLE" => {
                let mut description = String::new();
                let mut style_type: i32 = 0;
                for &(code, ref val) in &codes {
                    match code {
                        2 => description = val.clone(),
                        70 => style_type = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
                ObjectData::VisualStyle {
                    description,
                    style_type,
                }
            }
            "MATERIAL" => {
                let mut name = String::new();
                for &(code, ref val) in &codes {
                    if code == 1 {
                        name = val.clone();
                    }
                }
                ObjectData::Material { name }
            }
            "IMAGEDEF" => {
                let mut file_name = String::new();
                let (mut w, mut h) = (0.0, 0.0);
                for &(code, ref val) in &codes {
                    match code {
                        1 => file_name = val.clone(),
                        10 => w = val.parse().unwrap_or(0.0),
                        20 => h = val.parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
                ObjectData::ImageDef {
                    file_name,
                    image_size: [w, h],
                }
            }
            "IMAGEDEF_REACTOR" => {
                let mut image_handle = Handle::NULL;
                for &(code, ref val) in &codes {
                    if code == 330 {
                        image_handle = Handle::new(
                            u64::from_str_radix(val, 16).unwrap_or(0),
                        );
                    }
                }
                ObjectData::ImageDefReactor { image_handle }
            }
            "MLINESTYLE" => {
                let mut name = String::new();
                let mut description = String::new();
                let mut element_count: i16 = 0;
                for &(code, ref val) in &codes {
                    match code {
                        2 => name = val.clone(),
                        3 => description = val.clone(),
                        71 => element_count = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
                ObjectData::MLineStyle {
                    name,
                    description,
                    element_count,
                }
            }
            "MLEADERSTYLE" => {
                let mut name = String::new();
                let mut content_type: i16 = 0;
                let mut text_style_handle = Handle::NULL;
                for &(code, ref val) in &codes {
                    match code {
                        // MLEADERSTYLE uses code 3 for name in some versions
                        3 => name = val.clone(),
                        170 => content_type = val.parse().unwrap_or(0),
                        341 => {
                            text_style_handle = Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            );
                        }
                        _ => {}
                    }
                }
                ObjectData::MLeaderStyle {
                    name,
                    content_type,
                    text_style_handle,
                }
            }
            "TABLESTYLE" => {
                let mut name = String::new();
                let mut description = String::new();
                for &(code, ref val) in &codes {
                    match code {
                        3 => name = val.clone(),
                        300 => description = val.clone(),
                        _ => {}
                    }
                }
                ObjectData::TableStyle { name, description }
            }
            "SORTENTSTABLE" => {
                let mut entity_handles = Vec::new();
                let sort_handles = Vec::new();
                for &(code, ref val) in &codes {
                    match code {
                        331 => {
                            entity_handles.push(Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            ));
                        }
                        5 => {} // already parsed
                        _ => {
                            if (code == 330) && !sort_handles.is_empty() {
                                // additional owner refs after first
                            }
                        }
                    }
                }
                ObjectData::SortEntsTable {
                    entity_handles,
                    sort_handles,
                }
            }
            "DIMASSOC" => {
                let mut associativity: i32 = 0;
                let mut dimension_handle = Handle::NULL;
                for &(code, ref val) in &codes {
                    match code {
                        1 => associativity = val.parse().unwrap_or(0),
                        330 => {
                            dimension_handle = Handle::new(
                                u64::from_str_radix(val, 16).unwrap_or(0),
                            );
                        }
                        _ => {}
                    }
                }
                ObjectData::DimAssoc {
                    associativity,
                    dimension_handle,
                }
            }
            _ => ObjectData::Unknown {
                object_type: type_name.clone(),
            },
        };

        doc.objects.push(CadObject {
            handle,
            owner_handle,
            data,
        });

        if stream.current().is_none() {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected ENDSEC for OBJECTS section",
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Read a DXF file from a byte slice (auto-detects text vs binary, handles encoding).
pub fn read_dxf_bytes(input: &[u8]) -> Result<CadDocument, DxfReadError> {
    if is_binary_dxf(input) {
        return read_binary_dxf(input);
    }
    if let Ok(text) = std::str::from_utf8(input) {
        return read_dxf(text);
    }
    let codepage = detect_codepage(input);
    let encoding = codepage_to_encoding(codepage.as_deref());
    let (decoded, _, _) = encoding.decode(input);
    read_dxf(&decoded)
}

fn detect_codepage(data: &[u8]) -> Option<String> {
    let haystack = if data.len() > 4096 { &data[..4096] } else { data };
    let lossy = String::from_utf8_lossy(haystack);
    for line in lossy.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ANSI_") || trimmed.starts_with("ansi_") {
            return Some(trimmed.to_uppercase());
        }
    }
    None
}

fn codepage_to_encoding(codepage: Option<&str>) -> &'static encoding_rs::Encoding {
    match codepage {
        Some("ANSI_936") => encoding_rs::GBK,
        Some("ANSI_950") => encoding_rs::BIG5,
        Some("ANSI_932") => encoding_rs::SHIFT_JIS,
        Some("ANSI_949") => encoding_rs::EUC_KR,
        Some("ANSI_874") => encoding_rs::WINDOWS_874,
        Some("ANSI_1250") => encoding_rs::WINDOWS_1250,
        Some("ANSI_1251") => encoding_rs::WINDOWS_1251,
        Some("ANSI_1252") => encoding_rs::WINDOWS_1252,
        Some("ANSI_1253") => encoding_rs::WINDOWS_1253,
        Some("ANSI_1254") => encoding_rs::WINDOWS_1254,
        Some("ANSI_1255") => encoding_rs::WINDOWS_1255,
        Some("ANSI_1256") => encoding_rs::WINDOWS_1256,
        Some("ANSI_1257") => encoding_rs::WINDOWS_1257,
        Some("ANSI_1258") => encoding_rs::WINDOWS_1258,
        _ => encoding_rs::WINDOWS_1252,
    }
}

/// Read a binary DXF file by converting tokens to text-equivalent representation.
fn read_binary_dxf(input: &[u8]) -> Result<CadDocument, DxfReadError> {
    let tokenizer = BinaryDxfTokenizer::new(input)?;
    let mut lines = Vec::new();
    for token_result in tokenizer {
        let token = token_result?;
        lines.push(format!("{:>3}", token.code.value()));
        lines.push(token.raw_value);
    }
    let text = lines.join("\n");
    read_dxf(&text)
}

pub fn read_dxf(input: &str) -> Result<CadDocument, DxfReadError> {
    let mut stream = DxfStreamReader::new(input);
    let mut doc = CadDocument::new();

    while stream.find(0, "SECTION")? {
        if !stream.read_next()? {
            return Err(DxfReadError::UnexpectedEof {
                context: "expected section name after SECTION",
            });
        }
        if stream.current_code() != 2 {
            return Err(DxfReadError::UnexpectedToken {
                expected: "group code 2 (section name)".into(),
                got_code: stream.current_code(),
                got_value: stream.current_value_trimmed().into(),
            });
        }

        let name = stream.current_value_trimmed().to_string();

        match DxfSectionName::from_dxf(&name) {
            Some(DxfSectionName::Header) => read_header_section(&mut stream, &mut doc)?,
            Some(DxfSectionName::Classes) => read_classes_section(&mut stream, &mut doc)?,
            Some(DxfSectionName::Tables) => read_tables_section(&mut stream, &mut doc)?,
            Some(DxfSectionName::Blocks) => read_blocks_section(&mut stream, &mut doc)?,
            Some(DxfSectionName::Entities) => read_entities_section(&mut stream, &mut doc)?,
            Some(DxfSectionName::Objects) => read_objects_section(&mut stream, &mut doc)?,
            None => stream.skip_section()?,
        }
    }

    post_process(&mut doc);
    Ok(doc)
}

fn post_process(doc: &mut CadDocument) {
    use h7cad_native_model::Handle;

    let mut max_handle: u64 = 0;

    for entity in &doc.entities {
        max_handle = max_handle.max(entity.handle.value());
    }
    for obj in &doc.objects {
        max_handle = max_handle.max(obj.handle.value());
    }
    for (_, br) in &doc.block_records {
        max_handle = max_handle.max(br.handle.value());
        max_handle = max_handle.max(br.block_entity_handle.value());
        for ent in &br.entities {
            max_handle = max_handle.max(ent.handle.value());
        }
    }
    for (_, layer) in &doc.layers {
        max_handle = max_handle.max(layer.handle.value());
    }
    for (_, lt) in &doc.linetypes {
        max_handle = max_handle.max(lt.handle.value());
    }
    for (_, ts) in &doc.text_styles {
        max_handle = max_handle.max(ts.handle.value());
    }
    for (_, ds) in &doc.dim_styles {
        max_handle = max_handle.max(ds.handle.value());
    }
    max_handle = max_handle.max(doc.header.handseed);
    doc.set_next_handle(max_handle + 1);

    let pre_seeded: Vec<Handle> = doc
        .block_records
        .keys()
        .copied()
        .filter(|h| {
            let br = &doc.block_records[h];
            br.block_entity_handle == Handle::NULL
                && (br.name == "*Model_Space" || br.name == "*Paper_Space")
                && doc
                    .block_records
                    .values()
                    .any(|other| other.name == br.name && other.block_entity_handle != Handle::NULL)
        })
        .collect();
    for h in pre_seeded {
        doc.block_records.remove(&h);
    }
}

pub fn write_dxf(doc: &CadDocument) -> Result<String, String> {
    write_dxf_string(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_dxf() -> &'static str {
        concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  0\nENDSEC\n",
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        )
    }

    #[test]
    fn read_dxf_parses_minimal_file() {
        let doc = read_dxf(minimal_dxf()).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
    }

    #[test]
    fn read_dxf_reads_acadver_r2018() {
        let input = concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1032\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2018);
    }

    #[test]
    fn read_dxf_reads_acadver_r12() {
        let input = concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1009\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R12);
    }

    #[test]
    fn read_dxf_header_skips_unknown_variables() {
        let input = concat!(
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$EXTMIN\n 10\n0.0\n 20\n0.0\n 30\n0.0\n",
            "  9\n$ACADVER\n  1\nAC1021\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2007);
    }

    #[test]
    fn read_dxf_parses_classes_section() {
        let input = concat!(
            "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nCLASSES\n",
            "  0\nCLASS\n  1\nACDB_MLEADERSTYLE\n  2\nAcDbMLeaderStyle\n  3\nACAD\n 90\n4095\n 91\n0\n280\n0\n281\n0\n",
            "  0\nCLASS\n  1\nACDBDICTIONARYWDFLT\n  2\nAcDbDictionaryWithDefault\n  3\n\n 90\n0\n280\n0\n281\n0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.classes.len(), 2);
        assert_eq!(doc.classes[0].dxf_name, "ACDB_MLEADERSTYLE");
        assert_eq!(doc.classes[0].cpp_class_name, "AcDbMLeaderStyle");
        assert_eq!(doc.classes[0].application_name, "ACAD");
        assert_eq!(doc.classes[0].proxy_flags, 4095);
        assert!(!doc.classes[0].is_an_entity);
        assert_eq!(doc.classes[1].dxf_name, "ACDBDICTIONARYWDFLT");
    }

    #[test]
    fn read_dxf_parses_tables_section() {
        let input = concat!(
            "  0\nSECTION\n  2\nTABLES\n",
            "  0\nTABLE\n  2\nLAYER\n  5\n2\n 70\n2\n",
            "  0\nLAYER\n  5\n10\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n  2\n0\n 70\n0\n 62\n7\n  6\nContinuous\n",
            "  0\nLAYER\n  5\n11\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n  2\nDimensions\n 70\n0\n 62\n1\n  6\nContinuous\n",
            "  0\nENDTAB\n",
            "  0\nTABLE\n  2\nLTYPE\n  5\n5\n 70\n1\n",
            "  0\nLTYPE\n  5\n14\n100\nAcDbSymbolTableRecord\n100\nAcDbLinetypeTableRecord\n  2\nByBlock\n 70\n0\n",
            "  0\nENDTAB\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.tables.layer.entries.len(), 2);
        assert!(doc.tables.layer.entries.contains_key("0"));
        assert!(doc.tables.layer.entries.contains_key("Dimensions"));
        assert_eq!(
            doc.tables.layer.entries.get("0"),
            Some(&h7cad_native_model::Handle::new(0x10))
        );
        assert_eq!(doc.tables.linetype.entries.len(), 1);
        assert!(doc.tables.linetype.entries.contains_key("ByBlock"));
    }

    #[test]
    fn read_dxf_parses_blocks_section() {
        let input = concat!(
            "  0\nSECTION\n  2\nBLOCKS\n",
            "  0\nBLOCK\n  5\n20\n  8\n0\n  2\n*Model_Space\n 70\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  3\n*Model_Space\n  1\n\n",
            "  0\nENDBLK\n  5\n21\n  8\n0\n",
            "  0\nBLOCK\n  5\n1C\n  8\n0\n  2\n*Paper_Space\n 70\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  3\n*Paper_Space\n  1\n\n",
            "  0\nENDBLK\n  5\n1D\n  8\n0\n",
            "  0\nBLOCK\n  5\n30\n  8\n0\n  2\nMyBlock\n 70\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  3\nMyBlock\n  1\n\n",
            "  0\nLINE\n  5\n31\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 11\n1.0\n 21\n1.0\n 31\n0.0\n",
            "  0\nENDBLK\n  5\n32\n  8\n0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert!(doc.tables.block_record.entries.contains_key("MyBlock"));
        assert_eq!(
            doc.tables.block_record.entries.get("MyBlock"),
            Some(&h7cad_native_model::Handle::new(0x30))
        );
    }

    #[test]
    fn read_dxf_parses_entities_line() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nLINE\n  5\nA0\n  8\nLayer1\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 11\n10.0\n 21\n20.0\n 31\n0.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        let e = &doc.entities[0];
        assert_eq!(e.handle, h7cad_native_model::Handle::new(0xA0));
        assert_eq!(e.layer_name, "Layer1");
        match &e.data {
            h7cad_native_model::EntityData::Line { start, end } => {
                assert_eq!(*start, [1.0, 2.0, 0.0]);
                assert_eq!(*end, [10.0, 20.0, 0.0]);
            }
            _ => panic!("expected Line"),
        }
    }

    #[test]
    fn read_dxf_parses_entities_circle_arc() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nCIRCLE\n  5\nB0\n  8\n0\n 10\n5.0\n 20\n5.0\n 30\n0.0\n 40\n3.0\n",
            "  0\nARC\n  5\nB1\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 40\n10.0\n 50\n45.0\n 51\n135.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 2);

        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Circle { center, radius } => {
                assert_eq!(*center, [5.0, 5.0, 0.0]);
                assert_eq!(*radius, 3.0);
            }
            _ => panic!("expected Circle"),
        }
        match &doc.entities[1].data {
            h7cad_native_model::EntityData::Arc {
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                assert_eq!(*radius, 10.0);
                assert_eq!(*start_angle, 45.0);
                assert_eq!(*end_angle, 135.0);
            }
            _ => panic!("expected Arc"),
        }
    }

    #[test]
    fn read_dxf_parses_lwpolyline() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nLWPOLYLINE\n  5\nC0\n  8\n0\n 90\n3\n 70\n1\n",
            " 10\n0.0\n 20\n0.0\n",
            " 10\n10.0\n 20\n0.0\n",
            " 10\n10.0\n 20\n10.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::LwPolyline { vertices, closed } => {
                assert!(closed);
                assert_eq!(vertices.len(), 3);
                assert_eq!(vertices[0].x, 0.0);
                assert_eq!(vertices[1].x, 10.0);
                assert_eq!(vertices[2].y, 10.0);
            }
            _ => panic!("expected LwPolyline"),
        }
    }

    #[test]
    fn read_dxf_parses_text() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nTEXT\n  5\nD0\n  8\n0\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 40\n2.5\n  1\nHello World\n 50\n0.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Text { height, value, .. } => {
                assert_eq!(*height, 2.5);
                assert_eq!(value, "Hello World");
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn read_dxf_parses_polyline_2d() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nPOLYLINE\n  5\nF0\n  8\n0\n 66\n1\n 70\n1\n",
            "  0\nVERTEX\n  5\nF1\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n",
            "  0\nVERTEX\n  5\nF2\n  8\n0\n 10\n5.0\n 20\n0.0\n 30\n0.0\n",
            "  0\nVERTEX\n  5\nF3\n  8\n0\n 10\n5.0\n 20\n5.0\n 30\n0.0\n",
            "  0\nSEQEND\n  5\nF4\n  8\n0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Polyline {
                polyline_type,
                vertices,
                closed,
            } => {
                assert_eq!(*polyline_type, h7cad_native_model::PolylineType::Polyline2D);
                assert!(closed);
                assert_eq!(vertices.len(), 3);
                assert_eq!(vertices[0].position, [0.0, 0.0, 0.0]);
                assert_eq!(vertices[1].position, [5.0, 0.0, 0.0]);
                assert_eq!(vertices[2].position, [5.0, 5.0, 0.0]);
            }
            _ => panic!("expected Polyline"),
        }
    }

    #[test]
    fn read_dxf_parses_polyline_3d() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nPOLYLINE\n  5\nG0\n  8\n0\n 66\n1\n 70\n8\n",
            "  0\nVERTEX\n 10\n0.0\n 20\n0.0\n 30\n0.0\n",
            "  0\nVERTEX\n 10\n1.0\n 20\n2.0\n 30\n3.0\n",
            "  0\nSEQEND\n",
            "  0\nLINE\n  5\nG1\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 11\n1.0\n 21\n1.0\n 31\n0.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 2);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Polyline {
                polyline_type,
                vertices,
                ..
            } => {
                assert_eq!(*polyline_type, h7cad_native_model::PolylineType::Polyline3D);
                assert_eq!(vertices.len(), 2);
                assert_eq!(vertices[1].position, [1.0, 2.0, 3.0]);
            }
            _ => panic!("expected Polyline"),
        }
        assert!(matches!(
            doc.entities[1].data,
            h7cad_native_model::EntityData::Line { .. }
        ));
    }

    #[test]
    fn read_dxf_unknown_entity_preserved() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nHELIX\n  5\nE0\n  8\n0\n 70\n0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Unknown { entity_type } => {
                assert_eq!(entity_type, "HELIX");
            }
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn read_dxf_skips_unknown_sections() {
        let input = concat!(
            "  0\nSECTION\n  2\nTHUMBNAILIMAGE\n",
            " 90\n12345\n",
            "  0\nENDSEC\n",
            "  0\nSECTION\n  2\nHEADER\n",
            "  9\n$ACADVER\n  1\nAC1015\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
    }

    #[test]
    fn read_dxf_handles_all_six_sections() {
        let input = concat!(
            "  0\nSECTION\n  2\nHEADER\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nCLASSES\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nTABLES\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nBLOCKS\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n",
            "  0\nSECTION\n  2\nOBJECTS\n  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
    }

    #[test]
    fn read_dxf_errors_on_missing_section_name() {
        let input = "  0\nSECTION\n";
        let err = read_dxf(input).unwrap_err();
        assert!(matches!(err, DxfReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn read_dxf_errors_on_wrong_code_after_section() {
        let input = "  0\nSECTION\n  0\nHEADER\n  0\nENDSEC\n  0\nEOF\n";
        let err = read_dxf(input).unwrap_err();
        assert!(matches!(err, DxfReadError::UnexpectedToken { .. }));
    }

    #[test]
    fn read_dxf_errors_on_missing_endsec() {
        let input = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n";
        let err = read_dxf(input).unwrap_err();
        assert!(matches!(err, DxfReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn section_name_from_dxf_roundtrip() {
        assert_eq!(DxfSectionName::from_dxf("HEADER"), Some(DxfSectionName::Header));
        assert_eq!(DxfSectionName::from_dxf("CLASSES"), Some(DxfSectionName::Classes));
        assert_eq!(DxfSectionName::from_dxf("TABLES"), Some(DxfSectionName::Tables));
        assert_eq!(DxfSectionName::from_dxf("BLOCKS"), Some(DxfSectionName::Blocks));
        assert_eq!(DxfSectionName::from_dxf("ENTITIES"), Some(DxfSectionName::Entities));
        assert_eq!(DxfSectionName::from_dxf("OBJECTS"), Some(DxfSectionName::Objects));
        assert_eq!(DxfSectionName::from_dxf("THUMBNAILIMAGE"), None);
        assert_eq!(DxfSectionName::from_dxf(""), None);
    }

    #[test]
    fn stream_reader_find_and_skip() {
        let input = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  0\nENDSEC\n  0\nEOF\n";
        let mut stream = DxfStreamReader::new(input);

        assert!(stream.find(0, "SECTION").unwrap());
        assert!(stream.read_next().unwrap());
        assert_eq!(stream.current_code(), 2);
        assert_eq!(stream.current_value_trimmed(), "HEADER");

        stream.skip_section().unwrap();
        assert_eq!(stream.current_code(), 0);
        assert_eq!(stream.current_value_trimmed(), "ENDSEC");

        assert!(!stream.find(0, "SECTION").unwrap());
    }

    // -----------------------------------------------------------------------
    // Integration: ACadSharp DXF samples
    // -----------------------------------------------------------------------

    #[test]
    fn read_acad_sample_ac1015() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found at {path}");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
        assert!(!doc.entities.is_empty(), "should have entities");
        assert!(
            !doc.tables.layer.entries.is_empty(),
            "should have layers"
        );
        eprintln!(
            "AC1015: {} entities, {} layers, {} classes, {} block_records",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.classes.len(),
            doc.tables.block_record.entries.len(),
        );
    }

    #[test]
    fn read_acad_sample_ac1009() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1009_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R12);
        eprintln!(
            "AC1009: {} entities, {} layers",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
        );
    }

    #[test]
    fn read_acad_sample_ac1018() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1018_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2004);
        assert!(!doc.entities.is_empty());
        eprintln!(
            "AC1018: {} entities, {} layers, {} objects",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.objects.len(),
        );
    }

    #[test]
    fn read_acad_sample_ac1021() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1021_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2007);
        assert!(!doc.entities.is_empty());
        eprintln!(
            "AC1021: {} entities, {} layers, {} objects",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.objects.len(),
        );
    }

    #[test]
    fn read_acad_sample_ac1024() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1024_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2010);
        assert!(!doc.entities.is_empty());
        eprintln!(
            "AC1024: {} entities, {} layers, {} objects",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.objects.len(),
        );
    }

    #[test]
    fn read_acad_sample_ac1027() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1027_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2013);
        assert!(!doc.entities.is_empty());
        eprintln!(
            "AC1027: {} entities, {} layers, {} objects",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.objects.len(),
        );
    }

    #[test]
    fn entity_type_distribution_ac1015() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for e in &doc.entities {
            let name = match &e.data {
                h7cad_native_model::EntityData::Line { .. } => "LINE",
                h7cad_native_model::EntityData::Circle { .. } => "CIRCLE",
                h7cad_native_model::EntityData::Arc { .. } => "ARC",
                h7cad_native_model::EntityData::Point { .. } => "POINT",
                h7cad_native_model::EntityData::Ellipse { .. } => "ELLIPSE",
                h7cad_native_model::EntityData::Spline { .. } => "SPLINE",
                h7cad_native_model::EntityData::LwPolyline { .. } => "LWPOLYLINE",
                h7cad_native_model::EntityData::Polyline { .. } => "POLYLINE",
                h7cad_native_model::EntityData::Text { .. } => "TEXT",
                h7cad_native_model::EntityData::MText { .. } => "MTEXT",
                h7cad_native_model::EntityData::Insert { .. } => "INSERT",
                h7cad_native_model::EntityData::Dimension { .. } => "DIMENSION",
                h7cad_native_model::EntityData::Hatch { .. } => "HATCH",
                h7cad_native_model::EntityData::Leader { .. } => "LEADER",
                h7cad_native_model::EntityData::Attrib { .. } => "ATTRIB",
                h7cad_native_model::EntityData::AttDef { .. } => "ATTDEF",
                h7cad_native_model::EntityData::Viewport { .. } => "VIEWPORT",
                h7cad_native_model::EntityData::Face3D { .. } => "3DFACE",
                h7cad_native_model::EntityData::Solid { .. } => "SOLID",
                h7cad_native_model::EntityData::Ray { .. } => "RAY",
                h7cad_native_model::EntityData::XLine { .. } => "XLINE",
                h7cad_native_model::EntityData::MLine { .. } => "MLINE",
                h7cad_native_model::EntityData::Image { .. } => "IMAGE",
                h7cad_native_model::EntityData::Wipeout { .. } => "WIPEOUT",
                h7cad_native_model::EntityData::Tolerance { .. } => "TOLERANCE",
                h7cad_native_model::EntityData::Shape { .. } => "SHAPE",
                h7cad_native_model::EntityData::Solid3D { .. } => "3DSOLID",
                h7cad_native_model::EntityData::Region { .. } => "REGION",
                h7cad_native_model::EntityData::MultiLeader { .. } => "MULTILEADER",
                h7cad_native_model::EntityData::Table { .. } => "ACAD_TABLE",
                h7cad_native_model::EntityData::Mesh { .. } => "MESH",
                h7cad_native_model::EntityData::PdfUnderlay { .. } => "PDFUNDERLAY",
                h7cad_native_model::EntityData::Unknown { entity_type } => entity_type.as_str(),
            };
            *counts.entry(name.to_string()).or_default() += 1;
        }
        eprintln!("Entity type distribution (AC1015):");
        for (name, count) in &counts {
            eprintln!("  {name}: {count}");
        }
    }

    #[test]
    fn hatch_boundary_paths_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let hatches: Vec<_> = doc
            .entities
            .iter()
            .filter(|e| matches!(&e.data, h7cad_native_model::EntityData::Hatch { .. }))
            .collect();
        assert!(!hatches.is_empty(), "should have HATCH entities");
        let mut total_paths = 0;
        let mut total_edges = 0;
        for h in &hatches {
            if let h7cad_native_model::EntityData::Hatch {
                boundary_paths, ..
            } = &h.data
            {
                total_paths += boundary_paths.len();
                for bp in boundary_paths {
                    total_edges += bp.edges.len();
                }
            }
        }
        eprintln!(
            "HATCH: {} entities, {} boundary paths, {} edges",
            hatches.len(),
            total_paths,
            total_edges,
        );
        assert!(total_paths > 0, "should parse boundary paths");
        assert!(total_edges > 0, "should parse boundary edges");
    }

    #[test]
    fn dimension_subtypes_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let dims: Vec<_> = doc
            .entities
            .iter()
            .filter_map(|e| {
                if let h7cad_native_model::EntityData::Dimension {
                    dim_type,
                    block_name,
                    style_name,
                    measurement,
                    first_point,
                    second_point,
                    angle_vertex,
                    ..
                } = &e.data
                {
                    Some((*dim_type, block_name.clone(), style_name.clone(), *measurement, *first_point, *second_point, *angle_vertex))
                } else {
                    None
                }
            })
            .collect();
        assert!(!dims.is_empty(), "should have DIMENSION entities");

        let mut type_counts = std::collections::HashMap::new();
        for (dt, _, _, _, _, _, _) in &dims {
            let base_type = dt & 0x0F;
            *type_counts.entry(base_type).or_insert(0) += 1;
        }
        eprintln!("DIMENSION sub-types: {:?}", type_counts);

        for (dt, block_name, style_name, measurement, _, _, _) in &dims {
            assert!(!block_name.is_empty(), "dim type {} should have block_name", dt);
            eprintln!(
                "  dim_type={} (base={}), block={}, style={}, measurement={}",
                dt,
                dt & 0x0F,
                block_name,
                style_name,
                measurement,
            );
        }
    }

    #[test]
    fn dimension_linear_parses_subtype_fields() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nDIMENSION\n  5\nD0\n  8\n0\n",
            " 70\n     0\n",
            "  2\n*D1\n  3\nISO-25\n",
            "  1\n<>mm\n",
            " 10\n5.0\n 20\n10.0\n 30\n0.0\n",
            " 11\n2.5\n 21\n5.0\n 31\n0.0\n",
            " 13\n0.0\n 23\n0.0\n 33\n0.0\n",
            " 14\n5.0\n 24\n0.0\n 34\n0.0\n",
            " 42\n5.0\n 50\n0.0\n 52\n0.0\n",
            " 71\n     5\n",
            "  0\nENDSEC\n  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Dimension {
                dim_type,
                block_name,
                style_name,
                definition_point,
                text_midpoint,
                text_override,
                measurement,
                first_point,
                second_point,
                rotation,
                ext_line_rotation,
                attachment_point,
                ..
            } => {
                assert_eq!(*dim_type & 0x0F, 0, "should be Linear");
                assert_eq!(block_name, "*D1");
                assert_eq!(style_name, "ISO-25");
                assert_eq!(text_override, "<>mm");
                assert_eq!(definition_point, &[5.0, 10.0, 0.0]);
                assert_eq!(text_midpoint, &[2.5, 5.0, 0.0]);
                assert_eq!(first_point, &[0.0, 0.0, 0.0]);
                assert_eq!(second_point, &[5.0, 0.0, 0.0]);
                assert_eq!(*measurement, 5.0);
                assert_eq!(*rotation, 0.0);
                assert_eq!(*ext_line_rotation, 0.0);
                assert_eq!(*attachment_point, 5);
            }
            _ => panic!("expected Dimension"),
        }
    }

    #[test]
    fn dimension_radius_parses_subtype_fields() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nDIMENSION\n  5\nD1\n  8\n0\n",
            " 70\n     4\n",
            "  2\n*D2\n  3\nStandard\n",
            " 10\n10.0\n 20\n10.0\n 30\n0.0\n",
            " 15\n15.0\n 25\n10.0\n 35\n0.0\n",
            " 40\n3.5\n 42\n5.0\n",
            "  0\nENDSEC\n  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 1);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Dimension {
                dim_type,
                angle_vertex,
                leader_length,
                measurement,
                ..
            } => {
                assert_eq!(*dim_type & 0x0F, 4, "should be Radius");
                assert_eq!(angle_vertex, &[15.0, 10.0, 0.0]);
                assert_eq!(*leader_length, 3.5);
                assert_eq!(*measurement, 5.0);
            }
            _ => panic!("expected Dimension"),
        }
    }

    #[test]
    fn object_type_distribution_ac1018() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1018_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let mut type_counts = std::collections::BTreeMap::new();
        for obj in &doc.objects {
            let name = match &obj.data {
                h7cad_native_model::ObjectData::Dictionary { .. } => "DICTIONARY",
                h7cad_native_model::ObjectData::XRecord { .. } => "XRECORD",
                h7cad_native_model::ObjectData::Group { .. } => "GROUP",
                h7cad_native_model::ObjectData::Layout { .. } => "LAYOUT",
                h7cad_native_model::ObjectData::PlotSettings { .. } => "PLOTSETTINGS",
                h7cad_native_model::ObjectData::DictionaryVar { .. } => "DICTIONARYVAR",
                h7cad_native_model::ObjectData::Scale { .. } => "SCALE",
                h7cad_native_model::ObjectData::VisualStyle { .. } => "VISUALSTYLE",
                h7cad_native_model::ObjectData::Material { .. } => "MATERIAL",
                h7cad_native_model::ObjectData::ImageDef { .. } => "IMAGEDEF",
                h7cad_native_model::ObjectData::ImageDefReactor { .. } => "IMAGEDEF_REACTOR",
                h7cad_native_model::ObjectData::MLineStyle { .. } => "MLINESTYLE",
                h7cad_native_model::ObjectData::MLeaderStyle { .. } => "MLEADERSTYLE",
                h7cad_native_model::ObjectData::TableStyle { .. } => "TABLESTYLE",
                h7cad_native_model::ObjectData::SortEntsTable { .. } => "SORTENTSTABLE",
                h7cad_native_model::ObjectData::DimAssoc { .. } => "DIMASSOC",
                h7cad_native_model::ObjectData::Unknown { object_type } => object_type.as_str(),
            };
            *type_counts.entry(name.to_string()).or_insert(0u32) += 1;
        }
        let unknown_count: u32 = doc.objects.iter().filter(|o| matches!(&o.data, h7cad_native_model::ObjectData::Unknown { .. })).count() as u32;
        let known_count = doc.objects.len() as u32 - unknown_count;
        eprintln!("OBJECTS type distribution (AC1018, {} total):", doc.objects.len());
        for (name, count) in &type_counts {
            let is_unknown = matches!(&name.as_str(), &n if {
                let _doc_objs = &doc.objects;
                !matches!(n, "DICTIONARY" | "XRECORD" | "GROUP" | "LAYOUT" | "PLOTSETTINGS" |
                    "DICTIONARYVAR" | "SCALE" | "VISUALSTYLE" | "MATERIAL" |
                    "IMAGEDEF" | "IMAGEDEF_REACTOR" | "MLINESTYLE" | "MLEADERSTYLE" |
                    "TABLESTYLE" | "SORTENTSTABLE" | "DIMASSOC")
            });
            eprintln!("  {}: {}{}", name, count, if is_unknown { " [unknown]" } else { "" });
        }
        eprintln!("  >> {} known, {} unknown", known_count, unknown_count);
    }

    #[test]
    fn geometry_data_integrity_ac1015() {
        use h7cad_native_model::EntityData;
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();

        for ent in &doc.entities {
            match &ent.data {
                EntityData::Line { start, end } => {
                    assert!(start.iter().all(|v| v.is_finite()), "LINE start must be finite");
                    assert!(end.iter().all(|v| v.is_finite()), "LINE end must be finite");
                }
                EntityData::Circle { center, radius } => {
                    assert!(center.iter().all(|v| v.is_finite()));
                    assert!(*radius > 0.0, "CIRCLE radius must be positive, got {radius}");
                }
                EntityData::Arc { center, radius, start_angle, end_angle } => {
                    assert!(center.iter().all(|v| v.is_finite()));
                    assert!(*radius > 0.0, "ARC radius must be positive");
                    assert!(start_angle.is_finite() && end_angle.is_finite());
                }
                EntityData::Ellipse { center, major_axis, ratio, .. } => {
                    assert!(center.iter().all(|v| v.is_finite()));
                    let axis_len = (major_axis[0].powi(2) + major_axis[1].powi(2) + major_axis[2].powi(2)).sqrt();
                    assert!(axis_len > 0.0, "ELLIPSE major_axis length must be > 0");
                    assert!(*ratio > 0.0 && *ratio <= 1.0, "ELLIPSE ratio must be in (0,1], got {ratio}");
                }
                EntityData::Spline { degree, knots, control_points, weights, .. } => {
                    assert!(*degree >= 1, "SPLINE degree must be >= 1, got {degree}");
                    assert!(!control_points.is_empty(), "SPLINE must have control points");
                    assert!(!knots.is_empty(), "SPLINE must have knots");
                    if !weights.is_empty() {
                        assert_eq!(weights.len(), control_points.len(),
                            "SPLINE weights count must match control_points count");
                    }
                }
                EntityData::LwPolyline { vertices, .. } => {
                    assert!(!vertices.is_empty(), "LWPOLYLINE must have vertices");
                }
                EntityData::Polyline { vertices, .. } => {
                    assert!(!vertices.is_empty(), "POLYLINE must have vertices");
                }
                EntityData::Hatch { boundary_paths, .. } => {
                    for bp in boundary_paths {
                        assert!(!bp.edges.is_empty(), "HATCH boundary path must have edges");
                    }
                }
                EntityData::Insert { block_name, attribs, has_attribs, .. } => {
                    assert!(!block_name.is_empty(), "INSERT must have block_name");
                    if *has_attribs {
                        assert!(!attribs.is_empty(), "INSERT with has_attribs should have attribs");
                    }
                }
                EntityData::Dimension { block_name, .. } => {
                    assert!(!block_name.is_empty(), "DIMENSION must have block_name");
                }
                _ => {}
            }
        }
        eprintln!("Geometry data integrity: all {} entities passed", doc.entities.len());
    }

    #[test]
    fn block_entities_parsed_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let mut blocks_with_entities = 0;
        let mut total_block_entities = 0;
        for (_, record) in &doc.block_records {
            if !record.entities.is_empty() {
                blocks_with_entities += 1;
                total_block_entities += record.entities.len();
                eprintln!(
                    "  Block '{}': {} entities",
                    record.name,
                    record.entities.len(),
                );
            }
        }
        eprintln!(
            "BLOCKS: {} with entities, {} total block entities, {} block_records total",
            blocks_with_entities,
            total_block_entities,
            doc.block_records.len(),
        );
        assert!(blocks_with_entities > 0, "some blocks should have entities");
    }

    #[test]
    fn layer_properties_parsed_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();

        assert!(
            doc.layers.len() >= 2,
            "should have at least default '0' + other layers, got {}",
            doc.layers.len(),
        );

        let layer0 = doc.layers.get("0").expect("layer '0' must exist");
        assert!(layer0.color >= 0, "default layer should be on");
        assert!(!layer0.is_frozen, "default layer should not be frozen");

        let mut layers_with_color = 0;
        let mut layers_with_linetype = 0;
        for (name, layer) in &doc.layers {
            if layer.color != 7 {
                layers_with_color += 1;
            }
            if layer.linetype_name != "Continuous" {
                layers_with_linetype += 1;
            }
            eprintln!(
                "  Layer '{}': color={}, ltype={}, lw={}, frozen={}, locked={}, plot={}",
                name,
                layer.color,
                layer.linetype_name,
                layer.lineweight,
                layer.is_frozen,
                layer.is_locked,
                layer.plot,
            );
        }
        eprintln!(
            "LAYERS: {} total, {} non-default color, {} non-Continuous ltype",
            doc.layers.len(),
            layers_with_color,
            layers_with_linetype,
        );

        assert_eq!(
            doc.layers.len(),
            doc.tables.layer.entries.len(),
            "layer count should match SymbolTable count",
        );
    }

    #[test]
    fn table_properties_parsed_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();

        // --- LTYPE ---
        assert!(
            doc.linetypes.len() >= 3,
            "should have at least Continuous/ByLayer/ByBlock, got {}",
            doc.linetypes.len(),
        );
        let cont = doc.linetypes.get("Continuous").expect("Continuous must exist");
        assert!(cont.is_continuous(), "Continuous should have no segments");

        let mut complex_ltypes = 0;
        for (name, lt) in &doc.linetypes {
            if !lt.segments.is_empty() {
                complex_ltypes += 1;
            }
            eprintln!(
                "  LTYPE '{}': desc='{}', len={}, segs={}",
                name,
                lt.description,
                lt.pattern_length,
                lt.segments.len(),
            );
        }
        eprintln!(
            "LINETYPES: {} total, {} complex (with segments)",
            doc.linetypes.len(),
            complex_ltypes,
        );
        assert_eq!(
            doc.linetypes.len(),
            doc.tables.linetype.entries.len(),
            "linetype count should match SymbolTable",
        );

        // --- STYLE ---
        assert!(
            !doc.text_styles.is_empty(),
            "should have text styles",
        );
        for (name, ts) in &doc.text_styles {
            eprintln!(
                "  STYLE '{}': h={}, wf={}, font='{}'",
                name,
                ts.height,
                ts.width_factor,
                ts.font_name,
            );
        }
        assert_eq!(
            doc.text_styles.len(),
            doc.tables.style.entries.len(),
            "style count should match SymbolTable",
        );

        // --- DIMSTYLE ---
        assert!(
            !doc.dim_styles.is_empty(),
            "should have dim styles",
        );
        let std_ds = doc.dim_styles.get("Standard").or_else(|| doc.dim_styles.values().next());
        if let Some(ds) = std_ds {
            eprintln!(
                "  DIMSTYLE '{}': scale={}, asz={}, txt={}, dec={}",
                ds.name,
                ds.dimscale,
                ds.dimasz,
                ds.dimtxt,
                ds.dimdec,
            );
            assert!(ds.dimscale > 0.0, "dimscale should be positive");
        }
        assert_eq!(
            doc.dim_styles.len(),
            doc.tables.dimstyle.entries.len(),
            "dimstyle count should match SymbolTable",
        );
    }

    #[test]
    fn header_variables_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();
        let h = &doc.header;

        eprintln!(
            "HEADER: ver={:?}, extmin=[{:.2},{:.2},{:.2}], extmax=[{:.2},{:.2},{:.2}]",
            h.version, h.extmin[0], h.extmin[1], h.extmin[2], h.extmax[0], h.extmax[1], h.extmax[2],
        );
        eprintln!(
            "  ltscale={}, textsize={}, dimscale={}, lunits={}, luprec={}",
            h.ltscale, h.textsize, h.dimscale, h.lunits, h.luprec,
        );
        eprintln!("  handseed={:X}, next_handle={:X}", h.handseed, doc.next_handle());

        assert!(h.extmax[0] > h.extmin[0], "extmax.x should > extmin.x");
        assert!(h.extmax[1] > h.extmin[1], "extmax.y should > extmin.y");
        assert!(h.ltscale > 0.0, "ltscale should be positive");
        assert!(h.handseed > 0, "handseed should be set");
        assert!(
            doc.next_handle() >= h.handseed,
            "next_handle should be >= handseed",
        );
    }

    #[test]
    fn cross_references_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();

        let ms_handle = doc.model_space_handle();
        assert_ne!(ms_handle, h7cad_native_model::Handle::NULL, "model space handle should exist");

        let mut ms_entities = 0;
        let mut ps_entities = 0;
        let mut with_owner = 0;
        for entity in &doc.entities {
            if entity.owner_handle != h7cad_native_model::Handle::NULL {
                with_owner += 1;
            }
            if doc.is_model_space_entity(entity) {
                ms_entities += 1;
            } else {
                ps_entities += 1;
            }
        }
        eprintln!(
            "CROSS-REF: {} entities with owner, {} model space, {} paper space",
            with_owner, ms_entities, ps_entities,
        );
        assert!(with_owner > 0, "some entities should have owner_handle");
        assert!(ms_entities > 0, "should have model space entities");

        let mut resolved_colors = 0;
        let mut bylayer = 0;
        for entity in &doc.entities {
            let color = doc.resolve_color(entity);
            if entity.color_index == 256 {
                bylayer += 1;
            }
            if color > 0 && color < 256 {
                resolved_colors += 1;
            }
        }
        eprintln!(
            "COLOR: {} ByLayer resolved, {} total with valid ACI",
            bylayer, resolved_colors,
        );

        let mut insert_resolved = 0;
        for entity in &doc.entities {
            if doc.resolve_insert_block(entity).is_some() {
                insert_resolved += 1;
            }
        }
        eprintln!("INSERT: {} resolved to block records", insert_resolved);
        assert!(insert_resolved > 0, "some INSERTs should resolve to blocks");
    }

    #[test]
    fn entity_extended_properties_in_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            return;
        };
        let doc = read_dxf(&input).unwrap();

        let mut text_with_style = 0;
        let mut mtext_with_style = 0;
        let mut with_xdata = 0;
        let mut xdata_apps: std::collections::BTreeSet<String> = Default::default();

        for entity in doc.entities.iter().chain(
            doc.block_records.values().flat_map(|br| br.entities.iter()),
        ) {
            match &entity.data {
                h7cad_native_model::EntityData::Text { style_name, .. } => {
                    if !style_name.is_empty() {
                        text_with_style += 1;
                    }
                }
                h7cad_native_model::EntityData::MText { style_name, .. } => {
                    if !style_name.is_empty() {
                        mtext_with_style += 1;
                    }
                }
                _ => {}
            }
            if !entity.xdata.is_empty() {
                with_xdata += 1;
                for (app, _) in &entity.xdata {
                    xdata_apps.insert(app.clone());
                }
            }
        }

        eprintln!(
            "STYLES: {} TEXT with style, {} MTEXT with style",
            text_with_style, mtext_with_style,
        );
        eprintln!(
            "XDATA: {} entities with xdata, apps: {:?}",
            with_xdata, xdata_apps,
        );
    }

    #[test]
    fn read_dxf_parses_insert_with_attribs() {
        let input = concat!(
            "  0\nSECTION\n  2\nENTITIES\n",
            "  0\nINSERT\n  5\nA0\n  8\n0\n  2\nMyBlock\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 66\n1\n",
            "  0\nATTRIB\n  5\nA1\n  8\n0\n  2\nTAG1\n  1\nValue1\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 40\n2.5\n",
            "  0\nATTRIB\n  5\nA2\n  8\n0\n  2\nTAG2\n  1\nValue2\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 40\n2.5\n",
            "  0\nSEQEND\n  5\nA3\n  8\n0\n",
            "  0\nLINE\n  5\nB0\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 11\n1.0\n 21\n1.0\n 31\n0.0\n",
            "  0\nENDSEC\n",
            "  0\nEOF\n",
        );
        let doc = read_dxf(input).unwrap();
        assert_eq!(doc.entities.len(), 2);
        match &doc.entities[0].data {
            h7cad_native_model::EntityData::Insert {
                block_name,
                has_attribs,
                attribs,
                ..
            } => {
                assert_eq!(block_name, "MyBlock");
                assert!(has_attribs);
                assert_eq!(attribs.len(), 2);
                match &attribs[0].data {
                    h7cad_native_model::EntityData::Attrib { tag, value, .. } => {
                        assert_eq!(tag, "TAG1");
                        assert_eq!(value, "Value1");
                    }
                    _ => panic!("expected Attrib"),
                }
            }
            _ => panic!("expected Insert"),
        }
        assert!(matches!(
            doc.entities[1].data,
            h7cad_native_model::EntityData::Line { .. }
        ));
    }

    #[test]
    fn read_acad_sample_ac1032() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1032_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found at {path}");
            return;
        };
        let doc = read_dxf(&input).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2018);
        assert!(!doc.entities.is_empty(), "should have entities");
        eprintln!(
            "AC1032: {} entities, {} layers, {} classes",
            doc.entities.len(),
            doc.tables.layer.entries.len(),
            doc.classes.len(),
        );
    }

    // -----------------------------------------------------------------------
    // Tokenizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn tokenizer_reads_group_code_pairs() {
        let input = "0\nSECTION\n2\nHEADER\n0\nENDSEC\n";
        let tokens = tokenize_dxf(input).unwrap();

        assert_eq!(
            tokens,
            vec![
                DxfToken::new(GroupCode::new(0).unwrap(), "SECTION"),
                DxfToken::new(GroupCode::new(2).unwrap(), "HEADER"),
                DxfToken::new(GroupCode::new(0).unwrap(), "ENDSEC"),
            ]
        );
    }

    #[test]
    fn tokenizer_reports_missing_value_line() {
        let err = tokenize_dxf("0\nSECTION\n2\n").unwrap_err();

        assert_eq!(
            err,
            DxfParseError::UnexpectedEndOfInput {
                expected: "value line",
                line: 4,
            }
        );
    }

    #[test]
    fn tokenizer_rejects_negative_group_codes() {
        let err = tokenize_dxf("-1\noops\n").unwrap_err();

        assert_eq!(err, DxfParseError::InvalidGroupCode("-1".to_string()));
    }

    #[test]
    fn decode_supports_common_scalar_types() {
        let string_token = DxfToken::new(GroupCode::new(8).unwrap(), "Layer0");
        let double_token = DxfToken::new(GroupCode::new(10).unwrap(), "12.5");
        let short_token = DxfToken::new(GroupCode::new(70).unwrap(), "7");
        let bool_token = DxfToken::new(GroupCode::new(290).unwrap(), "1");
        let long_token = DxfToken::new(GroupCode::new(420).unwrap(), "16711680");
        let binary_token = DxfToken::new(GroupCode::new(1004).unwrap(), "0A0B");

        assert_eq!(string_token.decode().unwrap(), DxfValue::Str("Layer0".into()));
        assert_eq!(double_token.decode().unwrap(), DxfValue::Double(12.5));
        assert_eq!(short_token.decode().unwrap(), DxfValue::Short(7));
        assert_eq!(bool_token.decode().unwrap(), DxfValue::Bool(true));
        assert_eq!(long_token.decode().unwrap(), DxfValue::Long(16_711_680));
        assert_eq!(binary_token.decode().unwrap(), DxfValue::Binary(vec![0x0A, 0x0B]));
    }

    #[test]
    fn decode_reports_invalid_scalars() {
        let err = DxfToken::new(GroupCode::new(10).unwrap(), "abc")
            .decode()
            .unwrap_err();
        assert_eq!(
            err,
            DxfDecodeError::new(
                GroupCode::new(10).unwrap(),
                "abc",
                "invalid numeric value"
            )
        );

        let err = DxfToken::new(GroupCode::new(290).unwrap(), "2")
            .decode()
            .unwrap_err();
        assert_eq!(
            err,
            DxfDecodeError::new(GroupCode::new(290).unwrap(), "2", "expected boolean 0 or 1")
        );
    }

    #[test]
    fn roundtrip_minimal_document() {
        let doc = h7cad_native_model::CadDocument::new();
        let output = write_dxf(&doc).unwrap();
        let doc2 = read_dxf(&output).unwrap();
        assert_eq!(doc2.header.version, doc.header.version);
        assert_eq!(doc2.block_records.len(), doc.block_records.len());
    }

    #[test]
    fn roundtrip_with_entities() {
        use h7cad_native_model::*;
        let mut doc = CadDocument::new();
        doc.entities.push(Entity::new(EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [10.0, 20.0, 0.0],
        }));
        doc.entities.push(Entity::new(EntityData::Circle {
            center: [5.0, 5.0, 0.0],
            radius: 3.5,
        }));
        doc.entities.push(Entity::new(EntityData::Arc {
            center: [1.0, 2.0, 3.0],
            radius: 7.5,
            start_angle: 0.0,
            end_angle: 90.0,
        }));

        let output = write_dxf(&doc).unwrap();
        let doc2 = read_dxf(&output).unwrap();

        assert_eq!(doc2.entities.len(), 3, "should roundtrip 3 entities");
        match &doc2.entities[0].data {
            EntityData::Line { start, end } => {
                assert_eq!(*start, [0.0, 0.0, 0.0]);
                assert_eq!(*end, [10.0, 20.0, 0.0]);
            }
            other => panic!("expected Line, got {:?}", other),
        }
        match &doc2.entities[1].data {
            EntityData::Circle { center, radius } => {
                assert_eq!(*center, [5.0, 5.0, 0.0]);
                assert!((radius - 3.5).abs() < 1e-6);
            }
            other => panic!("expected Circle, got {:?}", other),
        }
        match &doc2.entities[2].data {
            EntityData::Arc { center, radius, start_angle, end_angle } => {
                assert_eq!(*center, [1.0, 2.0, 3.0]);
                assert!((radius - 7.5).abs() < 1e-6);
                assert!((start_angle - 0.0).abs() < 1e-6);
                assert!((end_angle - 90.0).abs() < 1e-6);
            }
            other => panic!("expected Arc, got {:?}", other),
        }
    }

    #[test]
    fn roundtrip_real_file_entity_count() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../",
            "../ACadSharp/samples/sample_AC1015_ascii.dxf"
        );
        let Ok(input) = std::fs::read_to_string(path) else {
            eprintln!("skipping: sample file not found at {path}");
            return;
        };
        let doc1 = read_dxf(&input).unwrap();
        let output = write_dxf(&doc1).unwrap();
        let doc2 = read_dxf(&output).unwrap();

        assert_eq!(
            doc2.header.version, doc1.header.version,
            "version mismatch"
        );
        assert_eq!(
            doc2.entities.len(),
            doc1.entities.len(),
            "entity count mismatch: wrote {} entities, read back {}",
            doc1.entities.len(),
            doc2.entities.len()
        );
        assert_eq!(
            doc2.layers.len(),
            doc1.layers.len(),
            "layer count mismatch"
        );

        let counts1 = doc1.entity_type_counts();
        let counts2 = doc2.entity_type_counts();
        for (typ, &c1) in &counts1 {
            let c2 = counts2.get(typ).copied().unwrap_or(0);
            assert_eq!(c2, c1, "type {typ}: wrote {c1}, read back {c2}");
        }
    }

    #[test]
    fn binary_dxf_sentinel_detected() {
        let mut data = Vec::new();
        data.extend_from_slice(b"AutoCAD Binary DXF\r\n\x1a\x00");
        // group 0, value "EOF\0"
        data.push(0u8);
        data.extend_from_slice(b"EOF\x00");
        let doc = read_dxf_bytes(&data).unwrap();
        assert!(doc.entities.is_empty());
    }

    #[test]
    fn legacy_encoding_fallback_windows_1252() {
        let latin1_bytes: &[u8] = b"  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$DWGCODEPAGE\n  3\nANSI_1252\n  0\nENDSEC\n  0\nEOF\n";
        let doc = read_dxf_bytes(latin1_bytes).unwrap();
        assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
    }
}
