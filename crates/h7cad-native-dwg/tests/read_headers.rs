use h7cad_native_dwg::{
    build_pending_document, classify_record_kind, classify_section_records, dispatch_object,
    read_dwg, record_payload_size, sniff_version, summarize_object, DispatchTarget, DwgFileHeader, DwgReadError, DwgVersion,
    ParsedRecordSummary, PendingObject, PendingObjectKind, SectionDescriptor, SectionMap,
};

#[test]
fn read_header_sniffs_known_versions() {
    assert_eq!(sniff_version(b"AC1015fixture").unwrap(), DwgVersion::Ac1015);
    assert_eq!(sniff_version(b"AC1018fixture").unwrap(), DwgVersion::Ac1018);
}

#[test]
fn read_header_rejects_unknown_magic() {
    assert_eq!(
        sniff_version(b"ZZ9999fixture").unwrap_err(),
        DwgReadError::InvalidMagic {
            found: "ZZ9999".to_string(),
        }
    );
}

#[test]
fn sniff_version_rejects_short_header_before_magic_parse() {
    assert_eq!(
        sniff_version(b"AC10").unwrap_err(),
        DwgReadError::TruncatedHeader { expected_at_least: 6 }
    );
}

#[test]
fn read_header_rejects_known_unsupported_versions_explicitly() {
    for version in [
        DwgVersion::Ac1021,
        DwgVersion::Ac1024,
        DwgVersion::Ac1027,
        DwgVersion::Ac1032,
    ] {
        let err = DwgFileHeader::parse(version.to_string().as_bytes()).unwrap_err();
        assert_eq!(err, DwgReadError::UnsupportedVersion(version));
    }
}

#[test]
fn read_dwg_returns_seeded_document_scaffold_for_supported_fixture() {
    let doc = read_dwg(&fixture_ac1018(1, &[(0x25, 0x03)], &[b"DWG"])).unwrap();
    assert_eq!(doc.model_space_handle().value(), 1);
    assert_eq!(doc.paper_space_handle().value(), 2);
    assert!(doc.block_records.contains_key(&doc.model_space_handle()));
    assert!(doc.block_records.contains_key(&doc.paper_space_handle()));
    let model_layout = doc.layout_by_name("Model").expect("model layout should exist");
    let paper_layout = doc
        .layout_by_name("Layout1")
        .expect("paper layout should exist");
    assert_eq!(model_layout.owner, doc.root_dictionary.handle);
    assert_eq!(paper_layout.owner, doc.root_dictionary.handle);
    assert_eq!(
        doc.block_records
            .get(&doc.model_space_handle())
            .and_then(|record| record.layout_handle),
        Some(model_layout.handle)
    );
    assert_eq!(
        doc.block_records
            .get(&doc.paper_space_handle())
            .and_then(|record| record.layout_handle),
        Some(paper_layout.handle)
    );
    assert_eq!(
        doc.root_dictionary
            .entries
            .get("ACAD_LAYOUT"),
        Some(&doc.root_dictionary.handle)
    );
    assert_eq!(
        doc.root_dictionary
            .entries
            .get(&format!("LAYOUT_{:X}", model_layout.handle.value())),
        Some(&model_layout.handle)
    );
    assert_eq!(
        doc.root_dictionary
            .entries
            .get(&format!("LAYOUT_{:X}", paper_layout.handle.value())),
        Some(&paper_layout.handle)
    );
}

