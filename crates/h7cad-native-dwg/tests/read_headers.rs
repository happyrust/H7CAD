use h7cad_native_dwg::{
    build_pending_document, classify_record_kind, classify_section_records, dispatch_object,
    read_dwg, record_payload_size, sniff_version, summarize_object, DispatchTarget, DwgFileHeader,
    DwgReadError, DwgVersion, ParsedRecordSummary, PendingObject, PendingObjectKind,
    SectionDescriptor, SectionMap,
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
            semantic_identity: None,
            semantic_link: None,
        },
        PendingObject {
            handle: h7cad_native_model::Handle::new(0x41),
            owner_handle: h7cad_native_model::Handle::new(0x90),
            section_index: 1,
            kind: PendingObjectKind::EntityRecord {
                record_index: 1,
                payload_size: 4,
            },
            semantic_identity: None,
            semantic_link: None,
        },
        PendingObject {
            handle: h7cad_native_model::Handle::new(0x52),
            owner_handle: h7cad_native_model::Handle::new(0x91),
            section_index: 2,
            kind: PendingObjectKind::ObjectRecord {
                record_index: 2,
                payload_size: 5,
            },
            semantic_identity: None,
            semantic_link: None,
        },
    ];

    let mut doc = h7cad_native_dwg::resolve_document(&pending);

    assert_eq!(doc.objects.len(), pending.objects.len());
    let resolved_projection = resolved_object_projection(&doc);
    assert_eq!(
        resolved_projection,
        vec![
            (0x30, 0, "DWG_TABLE_SECTION_0_RECORD_0_SIZE_3_TABLE_".to_string()),
            (0x41, 0x90, "DWG_ENTITY_SECTION_1_RECORD_1_SIZE_4_ENTITY_".to_string()),
            (0x52, 0x91, "DWG_OBJECT_SECTION_2_RECORD_2_SIZE_5_OBJECT_".to_string()),
        ]
    );
    assert_eq!(doc.next_handle(), 0x92);
    let allocated = doc.allocate_handle();
    assert_eq!(allocated.value(), 0x92);
    assert!(allocated.value() > 0x91);
}

#[test]
fn read_dwg_rejects_invalid_and_known_unsupported_versions_at_public_api_boundary() {
    assert_eq!(
        read_dwg(b"ZZ9999fixture").unwrap_err(),
        DwgReadError::InvalidMagic {
            found: "ZZ9999".to_string(),
        }
    );

    for version in [
        DwgVersion::Ac1021,
        DwgVersion::Ac1024,
        DwgVersion::Ac1027,
        DwgVersion::Ac1032,
    ] {
        let err = read_dwg(version.to_string().as_bytes()).unwrap_err();
        assert_eq!(err, DwgReadError::UnsupportedVersion(version));
    }
}

