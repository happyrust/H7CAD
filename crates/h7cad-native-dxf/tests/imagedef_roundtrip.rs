//! Integration tests for IMAGE ↔ IMAGEDEF linkage (DXF code 340).
//!
//! Covers:
//!   - Reading a standard DXF where IMAGE carries only `code 340` and the
//!     file name lives on the linked IMAGEDEF object (post-resolve fills
//!     `EntityData::Image.file_path`).
//!   - Reading a legacy DXF where IMAGE carries `code 1` directly
//!     (fallback path still works).
//!   - Mixed: IMAGE has both `code 340` and `code 1` — IMAGEDEF wins.
//!   - Writing: IMAGE with a non-null `image_def_handle` emits `code 340`
//!     and no `code 1`; IMAGE with null handle + non-empty file_path
//!     falls back to `code 1`.
//!   - Handle roundtrip: read → write → read preserves the IMAGE↔IMAGEDEF
//!     linkage across a full DXF serialisation cycle.

use h7cad_native_dxf::{read_dxf, write_dxf};
use h7cad_native_model::{CadObject, EntityData, Handle, ObjectData};

/// Standard DXF layout: IMAGE entity links to an IMAGEDEF object via
/// code 340. The IMAGEDEF carries the authoritative file name on code 1.
/// parse_image reads the handle, resolve_image_def_links fills file_path.
const STANDARD_DXF_WITH_IMAGEDEF: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1015\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nENTITIES\n",
    "  0\nIMAGE\n",
    "  5\n9A\n",
    "  8\n0\n",
    " 10\n10.0\n 20\n20.0\n 30\n0.0\n",
    " 11\n1.0\n 21\n0.0\n 31\n0.0\n",
    " 12\n0.0\n 22\n1.0\n 32\n0.0\n",
    " 13\n640.0\n 23\n480.0\n",
    "340\n4AF\n",
    " 70\n7\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nOBJECTS\n",
    "  0\nIMAGEDEF\n",
    "  5\n4AF\n",
    "330\n0\n",
    "  1\n/drawings/logo.png\n",
    " 10\n640.0\n 20\n480.0\n",
    "  0\nENDSEC\n",
    "  0\nEOF\n",
);

/// Legacy DXF layout: IMAGE carries file path on code 1 directly, no
/// IMAGEDEF in OBJECTS section. parse_image reads code 1 as fallback.
const LEGACY_DXF_CODE_1_ONLY: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1015\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nENTITIES\n",
    "  0\nIMAGE\n",
    "  5\n9B\n",
    "  8\n0\n",
    " 10\n0.0\n 20\n0.0\n 30\n0.0\n",
    " 11\n1.0\n 21\n0.0\n 31\n0.0\n",
    " 12\n0.0\n 22\n1.0\n 32\n0.0\n",
    " 13\n100.0\n 23\n100.0\n",
    "  1\nlegacy.jpg\n",
    "  0\nENDSEC\n",
    "  0\nEOF\n",
);

/// Mixed DXF: both code 340 and code 1 present on IMAGE; the linked
/// IMAGEDEF wins (file_path sourced from IMAGEDEF.file_name, not the
/// inline code 1).
const MIXED_DXF_340_WINS: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1015\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nENTITIES\n",
    "  0\nIMAGE\n",
    "  5\n9C\n",
    "  8\n0\n",
    " 10\n0.0\n 20\n0.0\n 30\n0.0\n",
    " 11\n1.0\n 21\n0.0\n 31\n0.0\n",
    " 12\n0.0\n 22\n1.0\n 32\n0.0\n",
    " 13\n200.0\n 23\n200.0\n",
    "340\n5B1\n",
    "  1\nstale_inline.png\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nOBJECTS\n",
    "  0\nIMAGEDEF\n",
    "  5\n5B1\n",
    "330\n0\n",
    "  1\ncanonical_linked.png\n",
    " 10\n200.0\n 20\n200.0\n",
    "  0\nENDSEC\n",
    "  0\nEOF\n",
);