#[test]
fn resolve_document_preserves_pending_layers_and_repairs_layout_links() {
    let mut pending = h7cad_native_dwg::PendingDocument::new(DwgVersion::Ac1018, 0);
    pending.layers.push(h7cad_native_dwg::PendingLayer {
        handle: h7cad_native_model::Handle::new(0x40),
        name: "Visible".to_string(),
    });

    let doc = h7cad_native_dwg::resolve_document(&pending);

    let model_layout = doc.layout_by_name("Model").expect("model layout should exist");
    let paper_layout = doc
        .layout_by_name("Layout1")
        .expect("paper layout should exist");
    assert!(doc.layers.contains_key("0"));
    assert!(doc.layers.contains_key("Visible"));
    assert_eq!(doc.tables.layer.entries.get("Visible"), Some(&h7cad_native_model::Handle::new(0x40)));
    assert_eq!(model_layout.owner, doc.root_dictionary.handle);
    assert_eq!(paper_layout.owner, doc.root_dictionary.handle);
    assert_eq!(
        doc.block_records
            .get(&model_layout.block_record_handle)
            .and_then(|record| record.layout_handle),
        Some(model_layout.handle)
    );
    assert_eq!(
        doc.block_records
            .get(&paper_layout.block_record_handle)
            .and_then(|record| record.layout_handle),
        Some(paper_layout.handle)
    );
    assert_eq!(
        doc.root_dictionary
            .entries
            .get(&format!("LAYOUT_{:X}", model_layout.handle.value())),
        Some(&model_layout.handle)
    );
    assert_eq!(
        doc.root_dictionary
            .entries
            .get(&format!("LAYOUT_{:X}", paper_layout.handle.value())),
        Some(&paper_layout.handle)
    );
}

#[test]
fn resolver_preserves_handles_owners_order_and_advances_allocation_state() {
    let mut pending = h7cad_native_dwg::PendingDocument::new(DwgVersion::Ac1018, 0);
    pending.objects = vec![
        PendingObject {
            handle: h7cad_native_model::Handle::new(0x30),
            owner_handle: h7cad_native_model::Handle::NULL,
            section_index: 0,
            kind: PendingObjectKind::TableRecord {
                record_index: 0,
                payload_size: 3,
            },
        },
        PendingObject {
            handle: h7cad_native_model::Handle::new(0x41),
            owner_handle: h7cad_native_model::Handle::new(0x90),
            section_index: 1,
            kind: PendingObjectKind::EntityRecord {
                record_index: 1,
                payload_size: 4,
            },
        },
        PendingObject {
            handle: h7cad_native_model::Handle::new(0x52),
            owner_handle: h7cad_native_model::Handle::new(0x91),
            section_index: 2,
            kind: PendingObjectKind::ObjectRecord {
                record_index: 2,
                payload_size: 5,
            },
        },
    ];

    let mut doc = h7cad_native_dwg::resolve_document(&pending);

    assert_eq!(doc.objects.len(), pending.objects.len());
    let resolved_projection = resolved_object_projection(&doc);
    assert_eq!(
        resolved_projection,
        vec![
            (0x30, 0, "DWG_TABLE_SECTION_0_RECORD_0_SIZE_3".to_string()),
            (0x41, 0x90, "DWG_ENTITY_SECTION_1_RECORD_1_SIZE_4".to_string()),
            (0x52, 0x91, "DWG_OBJECT_SECTION_2_RECORD_2_SIZE_5".to_string()),
        ]
    );
    assert_eq!(doc.next_handle(), 0x92);
    let allocated = doc.allocate_handle();
    assert_eq!(allocated.value(), 0x92);
    assert!(allocated.value() > 0x91);
}

#[test]
fn read_dwg_produces_deterministic_resolved_object_summaries() {
    let bytes = fixture_ac1018_with_pending_graph_payloads();

    let first = read_dwg(&bytes).unwrap();
    let second = read_dwg(&bytes).unwrap();

    assert_eq!(first.objects.len(), second.objects.len());
    assert_eq!(resolved_object_projection(&first), resolved_object_projection(&second));
}

#[test]
fn read_header_extracts_ac1015_section_count() {
    let bytes = fixture_ac1015(3, &[], &[]);
    let header = DwgFileHeader::parse(&bytes).unwrap();

    assert_eq!(header.version, DwgVersion::Ac1015);
    assert_eq!(header.section_count, 3);
}

