mod error;
mod file_header;
mod object_reader;
mod pending;
mod reader;
mod resolver;
mod section_map;
mod version;

use h7cad_native_model::CadDocument;

pub use error::DwgReadError;
pub use file_header::DwgFileHeader;
pub use object_reader::{
    dispatch_entity_record, dispatch_object, dispatch_object_record, dispatch_table_record,
    record_index, record_payload_size, summarize_object, DispatchTarget, ParsedRecordSummary,
};
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
    let pending = build_pending_document(&header, &sections, payloads);
    Ok(resolve_document(&pending))
}

pub fn build_pending_document(
    header: &DwgFileHeader,
    sections: &SectionMap,
    payloads: Vec<Vec<u8>>,
) -> PendingDocument {
    let mut pending = PendingDocument::new(header.version, header.section_count);
    pending.sections = sections
        .descriptors
        .iter()
        .zip(payloads)
        .map(|descriptor| PendingSection {
            index: descriptor.0.index,
            offset: descriptor.0.offset,
            size: descriptor.0.size,
            record_count: classify_section_records(&descriptor.1).len() as u32,
            payload: descriptor.1,
        })
        .collect();
    pending.objects = pending
        .sections
        .iter()
        .enumerate()
        .flat_map(|(index, section)| {
            classify_section_records(&section.payload)
                .into_iter()
                .enumerate()
                .map(move |(record_index, record)| PendingObject {
                    handle: h7cad_native_model::Handle::new(
                        0x100 + index as u64 * 0x10 + record_index as u64,
                    ),
                    owner_handle: h7cad_native_model::Handle::NULL,
                    section_index: section.index,
                    kind: classify_record_kind(section.index, record_index as u32, &record),
                })
        })
        .collect();
    pending
}

pub fn classify_section_records(payload: &[u8]) -> Vec<Vec<u8>> {
    if payload.is_empty() {
        return Vec::new();
    }

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

pub fn classify_record_kind(
    section_index: u32,
    record_index: u32,
    record: &[u8],
) -> PendingObjectKind {
    let payload_size = record.len();
    match section_index % 3 {
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
    }
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
    fn parse_section_map_ac1018_extracts_descriptors() {
        let entries = [(0x20_u32, 0x40_u32), (0x60_u32, 0x10_u32)];
        let bytes = fixture_ac1018(entries.len() as u32, &entries, &[]);
        let header = DwgFileHeader::parse(&bytes).unwrap();
        let sections = SectionMap::parse(&bytes, &header).unwrap();

        assert_eq!(sections.version, DwgVersion::Ac1018);
        assert_eq!(sections.descriptors.len(), 2);
        assert_eq!(
            sections.descriptors[0],
            SectionDescriptor {
                index: 0,
                offset: 0x20,
                size: 0x40
            }
        );
        assert_eq!(
            sections.descriptors[1],
            SectionDescriptor {
                index: 1,
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
        let entries = [(0x21_u32, 0x20_u32)];
        let bytes = fixture_ac1015(1, &entries, &[&vec![1; 0x20]]);
        let header = DwgFileHeader::parse(&bytes).unwrap();
        let sections = SectionMap::parse(&bytes, &header).unwrap();
        let payloads = sections.read_section_payloads(&bytes).unwrap();
        let pending = build_pending_document(&header, &sections, payloads);

        assert_eq!(pending.version, DwgVersion::Ac1015);
        assert_eq!(pending.section_count, 1);
        assert_eq!(pending.objects.len(), 1);
        assert_eq!(
            pending.sections,
            vec![PendingSection {
                index: 0,
                offset: 0x21,
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
        let records = classify_section_records(b"ABC\0DE\0F");
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
    fn dispatch_object_routes_to_typed_entry_points() {
        let table = PendingObject {
            handle: h7cad_native_model::Handle::new(1),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 0,
            kind: PendingObjectKind::TableRecord {
                record_index: 0,
                payload_size: 1,
            },
        };
        let entity = PendingObject {
            handle: h7cad_native_model::Handle::new(2),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 1,
            kind: PendingObjectKind::EntityRecord {
                record_index: 0,
                payload_size: 1,
            },
        };
        let object = PendingObject {
            handle: h7cad_native_model::Handle::new(3),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 2,
            kind: PendingObjectKind::ObjectRecord {
                record_index: 0,
                payload_size: 1,
            },
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
            }
        );
    }

    #[test]
    fn resolve_document_uses_parsed_record_summary_naming() {
        let doc = read_dwg(&fixture_ac1018(1, &[(0x25, 0x02)], &[b"HI"])).unwrap();
        assert_eq!(doc.objects.len(), 1);
        match &doc.objects[0].data {
            h7cad_native_model::ObjectData::Unknown { object_type } => {
                assert_eq!(object_type, "DWG_TABLE_SECTION_0_RECORD_0_SIZE_2");
            }
            other => panic!("expected unknown object summary, got {other:?}"),
        }
    }

    fn fixture_ac1015(
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        fixture_with_layout(DwgVersion::Ac1015, 0x15, section_count, entries, payloads)
    }

    fn fixture_ac1018(
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        fixture_with_layout(DwgVersion::Ac1018, 0x19, section_count, entries, payloads)
    }

    fn fixture_with_layout(
        version: DwgVersion,
        section_count_offset: usize,
        section_count: u32,
        entries: &[(u32, u32)],
        payloads: &[&[u8]],
    ) -> Vec<u8> {
        let directory_end = section_count_offset + 4 + entries.len() * 8;
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
        for (offset, size) in entries {
            bytes[cursor..cursor + 4].copy_from_slice(&offset.to_le_bytes());
            bytes[cursor + 4..cursor + 8].copy_from_slice(&size.to_le_bytes());
            cursor += 8;
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