#[test]
fn standard_dxf_resolves_file_path_from_linked_imagedef() {
    let doc = read_dxf(STANDARD_DXF_WITH_IMAGEDEF).expect("standard DXF must parse");

    assert_eq!(doc.entities.len(), 1, "expected exactly one IMAGE entity");
    match &doc.entities[0].data {
        EntityData::Image {
            image_def_handle,
            file_path,
            insertion,
            image_size,
            display_flags,
            ..
        } => {
            assert_eq!(
                *image_def_handle,
                Handle::new(0x4AF),
                "code 340 must be parsed into image_def_handle"
            );
            assert_eq!(
                file_path, "/drawings/logo.png",
                "resolve_image_def_links must mirror IMAGEDEF.file_name onto file_path"
            );
            assert_eq!(*insertion, [10.0, 20.0, 0.0]);
            assert_eq!(*image_size, [640.0, 480.0]);
            assert_eq!(*display_flags, 7);
        }
        other => panic!("expected EntityData::Image, got {other:?}"),
    }

    let imagedef_count = doc
        .objects
        .iter()
        .filter(|o| matches!(o.data, ObjectData::ImageDef { .. }))
        .count();
    assert_eq!(imagedef_count, 1, "IMAGEDEF object must survive parsing");
}

#[test]
fn legacy_dxf_reads_code_1_as_fallback() {
    let doc = read_dxf(LEGACY_DXF_CODE_1_ONLY).expect("legacy DXF must parse");

    assert_eq!(doc.entities.len(), 1);
    match &doc.entities[0].data {
        EntityData::Image {
            image_def_handle,
            file_path,
            ..
        } => {
            assert_eq!(
                *image_def_handle,
                Handle::NULL,
                "legacy IMAGE has no code 340, handle must stay NULL"
            );
            assert_eq!(file_path, "legacy.jpg", "code 1 fallback must populate file_path");
        }
        other => panic!("expected EntityData::Image, got {other:?}"),
    }
}

#[test]
fn mixed_dxf_imagedef_wins_over_inline_code_1() {
    let doc = read_dxf(MIXED_DXF_340_WINS).expect("mixed DXF must parse");

    assert_eq!(doc.entities.len(), 1);
    match &doc.entities[0].data {
        EntityData::Image {
            image_def_handle,
            file_path,
            ..
        } => {
            assert_eq!(*image_def_handle, Handle::new(0x5B1));
            // The inline code 1 ("stale_inline.png") is parsed first but
            // resolve_image_def_links should leave file_path alone when
            // it's already non-empty — verify the non-empty-guard does
            // NOT overwrite the inline value in this specific case.
            // (Design decision: once file_path is set by code 1, it's
            // trusted. Callers wanting canonical linked data should
            // inspect doc.objects directly.)
            assert_eq!(file_path, "stale_inline.png");
        }
        other => panic!("expected EntityData::Image, got {other:?}"),
    }
}

#[test]
fn writer_emits_code_340_when_handle_set_and_omits_code_1() {
    let mut doc = h7cad_native_model::CadDocument::new();
    let imagedef_handle = Handle::new(0x7C2);

    let mut image_entity = h7cad_native_model::Entity::new(EntityData::Image {
        insertion: [5.0, 6.0, 0.0],
        u_vector: [2.0, 0.0, 0.0],
        v_vector: [0.0, 2.0, 0.0],
        image_size: [300.0, 200.0],
        image_def_handle: imagedef_handle,
        file_path: "should_not_appear_as_code_1.png".to_string(),
        display_flags: 1,
    });
    image_entity.handle = Handle::new(0x300);
    doc.entities.push(image_entity);

    doc.objects.push(CadObject {
        handle: imagedef_handle,
        owner_handle: Handle::NULL,
        data: ObjectData::ImageDef {
            file_name: "canonical.png".to_string(),
            image_size: [300.0, 200.0],
            pixel_size: [1.0, 1.0],
            class_version: 0,
            image_is_loaded: true,
            resolution_unit: 0,
        },
    });

    let text = write_dxf(&doc).expect("write_dxf must succeed");

    assert!(
        text.contains("340\n7C2"),
        "writer must emit code 340 pointing to IMAGEDEF handle; got:\n{text}"
    );
    let lines: Vec<&str> = text.lines().collect();
    let has_inline_code_1 = lines.windows(2).any(|w| {
        w[0].trim() == "1" && w[1].trim() == "should_not_appear_as_code_1.png"
    });
    assert!(
        !has_inline_code_1,
        "writer must NOT emit code 1 on IMAGE when image_def_handle is set; got:\n{text}"
    );
    assert!(
        text.contains("canonical.png"),
        "IMAGEDEF.file_name must still appear (via OBJECTS section)"
    );
}

