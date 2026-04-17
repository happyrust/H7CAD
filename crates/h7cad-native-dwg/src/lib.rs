mod bit_reader;
mod error;
mod file_header;
mod handle_map;
mod known_section;
mod modular;
mod object_reader;
mod object_stream;
mod pending;
mod reader;
mod resolver;
mod section_map;
mod version;

use h7cad_native_model::CadDocument;
use h7cad_native_model::Handle;

pub use bit_reader::BitReader;
pub use error::DwgReadError;
pub use file_header::DwgFileHeader;
pub use handle_map::{parse_handle_map, HandleMapEntry};
pub use known_section::KnownSection;
pub use object_reader::{
    dispatch_entity_record, dispatch_object, dispatch_object_record, dispatch_table_record,
    record_index, record_payload_size, summarize_object, DispatchTarget, ParsedRecordSummary,
};
pub use object_stream::ObjectStreamCursor;
pub use pending::{
    PendingDocument, PendingEntity, PendingLayer, PendingObject, PendingObjectKind, PendingSection,
};
pub use reader::DwgReaderCursor;
pub use resolver::resolve_document;
pub use section_map::{SectionDescriptor, SectionMap};
pub use version::DwgVersion;

pub fn sniff_version(bytes: &[u8]) -> Result<DwgVersion, DwgReadError> {
    let magic = bytes
        .get(..6)
        .ok_or(DwgReadError::TruncatedHeader { expected_at_least: 6 })?;
    let magic = std::str::from_utf8(magic).map_err(|_| DwgReadError::InvalidMagic {
        found: String::from_utf8_lossy(magic).into_owned(),
    })?;
    DwgVersion::from_magic(magic)
}

pub fn read_dwg(bytes: &[u8]) -> Result<CadDocument, DwgReadError> {
    let header = DwgFileHeader::parse(bytes)?;
    let sections = SectionMap::parse(bytes, &header)?;
    let payloads = sections.read_section_payloads(bytes)?;
    let pending = build_pending_document(&header, &sections, payloads)?;
    resolve_document(&pending)
}