#[test]
fn read_header_extracts_ac1018_section_count() {
    let bytes = fixture_ac1018(7, &[], &[]);
    let header = DwgFileHeader::parse(&bytes).unwrap();

    assert_eq!(header.version, DwgVersion::Ac1018);
    assert_eq!(header.section_count, 7);
}

#[test]
fn read_header_reports_ac1015_boundary_truncation() {
    let err = DwgFileHeader::parse(&truncated_supported_header(DwgVersion::Ac1015)).unwrap_err();

    assert_eq!(err, DwgReadError::TruncatedHeader { expected_at_least: 25 });
}

#[test]
fn read_header_reports_ac1018_boundary_truncation() {
    let err = DwgFileHeader::parse(&truncated_supported_header(DwgVersion::Ac1018)).unwrap_err();

    assert_eq!(err, DwgReadError::TruncatedHeader { expected_at_least: 29 });
}

#[test]
fn read_header_extracts_ac1018_section_descriptors() {
    let bytes = fixture_ac1018(2, &[(0x40, 0x20), (0x80, 0x08)], &[&vec![0; 0x20], &vec![0; 0x08]]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();

    assert_eq!(
        sections.descriptors,
        vec![
            SectionDescriptor {
                index: 0,
                offset: 0x40,
                size: 0x20,
            },
            SectionDescriptor {
                index: 1,
                offset: 0x80,
                size: 0x08,
            },
        ]
    );
}

#[test]
fn zero_section_fixture_decodes_to_empty_descriptor_and_payload_lists() {
    let bytes = fixture_ac1018(0, &[], &[]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();

    assert_eq!(header.section_count, 0);
    assert!(sections.descriptors.is_empty());
    assert!(payloads.is_empty());
}

#[test]
fn zero_size_descriptor_yields_empty_payload_bytes() {
    let bytes = fixture_ac1018(2, &[(0x40, 0), (0x44, 3)], &[b"", b"XYZ"]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();

    assert_eq!(
        sections.descriptors,
        vec![
            SectionDescriptor {
                index: 0,
                offset: 0x40,
                size: 0,
            },
            SectionDescriptor {
                index: 1,
                offset: 0x44,
                size: 3,
            },
        ]
    );
    assert_eq!(payloads, vec![Vec::<u8>::new(), b"XYZ".to_vec()]);
}

#[test]
fn payload_order_follows_directory_order_when_offsets_are_non_monotonic() {
    let bytes = fixture_ac1018(
        3,
        &[(0x80, 3), (0x40, 2), (0x60, 4)],
        &[b"top", b"lo", b"mid!"],
    );
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();

    assert_eq!(
        sections.descriptors,
        vec![
            SectionDescriptor {
                index: 0,
                offset: 0x80,
                size: 3,
            },
            SectionDescriptor {
                index: 1,
                offset: 0x40,
                size: 2,
            },
            SectionDescriptor {
                index: 2,
                offset: 0x60,
                size: 4,
            },
        ]
    );
    assert_eq!(payloads, vec![b"top".to_vec(), b"lo".to_vec(), b"mid!".to_vec()]);
}

#[test]
fn truncated_section_directory_fixture_fails_before_partial_map_is_returned() {
    let bytes = fixture_ac1018_truncated_directory(2, &[(0x40, 3)]);
    let err = DwgFileHeader::parse(&bytes)
        .and_then(|header| SectionMap::parse(&bytes, &header))
        .unwrap_err();

    assert_eq!(
        err,
        DwgReadError::TruncatedSectionDirectory {
            version: DwgVersion::Ac1018,
            expected_at_least: 45,
            actual: 37,
        }
    );
}

#[test]
fn out_of_bounds_descriptor_fixture_reports_failing_section_context() {
    let bytes = fixture_ac1015_with_declared_spans(2, &[(0x29, 3), (0x80, 5)], &[(0x29, b"abc")]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let err = sections.read_section_payloads(&bytes).unwrap_err();

    assert_eq!(
        err,
        DwgReadError::SectionOutOfBounds {
            index: 1,
            offset: 0x80,
            size: 5,
            actual: bytes.len(),
        }
    );
}

#[test]
fn pending_document_preserves_section_directory_entries() {
    let bytes = fixture_ac1018(2, &[(0x40, 0x20), (0x80, 0x08)], &[&vec![1; 0x20], &vec![2; 0x08]]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();
    let pending = build_pending_document(&header, &sections, payloads);

    assert_eq!(pending.section_count, 2);
    assert_eq!(pending.sections.len(), 2);
    assert_eq!(pending.objects.len(), 2);
    assert_eq!(pending.sections[0].record_count, 1);
    assert_eq!(pending.sections[0].payload, vec![1; 0x20]);
    assert_eq!(pending.sections[1].payload, vec![2; 0x08]);
}

#[test]
fn pending_document_section_accounting_matches_emitted_objects() {
    let bytes = fixture_ac1018_with_pending_graph_payloads();
    let pending = parse_pending_fixture(&bytes);

    assert_eq!(
        pending
            .sections
            .iter()
            .map(|section| (section.index, section.offset, section.size, section.record_count))
            .collect::<Vec<_>>(),
        vec![
            (0, 0x80, 0, 0),
            (1, 0x90, 3, 1),
            (2, 0xA3, 5, 1),
            (3, 0xB8, 5, 1),
            (4, 0xCD, 17, 3),
            (5, 0xEE, 10, 2),
        ]
    );
    assert_eq!(
        pending.sections.iter().map(|section| section.record_count).sum::<u32>() as usize,
        pending.objects.len()
    );

    for section in &pending.sections {
        let per_section_count = pending
            .objects
            .iter()
            .filter(|object| object.section_index == section.index)
            .count();
        assert_eq!(per_section_count, section.record_count as usize);
    }
}

#[test]
fn pending_document_uses_pending_graph_edge_case_fixtures() {
    let payloads = pending_graph_payloads();

    let bytes = fixture_ac1018_with_pending_graph_payloads();
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payload_bytes = sections.read_section_payloads(&bytes).unwrap();
    let pending = build_pending_document(&header, &sections, payload_bytes);

    assert_eq!(pending.section_count, payloads.len() as u32);
    assert_eq!(pending.sections.len(), payloads.len());
    assert_eq!(pending.sections[0].payload, payloads[0].to_vec());
    assert_eq!(pending.sections[1].payload, payloads[1].to_vec());
    assert_eq!(pending.sections[2].payload, payloads[2].to_vec());
    assert_eq!(pending.sections[3].payload, payloads[3].to_vec());
    assert_eq!(pending.sections[4].payload, payloads[4].to_vec());
    assert_eq!(pending.sections[5].payload, payloads[5].to_vec());
    assert_eq!(
        pending.sections.iter().map(|section| section.record_count).collect::<Vec<_>>(),
        vec![0, 1, 1, 1, 3, 2]
    );
    assert_eq!(pending.objects.len(), 8);
}

#[test]
fn pending_graph_fixture_payloads_cover_edge_cases() {
    let payloads = pending_graph_payloads();

    assert_eq!(classify_section_records(payloads[0]), Vec::<Vec<u8>>::new());
    assert_eq!(classify_section_records(payloads[1]), vec![b"\0\0\0".to_vec()]);
    assert_eq!(classify_section_records(payloads[2]), vec![b"solo".to_vec()]);
    assert_eq!(classify_section_records(payloads[3]), vec![b"tail".to_vec()]);
    assert_eq!(
        classify_section_records(payloads[4]),
        vec![b"alpha".to_vec(), b"beta".to_vec(), b"gamma".to_vec()]
    );
    assert_eq!(
        classify_section_records(payloads[5]),
        vec![b"left".to_vec(), b"right".to_vec()]
    );
}

#[test]
fn pending_object_section_mapping_covers_each_real_section() {
    let bytes = fixture_ac1018_with_pending_graph_payloads();
    let pending = parse_pending_fixture(&bytes);

    let mapped_sections = pending
        .objects
        .iter()
        .map(|object| {
            let section = pending
                .sections
                .iter()
                .find(|section| section.index == object.section_index)
                .expect("pending object should map to a real section");
            (object.section_index, record_payload_size(object), section.record_count)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        mapped_sections,
        vec![
            (1, 3, 1),
            (2, 4, 1),
            (3, 4, 1),
            (4, 5, 3),
            (4, 4, 3),
            (4, 5, 3),
            (5, 4, 2),
            (5, 5, 2),
        ]
    );
}

#[test]
fn repeated_pending_graph_fixture_parse_is_stable() {
    let bytes = fixture_ac1018_with_pending_graph_payloads();

    let first = parse_pending_fixture(&bytes);
    let second = parse_pending_fixture(&bytes);

    assert_eq!(first.sections, second.sections);
    assert_eq!(first.objects, second.objects);
}

#[test]
fn section_payloads_are_read_from_directory_offsets() {
    let bytes = fixture_ac1015(2, &[(0x29, 3), (0x40, 4)], &[b"abc", b"DEFG"]);
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();

    assert_eq!(payloads, vec![b"abc".to_vec(), b"DEFG".to_vec()]);
}

#[test]
fn record_classifier_splits_zero_delimited_payload() {
    assert_eq!(
        classify_section_records(b"one\0two\0three"),
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );
}

#[test]
fn typed_record_classifier_maps_section_buckets() {
    assert_eq!(
        classify_record_kind(0, 0, b"A"),
        PendingObjectKind::TableRecord {
            record_index: 0,
            payload_size: 1,
        }
    );
    assert_eq!(
        classify_record_kind(1, 1, b"BB"),
        PendingObjectKind::EntityRecord {
            record_index: 1,
            payload_size: 2,
        }
    );
    assert_eq!(
        classify_record_kind(2, 2, b"CCC"),
        PendingObjectKind::ObjectRecord {
            record_index: 2,
            payload_size: 3,
        }
    );
}

#[test]
fn dispatch_entry_points_follow_pending_object_kind() {
    let object = PendingObject {
        handle: h7cad_native_model::Handle::new(9),
        owner_handle: h7cad_native_model::Handle::NULL,
        section_index: 1,
        kind: PendingObjectKind::EntityRecord {
            record_index: 2,
            payload_size: 5,
        },
    };

    assert_eq!(dispatch_object(&object), DispatchTarget::Entity);
    assert_eq!(record_payload_size(&object), 5);
    assert_eq!(
        summarize_object(&object),
        ParsedRecordSummary {
            target: DispatchTarget::Entity,
            section_index: 1,
            record_index: 2,
            payload_size: 5,
        }
    );
}

fn parse_pending_fixture(bytes: &[u8]) -> h7cad_native_dwg::PendingDocument {
    let header = DwgFileHeader::parse(bytes).unwrap();
    let sections = SectionMap::parse(bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(bytes).unwrap();
    build_pending_document(&header, &sections, payloads)
}

fn resolved_object_projection(
    doc: &h7cad_native_model::CadDocument,
) -> Vec<(u64, u64, String)> {
    doc.objects
        .iter()
        .map(|object| {
            let object_type = match &object.data {
                h7cad_native_model::ObjectData::Unknown { object_type } => object_type.clone(),
                other => panic!("expected unknown object summary, got {other:?}"),
            };
            (
                object.handle.value(),
                object.owner_handle.value(),
                object_type,
            )
        })
        .collect()
}

fn fixture_ac1018_with_pending_graph_payloads() -> Vec<u8> {
    let payloads = pending_graph_payloads();
    let entries = pending_graph_entries(&payloads);
    fixture_ac1018(payloads.len() as u32, &entries, &payloads)
}

fn pending_graph_payloads() -> Vec<&'static [u8]> {
    vec![
        b"",
        b"\0\0\0",
        b"\0solo",
        b"tail\0",
        b"alpha\0\0beta\0gamma",
        b"left\0right",
    ]
}

fn pending_graph_entries(payloads: &[&[u8]]) -> Vec<(u32, u32)> {
    let mut next_offset = 0x80_u32;
    payloads
        .iter()
        .map(|payload| {
            let entry = (next_offset, payload.len() as u32);
            next_offset += payload.len() as u32 + 0x10;
            entry
        })
        .collect()
}

fn fixture_ac1015(section_count: u32, entries: &[(u32, u32)], payloads: &[&[u8]]) -> Vec<u8> {
    fixture_with_layout(DwgVersion::Ac1015, 0x15, section_count, entries, payloads)
}

fn fixture_ac1018(section_count: u32, entries: &[(u32, u32)], payloads: &[&[u8]]) -> Vec<u8> {
    fixture_with_layout(DwgVersion::Ac1018, 0x19, section_count, entries, payloads)
}

fn fixture_ac1015_with_declared_spans(
    section_count: u32,
    entries: &[(u32, u32)],
    payloads: &[(u32, &[u8])],
) -> Vec<u8> {
    fixture_with_sparse_layout(DwgVersion::Ac1015, 0x15, section_count, entries, payloads)
}

fn fixture_ac1018_truncated_directory(section_count: u32, entries: &[(u32, u32)]) -> Vec<u8> {
    let directory_bytes = 0x19 + 4 + entries.len() * 8;
    let mut bytes = vec![0; directory_bytes];
    bytes[..6].copy_from_slice(DwgVersion::Ac1018.to_string().as_bytes());
    bytes[0x19..0x19 + 4].copy_from_slice(&section_count.to_le_bytes());
    let mut cursor = 0x19 + 4;
    for (offset, size) in entries {
        bytes[cursor..cursor + 4].copy_from_slice(&offset.to_le_bytes());
        bytes[cursor + 4..cursor + 8].copy_from_slice(&size.to_le_bytes());
        cursor += 8;
    }
    bytes
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
    bytes[..6].copy_from_slice(version.to_string().as_bytes());
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

fn fixture_with_sparse_layout(
    version: DwgVersion,
    section_count_offset: usize,
    section_count: u32,
    entries: &[(u32, u32)],
    payloads: &[(u32, &[u8])],
) -> Vec<u8> {
    let directory_end = section_count_offset + 4 + entries.len() * 8;
    let max_end = payloads
        .iter()
        .map(|(offset, payload)| *offset as usize + payload.len())
        .max()
        .unwrap_or(directory_end);
    let mut bytes = vec![0; directory_end.max(max_end)];
    bytes[..6].copy_from_slice(version.to_string().as_bytes());
    bytes[section_count_offset..section_count_offset + 4]
        .copy_from_slice(&section_count.to_le_bytes());

    let mut cursor = section_count_offset + 4;
    for (offset, size) in entries {
        bytes[cursor..cursor + 4].copy_from_slice(&offset.to_le_bytes());
        bytes[cursor + 4..cursor + 8].copy_from_slice(&size.to_le_bytes());
        cursor += 8;
    }

    for (offset, payload) in payloads {
        let start = *offset as usize;
        bytes[start..start + payload.len()].copy_from_slice(payload);
    }

    bytes
}

fn truncated_supported_header(version: DwgVersion) -> Vec<u8> {
    match version {
        DwgVersion::Ac1015 => version.to_string().as_bytes()[..].to_vec(),
        DwgVersion::Ac1018 => {
            let mut bytes = vec![0; 28];
            bytes[..6].copy_from_slice(version.to_string().as_bytes());
            bytes
        }
        other => panic!("unsupported test fixture version: {other:?}"),
    }
}