#[test]
fn read_dwg_fails_closed_on_structural_corruption_after_version_recognition() {
    let truncated_directory = fixture_ac1018_truncated_directory(2, &[(0x40, 3)]);
    assert_eq!(
        read_dwg(&truncated_directory).unwrap_err(),
        DwgReadError::TruncatedSectionDirectory {
            version: DwgVersion::Ac1018,
            expected_at_least: 45,
            actual: 37,
        }
    );

    let out_of_bounds =
        fixture_ac1015_with_declared_spans(2, &[(0x29, 3), (0x80, 5)], &[(0x29, b"abc")]);
    assert_eq!(
        read_dwg(&out_of_bounds).unwrap_err(),
        DwgReadError::SectionOutOfBounds {
            index: 1,
            offset: 0x80,
            size: 5,
            actual: out_of_bounds.len(),
        }
    );
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
    let pending = build_pending_document(&header, &sections, payloads).unwrap();

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
    let pending = build_pending_document(&header, &sections, payload_bytes).unwrap();

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

    assert_eq!(classify_section_records(payloads[0]).unwrap(), Vec::<Vec<u8>>::new());
    assert_eq!(classify_section_records(payloads[1]).unwrap(), vec![b"\0\0\0".to_vec()]);
    assert_eq!(classify_section_records(payloads[2]).unwrap(), vec![b"solo".to_vec()]);
    assert_eq!(classify_section_records(payloads[3]).unwrap(), vec![b"tail".to_vec()]);
    assert_eq!(
        classify_section_records(payloads[4]).unwrap(),
        vec![b"alpha".to_vec(), b"beta".to_vec(), b"gamma".to_vec()]
    );
    assert_eq!(
        classify_section_records(payloads[5]).unwrap(),
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
fn semantic_fixture_payloads_cover_reordered_embedded_zero_and_collision_cases() {
    let cases = semantic_record_fixture_cases();
    let reordered = cases
        .iter()
        .find(|case| case.id == "reordered-equivalent-a")
        .expect("reordered-equivalent-a fixture should exist");
    let reordered_pair = cases
        .iter()
        .find(|case| case.id == "reordered-equivalent-b")
        .expect("reordered-equivalent-b fixture should exist");
    let embedded_zero = cases
        .iter()
        .find(|case| case.id == "embedded-zero")
        .expect("embedded-zero fixture should exist");
    let collision = cases
        .iter()
        .find(|case| case.id == "same-size-collision")
        .expect("same-size-collision fixture should exist");
    let layout_variant = cases
        .iter()
        .find(|case| case.id == "layout-variant")
        .expect("layout-variant fixture should exist");
    let ownership = cases
        .iter()
        .find(|case| case.id == "ownership-graph")
        .expect("ownership-graph fixture should exist");
    let invalid = cases
        .iter()
        .find(|case| case.id == "invalid-ownership-graph")
        .expect("invalid-ownership-graph fixture should exist");

    assert_eq!(
        semantic_fixture_sections(reordered),
        vec![
            vec![
                b"TBL:LAYER:LayerAlpha:H10".to_vec(),
                b"ENT:LINE:E10:O30:LLayerAlpha".to_vec(),
            ],
            vec![b"OBJ:BLOCK:BlockAlpha:H30:LAYOUT=Model".to_vec()],
            vec![b"OBJ:LAYOUT:Model:H20:B30".to_vec()],
        ]
    );
    assert_eq!(
        semantic_fixture_sections(reordered_pair),
        vec![
            vec![b"OBJ:LAYOUT:Model:H20:B30".to_vec()],
            vec![
                b"TBL:LAYER:LayerAlpha:H10".to_vec(),
                b"ENT:LINE:E10:O30:LLayerAlpha".to_vec(),
            ],
            vec![b"OBJ:BLOCK:BlockAlpha:H30:LAYOUT=Model".to_vec()],
        ]
    );
    assert_eq!(
        semantic_fixture_sections(embedded_zero),
        vec![vec![
            b"OBJ:TEXT:Zero\0Payload:H44:O22".to_vec(),
            b"ENT:ARC:E44:O22:LLayerZero".to_vec(),
        ]]
    );
    assert_eq!(
        semantic_fixture_sections(collision),
        vec![vec![
            b"TBL:LTYPE:Dash:H50".to_vec(),
            b"TBL:STYLE:Wide:H51".to_vec(),
        ]]
    );
    assert_eq!(
        semantic_fixture_sections(layout_variant),
        vec![
            vec![b"OBJ:BLOCK:*Paper_Space:H72:LAYOUT=LayoutA".to_vec()],
            vec![b"OBJ:LAYOUT:LayoutA:H62:B72".to_vec()],
            vec![b"ENT:TEXT:E73:O72:LLayerPaper".to_vec()],
        ]
    );
    assert_eq!(
        semantic_fixture_sections(ownership),
        vec![
            vec![b"TBL:LAYER:LayerModel:H80".to_vec()],
            vec![
                b"OBJ:BLOCK:*Model_Space:H81:LAYOUT=Model".to_vec(),
                b"OBJ:LAYOUT:Model:H82:B81".to_vec(),
            ],
            vec![
                b"ENT:LINE:E83:O81:LLayerModel".to_vec(),
                b"ENT:INSERT:E84:O90:LLayerModel".to_vec(),
            ],
            vec![b"OBJ:BLOCK:DoorBlock:H90".to_vec()],
        ]
    );
    assert_eq!(
        semantic_fixture_sections(invalid),
        vec![
            vec![b"OBJ:LAYOUT:Broken:H95:BFF".to_vec()],
            vec![b"ENT:LINE:E96:OFF:LLayerBroken".to_vec()],
        ]
    );
}

#[test]
fn semantic_fixture_bytes_remain_stable_across_layout_variants() {
    let fixture = semantic_record_fixture("reordered-equivalent-a");
    let reordered = semantic_record_fixture("reordered-equivalent-b");
    let first = semantic_record_fixture("layout-variant");
    let second = semantic_record_fixture("layout-variant");

    assert_eq!(parse_pending_fixture(&first), parse_pending_fixture(&second));
    assert_ne!(fixture, reordered);
    let mut first_projection = parse_semantic_record_tuples(&fixture);
    let mut second_projection = parse_semantic_record_tuples(&reordered);
    first_projection.sort();
    second_projection.sort();
    assert_eq!(first_projection, second_projection);
}

#[test]
fn semantic_fixture_graph_cases_make_valid_and_invalid_relationships_explicit() {
    let valid = semantic_record_fixture("ownership-graph");
    let invalid = semantic_record_fixture("invalid-ownership-graph");

    assert_eq!(
        semantic_fixture_graph_projection(&valid),
        vec![
            "handle=128 owner=0 record=DWG_TABLE_SECTION_0_RECORD_0_SIZE_24_TABLE_LAYER_LAYERMODEL_HANDLE_80".to_string(),
            "handle=129 owner=0 record=DWG_OBJECT_SECTION_1_RECORD_0_SIZE_39_BLOCK_*MODEL_SPACE_LAYOUT_MODEL".to_string(),
            "handle=130 owner=0 record=DWG_OBJECT_SECTION_1_RECORD_1_SIZE_24_LAYOUT_MODEL_BLOCK_HANDLE_81".to_string(),
            "handle=131 owner=129 record=DWG_ENTITY_SECTION_2_RECORD_0_SIZE_28_ENTITY_LINE_LAYER_LAYERMODEL|OWNER_81".to_string(),
            "handle=132 owner=144 record=DWG_ENTITY_SECTION_2_RECORD_1_SIZE_30_ENTITY_INSERT_LAYER_LAYERMODEL|OWNER_90".to_string(),
            "handle=144 owner=0 record=DWG_OBJECT_SECTION_3_RECORD_0_SIZE_23_BLOCK_DOORBLOCK_".to_string(),
        ]
    );
    assert_eq!(
        semantic_fixture_graph_projection(&invalid),
        vec![
            "handle=149 owner=0 record=DWG_OBJECT_SECTION_0_RECORD_0_SIZE_25_LAYOUT_BROKEN_".to_string(),
            "handle=150 owner=255 record=DWG_ENTITY_SECTION_1_RECORD_0_SIZE_29_ENTITY_LINE_LAYER_LAYERBROKEN|OWNER_FF".to_string(),
        ]
    );
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
        classify_section_records(b"one\0two\0three").unwrap(),
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );
}

#[test]
fn semantic_decode_keeps_classification_stable_across_section_reordering() {
    let a = parse_pending_fixture(&semantic_record_fixture("reordered-equivalent-a"));
    let b = parse_pending_fixture(&semantic_record_fixture("reordered-equivalent-b"));

    let mut a_projection = a
        .objects
        .iter()
        .map(|object| format!("{:?}", object.kind))
        .collect::<Vec<_>>();
    let mut b_projection = b
        .objects
        .iter()
        .map(|object| format!("{:?}", object.kind))
        .collect::<Vec<_>>();
    a_projection.sort();
    b_projection.sort();
    assert_eq!(a_projection, b_projection);
}

#[test]
fn semantic_decode_embedded_zero_and_same_size_records_keep_distinct_identities() {
    let embedded = parse_semantic_record_tuples(&semantic_record_fixture("embedded-zero"));
    let collision = parse_semantic_record_tuples(&semantic_record_fixture("same-size-collision"));

    assert_eq!(
        embedded,
        vec![
            "size=26 semantic=ENT:ARC:E44:O22:LLayerZero".to_string(),
            "size=29 semantic=OBJ:TEXT:Zero\0Payload:H44:O22".to_string(),
        ]
    );
    assert_eq!(
        collision,
        vec![
            "size=18 semantic=TBL:LTYPE:Dash:H50".to_string(),
            "size=18 semantic=TBL:STYLE:Wide:H51".to_string(),
        ]
    );
}

#[test]
fn pending_layer_semantics_come_from_decoded_records() {
    let pending = parse_pending_fixture(&semantic_record_fixture("ownership-graph"));

    assert_eq!(
        pending.layers,
        vec![h7cad_native_dwg::PendingLayer {
            handle: h7cad_native_model::Handle::new(0x80),
            name: "LayerModel".to_string(),
        }]
    );
}

#[test]
fn pending_entity_semantics_preserve_handle_owner_and_layer_relationships() {
    let pending = parse_pending_fixture(&semantic_record_fixture("ownership-graph"));

    assert_eq!(
        pending.entities,
        vec![
            h7cad_native_dwg::PendingEntity {
                handle: h7cad_native_model::Handle::new(0x83),
                owner_handle: h7cad_native_model::Handle::new(0x81),
                layer_name: "LayerModel".to_string(),
            },
            h7cad_native_dwg::PendingEntity {
                handle: h7cad_native_model::Handle::new(0x84),
                owner_handle: h7cad_native_model::Handle::new(0x90),
                layer_name: "LayerModel".to_string(),
            },
        ]
    );
}

#[test]
fn semantic_provenance_projection_is_stable_across_repeated_parses() {
    let bytes = semantic_record_fixture("ownership-graph");

    let first = parse_pending_fixture(&bytes);
    let second = parse_pending_fixture(&bytes);

    assert_eq!(pending_semantic_projection(&first), pending_semantic_projection(&second));
}

#[test]
fn block_layout_semantics_are_outwardly_distinguishable_on_parser_surfaces() {
    let pending = parse_pending_fixture(&semantic_record_fixture("layout-variant"));
    let projection = pending_provenance_projection(&pending);

    assert!(projection.iter().any(|tuple| {
        tuple.semantic_identity == "block:*Paper_Space" && tuple.semantic_link == "layout:LayoutA"
    }));
    assert!(projection.iter().any(|tuple| {
        tuple.semantic_identity == "layout:LayoutA" && tuple.semantic_link == "block_handle:72"
    }));
    assert!(projection.iter().any(|tuple| {
        tuple.semantic_identity == "entity:TEXT"
            && tuple.semantic_link == "layer:LayerPaper|owner:72"
    }));
}

#[test]
fn pending_entity_summary_exposes_owner_and_layer_provenance_together() {
    let pending = parse_pending_fixture(&semantic_record_fixture("ownership-graph"));
    let summaries = pending
        .objects
        .iter()
        .map(summarize_object)
        .collect::<Vec<_>>();

    assert!(summaries.iter().any(|summary| {
        summary.semantic_identity == "entity:LINE"
            && summary.semantic_link == "layer:LayerModel|owner:81"
    }));
    assert!(summaries.iter().any(|summary| {
        summary.semantic_identity == "entity:INSERT"
            && summary.semantic_link == "layer:LayerModel|owner:90"
    }));
}

#[test]
fn semantic_decode_reports_real_nonzero_section_index() {
    let bytes = fixture_ac1018(
        2,
        &[(0x80, 3), (0xC0, 42)],
        &[b"abc", b"OBJ:LAYOUT:Broken:H95:BFF\0ENT:LINE:EXX:OFF"],
    );
    let header = DwgFileHeader::parse(&bytes).unwrap();
    let sections = SectionMap::parse(&bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(&bytes).unwrap();
    let err = build_pending_document(&header, &sections, payloads).unwrap_err();

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
fn outward_summaries_distinguish_same_size_semantic_records_by_identity_fields() {
    let pending = parse_pending_fixture(&semantic_record_fixture("same-size-collision"));
    let summaries = pending
        .objects
        .iter()
        .map(summarize_object)
        .collect::<Vec<_>>();

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].payload_size, summaries[1].payload_size);
    assert_ne!(summaries[0].semantic_identity, summaries[1].semantic_identity);
    assert_ne!(summaries[0].semantic_link, summaries[1].semantic_link);
}

#[test]
fn provenance_projection_stays_deterministic_across_equivalent_layout_variants() {
    let first = parse_pending_fixture(&semantic_record_fixture("layout-variant"));
    let second = parse_pending_fixture(&semantic_record_fixture("layout-variant"));

    assert_eq!(
        pending_provenance_projection(&first),
        pending_provenance_projection(&second)
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
        semantic_identity: None,
        semantic_link: None,
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
            semantic_identity: "entity".to_string(),
            semantic_link: String::new(),
        }
    );
}