pub fn build_pending_document(
    header: &DwgFileHeader,
    sections: &SectionMap,
    payloads: Vec<Vec<u8>>,
) -> Result<PendingDocument, DwgReadError> {
    let mut pending = PendingDocument::new(header.version, header.section_count);
    // Decode the `AcDb:Handles` section up front. It is byte-aligned and
    // shares nothing with the bit-stream pipelines, so pulling it out
    // of the generic record classifier keeps the rest of this function
    // unaffected. Fault-tolerant by design: synthetic test fixtures can
    // emit a record_number == 2 slot whose payload is not a real handle
    // map, and a partially corrupt Handle chunk in the wild should still
    // let the rest of the document resolve. Both cases degrade to "no
    // handle_offsets decoded" instead of failing the whole document.
    for (descriptor, payload) in sections.descriptors.iter().zip(payloads.iter()) {
        if KnownSection::from_record_number(descriptor.record_number) != Some(KnownSection::Handles)
        {
            continue;
        }
        if payload.is_empty() {
            continue;
        }
        if let Ok(entries) = parse_handle_map(payload) {
            pending.handle_offsets.extend(entries);
        }
    }
    let semantic_layers = payloads
        .iter()
        .flat_map(|payload| collect_semantic_layers(payload))
        .collect::<Vec<_>>();
    let layer_by_handle = semantic_layers
        .iter()
        .map(|layer| (layer.handle, layer.name.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let section_records = sections
        .descriptors
        .iter()
        .zip(payloads)
        .map(|descriptor| {
            let records = classify_section_records_for_section(descriptor.0.index, &descriptor.1)?;
            let record_count = records.len() as u32;
            Ok((
                PendingSection {
                    index: descriptor.0.index,
                    offset: descriptor.0.offset,
                    size: descriptor.0.size,
                    record_count,
                    payload: descriptor.1,
                },
                records,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    pending.sections = section_records
        .iter()
        .map(|(section, _)| section.clone())
        .collect();
    pending.objects = section_records
        .iter()
        .enumerate()
        .map(|(index, (section, records))| {
            records
                .iter()
                .enumerate()
                .map(|(record_index, record)| PendingObject {
                    handle: semantic_handle(record).unwrap_or_else(|| {
                        Handle::new(0x100 + index as u64 * 0x10 + record_index as u64)
                    }),
                    owner_handle: semantic_owner_handle(record).unwrap_or(Handle::NULL),
                    section_index: section.index,
                    kind: classify_record_kind(section.index, record_index as u32, record),
                    semantic_identity: semantic_identity(record),
                    semantic_link: semantic_link(record),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();
    pending.layers = section_records
        .iter()
        .flat_map(|(_, records)| records.iter().filter_map(|record| semantic_layer(record)))
        .chain(semantic_layers)
        .fold(Vec::<PendingLayer>::new(), |mut layers, layer| {
            if !layers.iter().any(|existing| existing.handle == layer.handle) {
                layers.push(layer);
            }
            layers
        });
    pending.entities = section_records
        .iter()
        .flat_map(|(_, records)| {
            records
                .iter()
                .filter_map(|record| semantic_entity(record, &layer_by_handle))
        })
        .collect();
    Ok(pending)
}

pub fn classify_section_records(payload: &[u8]) -> Result<Vec<Vec<u8>>, DwgReadError> {
    classify_section_records_for_section(0, payload)
}

fn classify_section_records_for_section(
    section_index: u32,
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, DwgReadError> {
    if payload.is_empty() {
        return Ok(Vec::new());
    }

    if contains_semantic_record_prefix(payload) {
        return decode_semantic_section_records(section_index, payload);
    }

    Ok(split_zero_delimited_records(payload))
}

pub fn classify_record_kind(
    section_index: u32,
    record_index: u32,
    record: &[u8],
) -> PendingObjectKind {
    let payload_size = record.len();
    match semantic_record_category(record) {
        Some(SemanticRecordCategory::Table) => PendingObjectKind::TableRecord {
            record_index,
            payload_size,
        },
        Some(SemanticRecordCategory::Entity) => PendingObjectKind::EntityRecord {
            record_index,
            payload_size,
        },
        Some(SemanticRecordCategory::Object) => PendingObjectKind::ObjectRecord {
            record_index,
            payload_size,
        },
        None => match section_index % 3 {
            0 => PendingObjectKind::TableRecord {
                record_index,
                payload_size,
            },
            1 => PendingObjectKind::EntityRecord {
                record_index,
                payload_size,
            },
            _ => PendingObjectKind::ObjectRecord {
                record_index,
                payload_size,
            },
        },
    }
}

#[derive(Clone, Copy)]
enum SemanticRecordCategory {
    Table,
    Entity,
    Object,
}

fn contains_semantic_record_prefix(payload: &[u8]) -> bool {
    payload
        .windows(4)
        .any(|window| matches!(window, b"TBL:" | b"ENT:" | b"OBJ:"))
}

fn decode_semantic_section_records(
    section_index: u32,
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, DwgReadError> {
    let semantic_start = find_first_semantic_prefix(payload).unwrap_or(0);
    let payload = &payload[semantic_start..];
    let mut records = Vec::new();
    let mut current = Vec::new();
    let mut index = 0usize;

    while index < payload.len() {
        if payload[index] == 0 {
            let tail = &payload[index + 1..];
            if tail.starts_with(b"TBL:") || tail.starts_with(b"ENT:") || tail.starts_with(b"OBJ:")
            {
                if current.is_empty() {
                    return Err(DwgReadError::SemanticDecode {
                        section_index,
                        record_index: records.len() as u32,
                        reason: "encountered semantic delimiter before record content".to_string(),
                    });
                }
                validate_semantic_record(section_index, records.len() as u32, &current)?;
                records.push(std::mem::take(&mut current));
                index += 1;
                continue;
            }
        }
        current.push(payload[index]);
        index += 1;
    }

    if !current.is_empty() {
        validate_semantic_record(section_index, records.len() as u32, &current)?;
        records.push(current);
    }

    if records.is_empty() && payload.iter().any(|byte| *byte == 0) {
        return Ok(vec![payload.to_vec()]);
    }

    Ok(records)
}

fn find_first_semantic_prefix(payload: &[u8]) -> Option<usize> {
    payload
        .windows(4)
        .position(|window| matches!(window, b"TBL:" | b"ENT:" | b"OBJ:"))
}

fn split_zero_delimited_records(payload: &[u8]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    let mut start = 0usize;
    for (idx, byte) in payload.iter().enumerate() {
        if *byte == 0 {
            if start < idx {
                records.push(payload[start..idx].to_vec());
            }
            start = idx + 1;
        }
    }
    if start < payload.len() {
        records.push(payload[start..].to_vec());
    }
    if records.is_empty() {
        records.push(payload.to_vec());
    }
    records
}

fn validate_semantic_record(
    section_index: u32,
    record_index: u32,
    record: &[u8],
) -> Result<(), DwgReadError> {
    if semantic_record_category(record).is_none() {
        return Ok(());
    }

    let text = std::str::from_utf8(record).map_err(|_| DwgReadError::SemanticDecode {
        section_index,
        record_index,
        reason: "semantic record is not valid UTF-8".to_string(),
    })?;
    let parts = text.split(':').collect::<Vec<_>>();
    if parts.len() < 4 {
        return Err(DwgReadError::SemanticDecode {
            section_index,
            record_index,
            reason: "semantic record is truncated".to_string(),
        });
    }

    let handle_fragment = parts
        .iter()
        .rev()
        .find(|part| part.starts_with('H') || part.starts_with('E'))
        .copied();
    if let Some(fragment) = handle_fragment {
        if fragment.len() < 2 || !fragment[1..].chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(DwgReadError::SemanticDecode {
                section_index,
                record_index,
                reason: format!("invalid semantic handle fragment `{fragment}`"),
            });
        }
    } else {
        return Err(DwgReadError::SemanticDecode {
            section_index,
            record_index,
            reason: "semantic record is missing a handle fragment".to_string(),
        });
    }

    let owner_fragment = parts
        .iter()
        .find(|part| part.starts_with('O') && part.len() > 1 && part.as_bytes()[1] != b'B')
        .copied();
    if let Some(owner_fragment) = owner_fragment {
        if owner_fragment.len() < 2 || !owner_fragment[1..].chars().all(|ch| ch.is_ascii_hexdigit())
        {
            return Err(DwgReadError::SemanticDecode {
                section_index,
                record_index,
                reason: format!("invalid semantic owner fragment `{owner_fragment}`"),
            });
        }
    }

    Ok(())
}

fn semantic_record_category(record: &[u8]) -> Option<SemanticRecordCategory> {
    if record.starts_with(b"TBL:") {
        Some(SemanticRecordCategory::Table)
    } else if record.starts_with(b"ENT:") {
        Some(SemanticRecordCategory::Entity)
    } else if record.starts_with(b"OBJ:") {
        Some(SemanticRecordCategory::Object)
    } else {
        None
    }
}

fn semantic_fields(record: &[u8]) -> Option<Vec<&str>> {
    semantic_record_category(record)?;
    std::str::from_utf8(record)
        .ok()
        .map(|text| text.split(':').collect::<Vec<_>>())
}

fn parse_handle_fragment(fragment: &str, prefix: char) -> Option<Handle> {
    let rest = fragment.strip_prefix(prefix)?;
    u64::from_str_radix(rest, 16).ok().map(Handle::new)
}

fn semantic_handle(record: &[u8]) -> Option<Handle> {
    let fields = semantic_fields(record)?;
    fields
        .iter()
        .rev()
        .find_map(|field| parse_handle_fragment(field, 'H').or_else(|| parse_handle_fragment(field, 'E')))
}

fn semantic_owner_handle(record: &[u8]) -> Option<Handle> {
    let fields = semantic_fields(record)?;
    fields.iter().find_map(|field| {
        if field.starts_with('O') && !field.starts_with("OBJ") {
            parse_handle_fragment(field, 'O')
        } else {
            None
        }
    })
}

fn semantic_layer(record: &[u8]) -> Option<PendingLayer> {
    let fields = semantic_fields(record)?;
    if fields.first().copied()? != "TBL" || fields.get(1).copied()? != "LAYER" {
        return None;
    }
    Some(PendingLayer {
        handle: semantic_handle(record)?,
        name: fields.get(2)?.to_string(),
    })
}

fn collect_semantic_layers(payload: &[u8]) -> Vec<PendingLayer> {
    let mut layers = Vec::new();
    let mut index = 0usize;
    while let Some(start) = payload[index..]
        .windows(10)
        .position(|window| window == b"TBL:LAYER:")
    {
        let start = index + start;
        let bytes = &payload[start..];
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        let candidate = &bytes[..end];
        if let Some(layer) = semantic_layer(candidate) {
            layers.push(layer);
        }
        index = start + end;
        if index >= payload.len() {
            break;
        }
    }
    layers
}

fn semantic_entity(
    record: &[u8],
    layer_by_handle: &std::collections::BTreeMap<Handle, String>,
) -> Option<PendingEntity> {
    let fields = semantic_fields(record)?;
    if fields.first().copied()? != "ENT" {
        return None;
    }
    let layer_handle = semantic_layer_handle(record);
    let layer_name = layer_handle
        .and_then(|handle| {
            layer_by_handle
                .get(&handle)
                .cloned()
                .or_else(|| semantic_layer_name_from_fields(&fields, handle))
        })
        .or_else(|| semantic_inline_layer_name(&fields))
        .unwrap_or_default();
    Some(PendingEntity {
        handle: semantic_handle(record)?,
        owner_handle: semantic_owner_handle(record).unwrap_or(Handle::NULL),
        layer_name,
    })
}

fn semantic_identity(record: &[u8]) -> Option<String> {
    let fields = semantic_fields(record)?;
    match fields.first().copied()? {
        "TBL" => Some(format!("table:{}:{}", fields.get(1)?, fields.get(2)?)),
        "ENT" => Some(format!("entity:{}", fields.get(1)?)),
        "OBJ" => match fields.get(1).copied()? {
            "BLOCK" => Some(format!("block:{}", fields.get(2)?)),
            "LAYOUT" => Some(format!("layout:{}", fields.get(2)?)),
            other => Some(format!("object:{other}")),
        },
        _ => None,
    }
}

fn semantic_link(record: &[u8]) -> Option<String> {
    let fields = semantic_fields(record)?;
    match fields.first().copied()? {
        "TBL" => Some(format!("handle:{:X}", semantic_handle(record)?.value())),
        "ENT" => {
            let layer_handle = semantic_layer_handle(record);
            let layer = layer_handle
                .and_then(|handle| semantic_layer_name_from_fields(&fields, handle))
                .or_else(|| semantic_inline_layer_name(&fields));
            let owner = semantic_owner_handle(record)
                .filter(|handle| *handle != Handle::NULL)
                .map(|handle| format!("owner:{:X}", handle.value()));
            let layer_handle = layer_handle.map(|handle| format!("layer_handle:{:X}", handle.value()));
            let layer = layer.map(|layer| format!("layer:{layer}"));
            let mut parts = Vec::new();
            if let Some(layer_handle) = layer_handle {
                parts.push(layer_handle);
            }
            if let Some(layer) = layer {
                parts.push(layer);
            }
            if let Some(owner) = owner {
                parts.push(owner);
            }
            match parts.is_empty() {
                true => None,
                false => Some(parts.join("|")),
            }
        }
        "OBJ" => match fields.get(1).copied()? {
            "BLOCK" => fields
                .iter()
                .skip(2)
                .find_map(|field| field.strip_prefix("LAYOUT="))
                .map(|layout| format!("layout:{layout}")),
            "LAYOUT" => fields
                .iter()
                .skip(2)
                .find_map(|field| field.strip_prefix('B'))
                .filter(|handle| handle.chars().all(|ch| ch.is_ascii_hexdigit()))
                .map(|handle| format!("block_handle:{handle}")),
            other => Some(format!("object:{other}")),
        },
        _ => None,
    }
}

fn semantic_layer_handle(record: &[u8]) -> Option<Handle> {
    let fields = semantic_fields(record)?;
    fields.iter().find_map(|field| {
        field
            .strip_prefix("LR")
            .and_then(|value| u64::from_str_radix(value, 16).ok())
            .map(Handle::new)
    })
}

fn semantic_inline_layer_name(fields: &[&str]) -> Option<String> {
    fields
        .iter()
        .skip(2)
        .find_map(|field| field.strip_prefix('L'))
        .filter(|layer| !layer.is_empty() && !layer.starts_with('R'))
        .map(ToString::to_string)
}

fn semantic_layer_name_from_fields(fields: &[&str], handle: Handle) -> Option<String> {
    let handle_hex = format!("{:X}", handle.value());
    fields
        .iter()
        .position(|field| *field == "LAYER")
        .and_then(|pos| fields.get(pos + 1).copied())
        .filter(|_| {
            fields
                .iter()
                .find_map(|field| field.strip_prefix('H'))
                .is_some_and(|value| value == handle_hex)
        })
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_version_accepts_supported_baseline_versions() {
        assert_eq!(sniff_version(b"AC1015rest").unwrap(), DwgVersion::Ac1015);
        assert_eq!(sniff_version(b"AC1018rest").unwrap(), DwgVersion::Ac1018);
    }

    #[test]
    fn sniff_version_rejects_short_headers() {
        assert_eq!(
            sniff_version(b"AC10").unwrap_err(),
            DwgReadError::TruncatedHeader { expected_at_least: 6 }
        );
    }

    #[test]
    fn read_dwg_returns_minimal_native_document_for_known_versions() {
        let doc = read_dwg(&fixture_ac1015(1, &[(0x25, 0x03)], &[b"ABC"])).unwrap();
        assert_eq!(doc.header.handseed, 6);
        assert_eq!(doc.model_space_handle().value(), 1);
        assert_eq!(doc.objects.len(), 1);
    }

    #[test]
    fn parse_file_header_ac1015_extracts_section_count() {
        let bytes = fixture_ac1015(2, &[], &[]);
        let header = DwgFileHeader::parse(&bytes).unwrap();

        assert_eq!(header.version, DwgVersion::Ac1015);
        assert_eq!(header.section_count, 2);
    }

    #[test]
    fn parse_section_map_extracts_descriptors() {
        let entries = [(0x20_u32, 0x40_u32), (0x60_u32, 0x10_u32)];
        let bytes = fixture_ac1015(entries.len() as u32, &entries, &[]);
        let header = DwgFileHeader::parse(&bytes).unwrap();
        let sections = SectionMap::parse(&bytes, &header).unwrap();

        assert_eq!(sections.version, DwgVersion::Ac1015);
        assert_eq!(sections.descriptors.len(), 2);
        assert_eq!(
            sections.descriptors[0],
            SectionDescriptor {
                index: 0,
                record_number: 0,
                offset: 0x20,
                size: 0x40
            }
        );
        assert_eq!(
            sections.descriptors[1],
            SectionDescriptor {
                index: 1,
                record_number: 1,
                offset: 0x60,
                size: 0x10
            }
        );
    }

    #[test]
    fn reader_cursor_reads_bytes_and_u32() {
        let mut reader = DwgReaderCursor::new(DwgVersion::Ac1015, &[0xAA, 0x78, 0x56, 0x34, 0x12]);
        assert_eq!(reader.read_u8().unwrap(), 0xAA);
        assert_eq!(reader.read_u32_le().unwrap(), 0x12345678);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn build_pending_document_keeps_section_metadata() {
        let entries = [(0x40_u32, 0x20_u32)];
        let bytes = fixture_ac1015(1, &entries, &[&vec![1; 0x20]]);
        let header = DwgFileHeader::parse(&bytes).unwrap();
        let sections = SectionMap::parse(&bytes, &header).unwrap();
        let payloads = sections.read_section_payloads(&bytes).unwrap();
        let pending = build_pending_document(&header, &sections, payloads).unwrap();

        assert_eq!(pending.version, DwgVersion::Ac1015);
        assert_eq!(pending.section_count, 1);
        assert_eq!(pending.objects.len(), 1);
        assert_eq!(
            pending.sections,
            vec![PendingSection {
                index: 0,
                offset: 0x40,
                size: 0x20,
                record_count: 1,
                payload: vec![1; 0x20],
            }]
        );
        assert_eq!(
            pending.objects,
            vec![PendingObject {
                handle: h7cad_native_model::Handle::new(0x100),
                owner_handle: h7cad_native_model::Handle::NULL,
                section_index: 0,
                kind: PendingObjectKind::TableRecord {
                    record_index: 0,
                    payload_size: 0x20,
                },
                semantic_identity: None,
                semantic_link: None,
            }]
        );
    }

    #[test]
    fn section_map_reads_payload_bytes() {
        let bytes = fixture_ac1018(2, &[(0x30, 3), (0x40, 2)], &[b"xyz", b"OK"]);
        let header = DwgFileHeader::parse(&bytes).unwrap();
        let sections = SectionMap::parse(&bytes, &header).unwrap();
        let payloads = sections.read_section_payloads(&bytes).unwrap();

        assert_eq!(payloads, vec![b"xyz".to_vec(), b"OK".to_vec()]);
    }

    #[test]
    fn classify_section_records_splits_on_zero_delimiters() {
        let records = classify_section_records(b"ABC\0DE\0F").unwrap();
        assert_eq!(records, vec![b"ABC".to_vec(), b"DE".to_vec(), b"F".to_vec()]);
    }

    #[test]
    fn classify_record_kind_uses_section_index_buckets() {
        assert_eq!(
            classify_record_kind(0, 2, b"AA"),
            PendingObjectKind::TableRecord {
                record_index: 2,
                payload_size: 2,
            }
        );
        assert_eq!(
            classify_record_kind(1, 3, b"BBB"),
            PendingObjectKind::EntityRecord {
                record_index: 3,
                payload_size: 3,
            }
        );
        assert_eq!(
            classify_record_kind(2, 4, b"CCCC"),
            PendingObjectKind::ObjectRecord {
                record_index: 4,
                payload_size: 4,
            }
        );
    }

    #[test]
    fn classify_section_records_decodes_semantic_boundaries_without_splitting_embedded_zero() {
        let records = decode_semantic_section_records(
            4,
            b"OBJ:TEXT:Zero\0Payload:H44:O22\0ENT:ARC:E44:O22:LLayerZero",
        )
        .unwrap();
        assert_eq!(
            records,
            vec![
                b"OBJ:TEXT:Zero\0Payload:H44:O22".to_vec(),
                b"ENT:ARC:E44:O22:LLayerZero".to_vec()
            ]
        );
    }

    #[test]
    fn classify_section_records_rejects_structurally_valid_semantic_corruption() {
        let err = decode_semantic_section_records(1, b"OBJ:LAYOUT:Broken:H95:BFF\0ENT:LINE:EXX:OFF")
            .unwrap_err();
        assert_eq!(
            err,
            DwgReadError::SemanticDecode {
                section_index: 1,
                record_index: 1,
                reason: "invalid semantic handle fragment `EXX`".to_string(),
            }
        );
    }

    #[test]
    fn classify_section_records_preserves_nonsemantic_zero_delimiter_behavior() {
        let records = classify_section_records(b"ABC\0DE\0F").unwrap();
        assert_eq!(records, vec![b"ABC".to_vec(), b"DE".to_vec(), b"F".to_vec()]);
    }

    #[test]
    fn dispatch_object_routes_to_typed_entry_points() {
        let table = PendingObject {
            handle: h7cad_native_model::Handle::new(1),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 0,
            kind: PendingObjectKind::TableRecord {
                record_index: 0,
                payload_size: 1,
            },
            semantic_identity: None,
            semantic_link: None,
        };
        let entity = PendingObject {
            handle: h7cad_native_model::Handle::new(2),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 1,
            kind: PendingObjectKind::EntityRecord {
                record_index: 0,
                payload_size: 1,
            },
            semantic_identity: None,
            semantic_link: None,
        };
        let object = PendingObject {
            handle: h7cad_native_model::Handle::new(3),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 2,
            kind: PendingObjectKind::ObjectRecord {
                record_index: 0,
                payload_size: 1,
            },
            semantic_identity: None,
            semantic_link: None,
        };

        assert_eq!(dispatch_object(&table), DispatchTarget::Table);
        assert_eq!(dispatch_object(&entity), DispatchTarget::Entity);
        assert_eq!(dispatch_object(&object), DispatchTarget::Object);
        assert_eq!(record_payload_size(&table), 1);
        assert_eq!(record_payload_size(&entity), 1);
        assert_eq!(record_payload_size(&object), 1);
        assert_eq!(record_index(&table), 0);
        assert_eq!(
            summarize_object(&entity),
            ParsedRecordSummary {
                target: DispatchTarget::Entity,
                section_index: 1,
                record_index: 0,
                payload_size: 1,
                semantic_identity: "entity".to_string(),
                semantic_link: String::new(),
            }
        );
    }

    #[test]
    fn resolve_document_uses_parsed_record_summary_naming() {
        let doc = read_dwg(&fixture_ac1018(1, &[(0x25, 0x02)], &[b"HI"])).unwrap();
        assert_eq!(doc.objects.len(), 1);
        match &doc.objects[0].data {
            h7cad_native_model::ObjectData::Unknown { object_type } => {
                assert_eq!(object_type, "DWG_TABLE_SECTION_0_RECORD_0_SIZE_2_TABLE_");
            }
            other => panic!("expected unknown object summary, got {other:?}"),
        }
    }

    fn fixture_ac1015(
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        fixture_with_layout(DwgVersion::Ac1015, section_count, entries, payloads)
    }

    /// Legacy alias. Historic tests called `fixture_ac1018`, but the
    /// synthetic byte layout never matched real AC1018 structure. All
    /// such fixtures are now routed through the AC1015 layout so they
    /// keep exercising the section-map + pending-graph code paths.
    fn fixture_ac1018(
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        fixture_with_layout(DwgVersion::Ac1015, section_count, entries, payloads)
    }

    fn fixture_with_layout(
        version: DwgVersion,
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        let section_count_offset = crate::file_header::section_count_offset(version)
            .expect("fixture version must be supported by file header decoder");
        // AC1015 section locator records are 9 bytes each.
        let record_size = 9usize;
        let directory_end = section_count_offset + 4 + entries.len() * record_size;
        let max_end = entries
            .iter()
            .map(|(offset, size)| *offset as usize + *size as usize)
            .max()
            .unwrap_or(directory_end);
        let mut bytes = vec![0; directory_end.max(max_end)];
        let magic = version.to_string();
        bytes[..6].copy_from_slice(magic.as_bytes());
        bytes[section_count_offset..section_count_offset + 4]
            .copy_from_slice(&section_count.to_le_bytes());

        let mut cursor = section_count_offset + 4;
        for (index, (offset, size)) in entries.iter().enumerate() {
            bytes[cursor] = index as u8;
            bytes[cursor + 1..cursor + 5].copy_from_slice(&offset.to_le_bytes());
            bytes[cursor + 5..cursor + 9].copy_from_slice(&size.to_le_bytes());
            cursor += record_size;
        }

        for ((offset, size), payload) in entries.iter().zip(payloads.iter()) {
            let start = *offset as usize;
            let expected = *size as usize;
            assert_eq!(payload.len(), expected);
            bytes[start..start + expected].copy_from_slice(payload);
        }
        bytes
    }
}