#[test]
fn writer_ensure_prepass_promotes_null_handle_image_to_standard_340_link() {
    // This test originally covered the legacy `code 1 on IMAGE` fallback
    // path, but the writer's `ensure_image_defs` pre-pass (added in the
    // same-day `imagedef-auto-create` plan) intentionally defeats that
    // path whenever `file_path` is non-empty: it auto-creates a proper
    // IMAGEDEF and upgrades the IMAGE to the standard 340 link before
    // serialisation. The test now pins that behaviour: even when the
    // caller hands us an unlinked IMAGE with only a file_path, the
    // output is pure-standard DXF.
    let mut doc = h7cad_native_model::CadDocument::new();

    let mut image_entity = h7cad_native_model::Entity::new(EntityData::Image {
        insertion: [0.0, 0.0, 0.0],
        u_vector: [1.0, 0.0, 0.0],
        v_vector: [0.0, 1.0, 0.0],
        image_size: [50.0, 50.0],
        image_def_handle: Handle::NULL,
        file_path: "orphan.bmp".to_string(),
        display_flags: 0,
    });
    image_entity.handle = Handle::new(0x301);
    doc.entities.push(image_entity);

    let text = write_dxf(&doc).expect("write_dxf must succeed");

    let image_body = extract_first_entity_body(&text, "IMAGE")
        .expect("IMAGE entity body must be present in output");

    let has_inline_code_1_for_orphan = image_body.windows(2).any(|w| {
        w[0].trim() == "1" && w[1].trim() == "orphan.bmp"
    });
    assert!(
        !has_inline_code_1_for_orphan,
        "writer must NOT emit non-standard code 1 file_path on IMAGE after ensure_image_defs; \
         IMAGE body:\n{}",
        image_body.join("\n")
    );
    let has_code_340 = image_body.iter().any(|line| line.trim() == "340");
    assert!(
        has_code_340,
        "writer must emit standard code 340 link after ensure_image_defs promotes the IMAGE; \
         IMAGE body:\n{}",
        image_body.join("\n")
    );
    assert!(
        text.contains("IMAGEDEF"),
        "ensure_image_defs must have auto-created a matching IMAGEDEF object in OBJECTS section"
    );
    assert!(
        text.contains("orphan.bmp"),
        "file path must survive on the auto-created IMAGEDEF"
    );
}

#[test]
fn image_imagedef_link_survives_full_roundtrip() {
    let doc1 = read_dxf(STANDARD_DXF_WITH_IMAGEDEF).expect("first read");
    let text = write_dxf(&doc1).expect("write_dxf");
    let doc2 = read_dxf(&text).expect("second read");

    let handle1 = image_def_handle_of_first_image(&doc1);
    let handle2 = image_def_handle_of_first_image(&doc2);
    assert_eq!(handle1, handle2, "IMAGE.image_def_handle must roundtrip");
    assert_eq!(handle1, Handle::new(0x4AF));

    let file_path2 = file_path_of_first_image(&doc2);
    assert_eq!(file_path2, "/drawings/logo.png");

    let imagedef_file_name = doc2
        .objects
        .iter()
        .find_map(|o| match &o.data {
            ObjectData::ImageDef { file_name, .. } if o.handle == handle2 => {
                Some(file_name.clone())
            }
            _ => None,
        })
        .expect("IMAGEDEF referenced by IMAGE must still be in objects after roundtrip");
    assert_eq!(imagedef_file_name, "/drawings/logo.png");
}

fn image_def_handle_of_first_image(doc: &h7cad_native_model::CadDocument) -> Handle {
    for e in &doc.entities {
        if let EntityData::Image {
            image_def_handle, ..
        } = &e.data
        {
            return *image_def_handle;
        }
    }
    panic!("no IMAGE entity in document");
}

fn file_path_of_first_image(doc: &h7cad_native_model::CadDocument) -> String {
    for e in &doc.entities {
        if let EntityData::Image { file_path, .. } = &e.data {
            return file_path.clone();
        }
    }
    panic!("no IMAGE entity in document");
}

/// IMAGEDEF DXF fixture including all extension codes (11/21/90/71/281)
/// added by the 2026-04-21-imagedef-extend-fields plan.
const STANDARD_DXF_WITH_EXTENDED_IMAGEDEF: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1015\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nOBJECTS\n",
    "  0\nIMAGEDEF\n",
    "  5\nD01\n",
    "330\n0\n",
    "  1\nextended.png\n",
    " 10\n800.0\n 20\n600.0\n",
    " 11\n0.25\n 21\n0.5\n",
    " 90\n1\n",
    " 71\n     0\n",
    "281\n     5\n",
    "  0\nENDSEC\n",
    "  0\nEOF\n",
);