fn parse_pending_fixture(bytes: &[u8]) -> h7cad_native_dwg::PendingDocument {
    let header = DwgFileHeader::parse(bytes).unwrap();
    let sections = SectionMap::parse(bytes, &header).unwrap();
    let payloads = sections.read_section_payloads(bytes).unwrap();
    build_pending_document(&header, &sections, payloads).unwrap()
}

fn pending_semantic_projection(
    pending: &h7cad_native_dwg::PendingDocument,
) -> Vec<(u64, u64, &'static str, String)> {
    let mut projection = pending
        .objects
        .iter()
        .map(|object| {
            let semantic_kind = match object.kind {
                PendingObjectKind::TableRecord { .. } => "table",
                PendingObjectKind::EntityRecord { .. } => "entity",
                PendingObjectKind::ObjectRecord { .. } => "object",
            };
            (
                object.handle.value(),
                object.owner_handle.value(),
                semantic_kind,
                pending
                    .entities
                    .iter()
                    .find(|entity| entity.handle == object.handle)
                    .map(|entity| entity.layer_name.clone())
                    .or_else(|| {
                        pending
                            .layers
                            .iter()
                            .find(|layer| layer.handle == object.handle)
                            .map(|layer| layer.name.clone())
                    })
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    projection.sort();
    projection
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PendingProvenanceTuple {
    handle: u64,
    owner: u64,
    target: &'static str,
    semantic_identity: String,
    semantic_link: String,
}

fn pending_provenance_projection(
    pending: &h7cad_native_dwg::PendingDocument,
) -> Vec<PendingProvenanceTuple> {
    let mut projection = pending
        .objects
        .iter()
        .map(|object| {
            let target = match object.kind {
                PendingObjectKind::TableRecord { .. } => "table",
                PendingObjectKind::EntityRecord { .. } => "entity",
                PendingObjectKind::ObjectRecord { .. } => "object",
            };
            let summary = summarize_object(object);
            PendingProvenanceTuple {
                handle: object.handle.value(),
                owner: object.owner_handle.value(),
                target,
                semantic_identity: summary.semantic_identity,
                semantic_link: summary.semantic_link,
            }
        })
        .collect::<Vec<_>>();
    projection.sort();
    projection
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

fn semantic_record_fixture(id: &str) -> Vec<u8> {
    let case = semantic_record_fixture_cases()
        .into_iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("unknown semantic fixture case: {id}"));
    let payload_refs = case
        .payloads
        .iter()
        .map(|payload| payload.as_slice())
        .collect::<Vec<_>>();
    fixture_ac1018(case.entries.len() as u32, &case.entries, &payload_refs)
}

fn semantic_record_fixture_cases() -> Vec<SemanticFixtureCase> {
    vec![
        SemanticFixtureCase::new(
            "reordered-equivalent-a",
            semantic_entries(&[0x80, 0xC0, 0xF0], &[
                semantic_join(&[
                    b"TBL:LAYER:LayerAlpha:H10",
                    b"ENT:LINE:E10:O30:LLayerAlpha",
                ]),
                semantic_join(&[b"OBJ:BLOCK:BlockAlpha:H30:LAYOUT=Model"]),
                semantic_join(&[b"OBJ:LAYOUT:Model:H20:B30"]),
            ]),
        ),
        SemanticFixtureCase::new(
            "reordered-equivalent-b",
            semantic_entries(&[0x80, 0xB0, 0xF0], &[
                semantic_join(&[b"OBJ:LAYOUT:Model:H20:B30"]),
                semantic_join(&[
                    b"TBL:LAYER:LayerAlpha:H10",
                    b"ENT:LINE:E10:O30:LLayerAlpha",
                ]),
                semantic_join(&[b"OBJ:BLOCK:BlockAlpha:H30:LAYOUT=Model"]),
            ]),
        ),
        SemanticFixtureCase::new(
            "embedded-zero",
            semantic_entries(&[0x80], &[semantic_join(&[
                b"OBJ:TEXT:Zero\0Payload:H44:O22",
                b"ENT:ARC:E44:O22:LLayerZero",
            ])]),
        ),
        SemanticFixtureCase::new(
            "same-size-collision",
            semantic_entries(&[0x80], &[semantic_join(&[
                b"TBL:LTYPE:Dash:H50",
                b"TBL:STYLE:Wide:H51",
            ])]),
        ),
        SemanticFixtureCase::new(
            "layout-variant",
            semantic_entries(&[0x80, 0xC0, 0xF0], &[
                semantic_join(&[b"OBJ:BLOCK:*Paper_Space:H72:LAYOUT=LayoutA"]),
                semantic_join(&[b"OBJ:LAYOUT:LayoutA:H62:B72"]),
                semantic_join(&[b"ENT:TEXT:E73:O72:LLayerPaper"]),
            ]),
        ),
        SemanticFixtureCase::new(
            "ownership-graph",
            semantic_entries(&[0x80, 0xB0, 0xF0, 0x130], &[
                semantic_join(&[b"TBL:LAYER:LayerModel:H80"]),
                semantic_join(&[
                    b"OBJ:BLOCK:*Model_Space:H81:LAYOUT=Model",
                    b"OBJ:LAYOUT:Model:H82:B81",
                ]),
                semantic_join(&[
                    b"ENT:LINE:E83:O81:LLayerModel",
                    b"ENT:INSERT:E84:O90:LLayerModel",
                ]),
                semantic_join(&[b"OBJ:BLOCK:DoorBlock:H90"]),
            ]),
        ),
        SemanticFixtureCase::new(
            "invalid-ownership-graph",
            semantic_entries(&[0x80, 0xC0], &[
                semantic_join(&[b"OBJ:LAYOUT:Broken:H95:BFF"]),
                semantic_join(&[b"ENT:LINE:E96:OFF:LLayerBroken"]),
            ]),
        ),
    ]
}

fn semantic_fixture_sections(case: &SemanticFixtureCase) -> Vec<Vec<Vec<u8>>> {
    case.payloads
        .iter()
        .map(|payload| semantic_split_records(payload))
        .collect()
}

fn semantic_fixture_graph_projection(bytes: &[u8]) -> Vec<String> {
    let doc = read_dwg(bytes).unwrap();
    let mut projection = resolved_object_projection(&doc);
    projection.sort();
    projection
        .into_iter()
        .map(|(handle, owner, object_type)| format!("handle={handle} owner={owner} record={object_type}"))
        .collect()
}

fn parse_semantic_record_tuples(bytes: &[u8]) -> Vec<String> {
    let mut projection = semantic_record_fixture_cases()
        .into_iter()
        .flat_map(|case| {
            let fixture = semantic_record_fixture(case.id);
            if fixture == bytes {
                semantic_fixture_sections(&case)
                    .into_iter()
                    .flatten()
                    .map(|record| {
                        format!(
                            "size={} semantic={}",
                            record.len(),
                            String::from_utf8_lossy(&record)
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
        .collect::<Vec<_>>();
    projection.sort();
    projection
}

fn semantic_entries(offsets: &[u32], payloads: &[Vec<u8>]) -> (Vec<(u32, u32)>, Vec<Vec<u8>>) {
    assert_eq!(offsets.len(), payloads.len());
    let entries = offsets
        .iter()
        .zip(payloads.iter())
        .map(|(offset, payload)| (*offset, payload.len() as u32))
        .collect::<Vec<_>>();
    (entries, payloads.to_vec())
}

fn semantic_join(records: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            bytes.push(0);
        }
        bytes.extend_from_slice(record);
    }
    bytes
}

fn semantic_split_records(payload: &[u8]) -> Vec<Vec<u8>> {
    if payload.is_empty() {
        return Vec::new();
    }

    let mut records = Vec::new();
    let mut current = Vec::new();
    let mut index = 0usize;
    while index < payload.len() {
        if payload[index] == 0 {
            let tail = &payload[index + 1..];
            if tail.starts_with(b"TBL:") || tail.starts_with(b"ENT:") || tail.starts_with(b"OBJ:") {
                if !current.is_empty() {
                    records.push(std::mem::take(&mut current));
                }
                index += 1;
                continue;
            }
        }
        current.push(payload[index]);
        index += 1;
    }
    if !current.is_empty() {
        records.push(current);
    }
    records
}

struct SemanticFixtureCase {
    id: &'static str,
    entries: Vec<(u32, u32)>,
    payloads: Vec<Vec<u8>>,
}

impl SemanticFixtureCase {
    fn new(id: &'static str, fixture: (Vec<(u32, u32)>, Vec<Vec<u8>>)) -> Self {
        let (entries, payloads) = fixture;
        Self { id, entries, payloads }
    }
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