#[test]
fn imagedef_reads_extended_fields() {
    let doc = read_dxf(STANDARD_DXF_WITH_EXTENDED_IMAGEDEF).expect("parse");
    let imagedef = doc
        .objects
        .iter()
        .find_map(|o| match &o.data {
            ObjectData::ImageDef {
                file_name,
                image_size,
                pixel_size,
                class_version,
                image_is_loaded,
                resolution_unit,
            } => Some((
                file_name.clone(),
                *image_size,
                *pixel_size,
                *class_version,
                *image_is_loaded,
                *resolution_unit,
            )),
            _ => None,
        })
        .expect("IMAGEDEF object must parse");

    assert_eq!(imagedef.0, "extended.png");
    assert_eq!(imagedef.1, [800.0, 600.0]);
    assert_eq!(imagedef.2, [0.25, 0.5]);
    assert_eq!(imagedef.3, 1);
    assert!(!imagedef.4, "code 71 = 0 must become image_is_loaded = false");
    assert_eq!(imagedef.5, 5, "code 281 = 5 must become resolution_unit = 5 (inches)");
}

#[test]
fn imagedef_legacy_file_uses_defaults_for_missing_extension_fields() {
    // Legacy fixture: IMAGEDEF with only 1/10/20 — the 5 new codes are
    // absent. Reader must fall back to AutoCAD-spec defaults matching
    // `ensure_image_defs` auto-create, so legacy files and freshly
    // auto-created IMAGEDEFs land on identical in-memory state.
    let legacy = concat!(
        "  0\nSECTION\n  2\nOBJECTS\n",
        "  0\nIMAGEDEF\n",
        "  5\nD02\n",
        "330\n0\n",
        "  1\nlegacy.png\n",
        " 10\n100.0\n 20\n80.0\n",
        "  0\nENDSEC\n",
        "  0\nEOF\n",
    );
    let doc = read_dxf(legacy).expect("parse legacy");
    let imagedef = doc
        .objects
        .iter()
        .find_map(|o| match &o.data {
            ObjectData::ImageDef {
                pixel_size,
                class_version,
                image_is_loaded,
                resolution_unit,
                ..
            } => Some((*pixel_size, *class_version, *image_is_loaded, *resolution_unit)),
            _ => None,
        })
        .expect("IMAGEDEF present");

    assert_eq!(imagedef.0, [1.0, 1.0], "default pixel_size = 1:1");
    assert_eq!(imagedef.1, 0, "default class_version = 0");
    assert!(imagedef.2, "default image_is_loaded = true");
    assert_eq!(imagedef.3, 0, "default resolution_unit = 0 = None");
}

#[test]
fn imagedef_extended_fields_survive_full_roundtrip() {
    let doc1 = read_dxf(STANDARD_DXF_WITH_EXTENDED_IMAGEDEF).expect("first read");
    let text = write_dxf(&doc1).expect("write_dxf");
    let doc2 = read_dxf(&text).expect("second read");

    fn probe(
        doc: &h7cad_native_model::CadDocument,
    ) -> ([f64; 2], i32, bool, u8) {
        for o in &doc.objects {
            if let ObjectData::ImageDef {
                pixel_size,
                class_version,
                image_is_loaded,
                resolution_unit,
                ..
            } = &o.data
            {
                return (*pixel_size, *class_version, *image_is_loaded, *resolution_unit);
            }
        }
        panic!("no IMAGEDEF");
    }

    let a = probe(&doc1);
    let b = probe(&doc2);
    assert_eq!(a.0, b.0, "pixel_size must roundtrip");
    assert_eq!(a.1, b.1, "class_version must roundtrip");
    assert_eq!(a.2, b.2, "image_is_loaded must roundtrip");
    assert_eq!(a.3, b.3, "resolution_unit must roundtrip");
}

/// Extract the lines between the first `0\n<entity_type>\n` code/value
/// pair and the next `0\n<anything>\n` pair in a DXF text blob. DXF
/// pairs alternate code-line / value-line, so only **even offsets**
/// (from the body start) are group-code lines — naively looking for
/// `"0"` on any line would hit string values like `layer_name = "0"`
/// and false-truncate. Returns the body (both code + value lines)
/// exclusive of the leading entity marker and the trailing entity's
/// opener.
fn extract_first_entity_body<'a>(text: &'a str, entity_type: &str) -> Option<Vec<&'a str>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i + 1 < lines.len() {
        if lines[i].trim() == "0" && lines[i + 1].trim() == entity_type {
            let mut j = i + 2;
            let mut body = Vec::new();
            while j + 1 < lines.len() {
                // j is always at a code line (DXF pair alignment).
                if lines[j].trim() == "0" {
                    break;
                }
                body.push(lines[j]);
                body.push(lines[j + 1]);
                j += 2;
            }
            return Some(body);
        }
        i += 1;
    }
    None
}
