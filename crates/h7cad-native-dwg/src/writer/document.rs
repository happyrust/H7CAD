//! Top-level AC1015 DWG document composer (F5.M2.T4 → F5.M4.E).
//!
//! Stitches the writer pipeline:
//! - `write_ac1015_file_header_prefix` (M2.T2)
//! - `write_ac1015_section_locator_directory` (M2.T2)
//! - `compose_ac1015_object_slice` (M4.B) per `Entity`
//! - `write_ac1015_handle_map_payload` (M3) over the derived
//!   `handle_offsets`
//! - the six `write_ac1015_<section>_section` composers (M2.T3)
//!
//! into a byte stream that the native reader (`read_dwg`) can re-parse
//! into a [`CadDocument`] with the original `entities` recovered.
//!
//! On-disk layout produced (M4.E):
//!
//! ```text
//! [file_header_prefix 0x19]
//! [directory 6 × 9 = 54 bytes]
//! [object_slices: each Entity's MS prefix + body bits + 2-byte CRC stub]
//! [section payloads in record-number order]
//! ```
//!
//! object_slices precede the section payloads so each Entity's
//! absolute `file_offset` can be derived in a single forward pass:
//! handles section consumes the `(handle, file_offset)` table, and
//! the directory's section descriptors carry offsets that already
//! include the object_slices total length.
//!
//! M5 known limitations:
//! - Only `EntityData::Line`, `EntityData::Circle`, `EntityData::Arc`,
//!   `EntityData::Point`, `EntityData::LwPolyline`, and `EntityData::Text`
//!   plus `EntityData::Attrib` are supported. Other entity types surface
//!   `DwgWriteError::Unsupported(... pending F5.M5)`.
//! - The common-entity-header writer is the M4.C minimal config:
//!   `owner_handle` always reads back as `Handle::NULL`,
//!   `lineweight` always reads back as `-3` ByDefault. See
//!   `docs/plans/2026-05-09-dwg-m4c-entity-common-minimal-plan.md`.
//! - facade `save(NativeFormat::Dwg, _)` keeps returning the M2
//!   placeholder; switch is gated on M5/M6 (see
//!   `docs/plans/2026-05-09-dwg-m4e-write-dwg-line-roundtrip-plan.md`
//!   §8).

use crate::bit_writer::BitWriter;
use crate::entity_arc::ArcGeometry;
use crate::entity_attrib::AttribGeometry;
use crate::entity_circle::CircleGeometry;
use crate::entity_ellipse::EllipseGeometry;
use crate::entity_hatch::HatchGeometry;
use crate::entity_insert::InsertGeometry;
use crate::entity_line::LineGeometry;
use crate::entity_lwpolyline::LwPolylineGeometry;
use crate::entity_mtext::MTextGeometry;
use crate::entity_point::PointGeometry;
use crate::entity_ray::RayGeometry;
use crate::entity_solid::{Face3DGeometry, SolidGeometry};
use crate::entity_spline::SplineGeometry;
use crate::entity_text::TextGeometry;
use crate::entity_viewport::ViewportGeometry;
use crate::handle_map::HandleMapEntry;
use crate::object_header::{ObjectHeader, HANDLE_CODE_HARD_OWNER};
use crate::writer::entity_arc::write_arc_geometry;
use crate::writer::entity_attrib::write_attrib_geometry;
use crate::writer::entity_circle::write_circle_geometry;
use crate::writer::entity_common::{write_ac1015_entity_common_minimal, EntityCommonMinimal};
use crate::writer::entity_ellipse::write_ellipse_geometry;
use crate::writer::entity_face3d::write_face3d_geometry;
use crate::writer::entity_hatch::write_hatch_geometry;
use crate::writer::entity_insert::write_insert_geometry;
use crate::writer::entity_line::write_line_geometry;
use crate::writer::entity_lwpolyline::write_lwpolyline_geometry;
use crate::writer::entity_mtext::write_mtext_geometry;
use crate::writer::entity_point::write_point_geometry;
use crate::writer::entity_ray::write_ray_geometry;
use crate::writer::entity_solid::write_solid_geometry;
use crate::writer::entity_spline::write_spline_geometry;
use crate::writer::entity_text::write_text_geometry;
use crate::writer::entity_viewport::write_viewport_geometry;
use crate::writer::file_header::{
    write_ac1015_file_header_prefix, write_ac1015_section_locator_directory,
    AC1015_FILE_HEADER_PREFIX_LEN, AC1015_SECTION_LOCATOR_ENTRY_LEN,
};
use crate::writer::handle_map::write_ac1015_handle_map_payload;
use crate::writer::object_header::write_ac1015_object_header;
use crate::writer::object_slice::compose_ac1015_object_slice;
use crate::writer::section_aux_header::write_ac1015_aux_header_section;
use crate::writer::section_classes::write_ac1015_classes_section;
use crate::writer::section_handles::write_ac1015_handles_section;
use crate::writer::section_header::write_ac1015_header_section;
use crate::writer::section_obj_free_space::write_ac1015_obj_free_space_section;
use crate::writer::section_template::write_ac1015_template_section;
use crate::{DwgWriteError, KnownSection, SectionDescriptor};
use h7cad_native_model::{CadDocument, Entity, EntityData, Handle, LwVertex};

/// Number of AC1015 well-known sections this composer always emits:
/// `Header / Classes / Handles / ObjFreeSpace / Template / AuxHeader`.
pub const AC1015_KNOWN_SECTION_COUNT: u32 = 6;

/// Minimum byte length of a `write_dwg(empty_doc)` output. Computed
/// from the fixed file header prefix and the six-entry section
/// locator directory (all six payloads are zero-length when no
/// entities are present).
pub const AC1015_EMPTY_DWG_MIN_LEN: usize = AC1015_FILE_HEADER_PREFIX_LEN
    + (AC1015_KNOWN_SECTION_COUNT as usize) * AC1015_SECTION_LOCATOR_ENTRY_LEN;

/// AC1015 object-class number for LINE. Mirrors the value the
/// reader's `try_decode_entity_body` dispatch routes on.
const AC1015_OBJECT_TYPE_LINE: i16 = 19;
/// AC1015 object-class number for CIRCLE. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_CIRCLE: i16 = 18;
/// AC1015 object-class number for ARC. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_ARC: i16 = 17;
/// AC1015 object-class number for POINT. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_POINT: i16 = 27;
/// AC1015 object-class number for LWPOLYLINE. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_LWPOLYLINE: i16 = 77;
/// AC1015 object-class number for TEXT. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_TEXT: i16 = 1;
/// AC1015 object-class number for ATTRIB. Mirrors the reader dispatch.
const AC1015_OBJECT_TYPE_ATTRIB: i16 = 2;
/// AC1015 object-class number for SOLID. Mirrors the reader dispatch
/// (`SOLID_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_SOLID: i16 = 31;
/// AC1015 object-class number for 3DFACE. Mirrors the reader dispatch
/// (`FACE3D_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_FACE3D: i16 = 28;
/// AC1015 object-class number for RAY. Mirrors the reader dispatch
/// (`RAY_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_RAY: i16 = 38;
/// AC1015 object-class number for XLINE. Mirrors the reader dispatch
/// (`XLINE_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_XLINE: i16 = 40;
/// AC1015 object-class number for ELLIPSE. Mirrors the reader dispatch
/// (`ELLIPSE_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_ELLIPSE: i16 = 35;
/// AC1015 object-class number for SPLINE. Mirrors the reader dispatch
/// (`SPLINE_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_SPLINE: i16 = 36;
/// AC1015 object-class number for MTEXT. Mirrors the reader dispatch
/// (`MTEXT_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_MTEXT: i16 = 44;
/// AC1015 object-class number for INSERT. Mirrors the reader dispatch
/// (`INSERT_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_INSERT: i16 = 7;
/// AC1015 object-class number for VIEWPORT. Mirrors the reader dispatch
/// (`VIEWPORT_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_VIEWPORT: i16 = 34;
/// AC1015 object-class number for HATCH. Mirrors the reader dispatch
/// (`HATCH_OBJECT_TYPE` in `crate::lib`).
const AC1015_OBJECT_TYPE_HATCH: i16 = 78;

/// Serialise `doc` into AC1015 (R2000) DWG bytes.
///
/// Contract:
/// - `doc.entities` may contain LINE, CIRCLE, ARC, POINT, LWPOLYLINE,
///   TEXT, ATTRIB, SOLID, 3DFACE, RAY, XLINE, ELLIPSE, SPLINE, MTEXT,
///   INSERT, VIEWPORT, and HATCH entities; other entity types
///   surface `DwgWriteError::Unsupported`.
/// - Returns a byte stream that `read_dwg` re-parses into a
///   [`CadDocument`] whose default tables match
///   `CadDocument::new()` and whose `entities` recover (geometry,
///   layer name, color, linetype scale, invisible) per entity.
///   Common-header `owner_handle` and `lineweight` known
///   limitations apply (see module docs).
pub fn write_dwg(doc: &CadDocument) -> Result<Vec<u8>, DwgWriteError> {
    // Step 1: encode every entity into an object slice and remember
    // its absolute file offset so the handles section can index it.
    let object_slices_start: u32 = u32::try_from(
        AC1015_FILE_HEADER_PREFIX_LEN
            + (AC1015_KNOWN_SECTION_COUNT as usize) * AC1015_SECTION_LOCATOR_ENTRY_LEN,
    )
    .map_err(|_| {
        DwgWriteError::InvalidValue("AC1015 file header + directory length exceeds u32::MAX".into())
    })?;

    let mut cursor: u32 = object_slices_start;
    let mut object_slices: Vec<Vec<u8>> = Vec::with_capacity(doc.entities.len());
    let mut handle_offsets: Vec<HandleMapEntry> = Vec::with_capacity(doc.entities.len());
    for entity in &doc.entities {
        let slice = encode_entity(doc, entity)?;
        let size = u32::try_from(slice.len()).map_err(|_| DwgWriteError::SectionTooLarge {
            section: "object_slice",
            bytes: slice.len(),
            limit: u32::MAX as usize,
        })?;
        handle_offsets.push(HandleMapEntry {
            handle: entity.handle,
            offset: cursor as i64,
        });
        cursor = cursor
            .checked_add(size)
            .ok_or(DwgWriteError::SectionTooLarge {
                section: "object_slice",
                bytes: slice.len(),
                limit: u32::MAX as usize,
            })?;
        object_slices.push(slice);
    }

    // Step 2: build the six section payloads. The Handles payload
    // now consumes our derived `handle_offsets`.
    let header_payload = write_ac1015_header_section(doc)?;
    let classes_payload = write_ac1015_classes_section(doc)?;
    let handles_payload = if handle_offsets.is_empty() {
        write_ac1015_handles_section(&[])?
    } else {
        write_ac1015_handle_map_payload(&handle_offsets)?
    };
    let obj_free_space_payload = write_ac1015_obj_free_space_section();
    let template_payload = write_ac1015_template_section();
    let aux_header_payload = write_ac1015_aux_header_section();

    let payloads = [
        (KnownSection::Header, header_payload),
        (KnownSection::Classes, classes_payload),
        (KnownSection::Handles, handles_payload),
        (KnownSection::ObjFreeSpace, obj_free_space_payload),
        (KnownSection::Template, template_payload),
        (KnownSection::AuxHeader, aux_header_payload),
    ];

    // Step 3: descriptors carry offsets that *already* include the
    // object_slices region. `cursor` is currently sitting right
    // after the last object slice — exactly where the first section
    // payload starts.
    let mut descriptors = Vec::with_capacity(payloads.len());
    for (index, (section, payload)) in payloads.iter().enumerate() {
        let size = u32::try_from(payload.len()).map_err(|_| DwgWriteError::SectionTooLarge {
            section: ac1015_section_name(*section),
            bytes: payload.len(),
            limit: u32::MAX as usize,
        })?;
        descriptors.push(SectionDescriptor {
            index: index as u32,
            record_number: ac1015_section_record_number(*section),
            offset: cursor,
            size,
        });
        cursor = cursor
            .checked_add(size)
            .ok_or(DwgWriteError::SectionTooLarge {
                section: ac1015_section_name(*section),
                bytes: payload.len(),
                limit: u32::MAX as usize,
            })?;
    }

    // Step 4: assemble the final byte buffer.
    let prefix = write_ac1015_file_header_prefix(AC1015_KNOWN_SECTION_COUNT)?;
    let directory = write_ac1015_section_locator_directory(&descriptors)?;
    debug_assert_eq!(
        directory.len(),
        (AC1015_KNOWN_SECTION_COUNT as usize) * AC1015_SECTION_LOCATOR_ENTRY_LEN
    );

    let mut bytes = Vec::with_capacity(cursor as usize);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(&directory);
    for slice in &object_slices {
        bytes.extend_from_slice(slice);
    }
    for (_, payload) in payloads.iter() {
        bytes.extend_from_slice(payload);
    }
    Ok(bytes)
}

/// Encode a single entity to its AC1015 object slice. The returned
/// `Vec<u8>` is a complete `[MS prefix][body bits][2-byte CRC stub]`
/// blob suitable for direct concatenation into the file's
/// object-slice region.
fn encode_entity(doc: &CadDocument, entity: &Entity) -> Result<Vec<u8>, DwgWriteError> {
    match &entity.data {
        EntityData::Line { start, end } => encode_line_entity(doc, entity, *start, *end),
        EntityData::Circle { center, radius } => {
            encode_circle_entity(doc, entity, *center, *radius)
        }
        EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => encode_arc_entity(doc, entity, *center, *radius, *start_angle, *end_angle),
        EntityData::Point { position } => encode_point_entity(doc, entity, *position),
        EntityData::LwPolyline {
            vertices,
            closed,
            constant_width,
        } => encode_lwpolyline_entity(doc, entity, vertices.clone(), *closed, *constant_width),
        EntityData::Text {
            insertion,
            height,
            value,
            rotation,
            style_name,
            width_factor,
            oblique_angle,
            horizontal_alignment,
            vertical_alignment,
            alignment_point,
        } => encode_text_entity(
            doc,
            entity,
            *insertion,
            *height,
            value.clone(),
            *rotation,
            style_name,
            *width_factor,
            *oblique_angle,
            *horizontal_alignment,
            *vertical_alignment,
            *alignment_point,
        ),
        EntityData::Attrib {
            tag,
            value,
            insertion,
            height,
        } => encode_attrib_entity(doc, entity, tag.clone(), value.clone(), *insertion, *height),
        EntityData::Solid {
            corners,
            normal,
            thickness,
        } => encode_solid_entity(doc, entity, *corners, *normal, *thickness),
        EntityData::Face3D {
            corners,
            invisible_edges,
        } => encode_face3d_entity(doc, entity, *corners, *invisible_edges),
        EntityData::Ray { origin, direction } => {
            encode_ray_like_entity(doc, entity, *origin, *direction, EntityKind::Ray)
        }
        EntityData::XLine { origin, direction } => {
            encode_ray_like_entity(doc, entity, *origin, *direction, EntityKind::XLine)
        }
        EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => encode_ellipse_entity(
            doc,
            entity,
            *center,
            *major_axis,
            *ratio,
            *start_param,
            *end_param,
        ),
        EntityData::Spline {
            degree,
            closed,
            knots,
            control_points,
            weights,
            fit_points,
            start_tangent,
            end_tangent,
        } => encode_spline_entity(
            doc,
            entity,
            SplineGeometry {
                degree: *degree,
                closed: *closed,
                knots: knots.clone(),
                control_points: control_points.clone(),
                weights: weights.clone(),
                fit_points: fit_points.clone(),
                start_tangent: *start_tangent,
                end_tangent: *end_tangent,
            },
        ),
        EntityData::MText {
            insertion,
            height,
            width,
            rectangle_height,
            value,
            rotation,
            style_name,
            attachment_point,
            line_spacing_factor,
            drawing_direction,
        } => encode_mtext_entity(
            doc,
            entity,
            *insertion,
            *height,
            *width,
            *rectangle_height,
            value.clone(),
            *rotation,
            style_name,
            *attachment_point,
            *line_spacing_factor,
            *drawing_direction,
        ),
        EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            has_attribs: _,
            attribs: _,
        } => encode_insert_entity(
            doc,
            entity,
            block_name,
            *insertion,
            *scale,
            *rotation,
        ),
        EntityData::Viewport {
            center,
            width,
            height,
        } => encode_viewport_entity(doc, entity, *center, *width, *height),
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => encode_hatch_entity(
            doc,
            entity,
            pattern_name.clone(),
            *solid_fill,
            boundary_paths.clone(),
        ),
        other => Err(DwgWriteError::Unsupported(format!(
            "entity type {other:?} pending F5.M5"
        ))),
    }
}

/// Discriminates RAY vs XLINE inside [`encode_ray_like_entity`].
/// Both share the wire body (`3BD origin + 3BD direction`) but emit
/// different object-type codes and DXF type names.
#[derive(Copy, Clone)]
enum EntityKind {
    Ray,
    XLine,
}

fn encode_line_entity(
    doc: &CadDocument,
    entity: &Entity,
    start: [f64; 3],
    end: [f64; 3],
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_line_geometry(
        LineGeometry {
            start,
            end,
            thickness: entity.thickness,
            extrusion: entity.extrusion,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_LINE, "LINE", &main, &handle)
}

fn encode_circle_entity(
    doc: &CadDocument,
    entity: &Entity,
    center: [f64; 3],
    radius: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_circle_geometry(
        CircleGeometry {
            center,
            radius,
            thickness: entity.thickness,
            extrusion: entity.extrusion,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_CIRCLE, "CIRCLE", &main, &handle)
}

fn encode_arc_entity(
    doc: &CadDocument,
    entity: &Entity,
    center: [f64; 3],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_arc_geometry(
        ArcGeometry {
            center,
            radius,
            thickness: entity.thickness,
            extrusion: entity.extrusion,
            start_angle,
            end_angle,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_ARC, "ARC", &main, &handle)
}

fn encode_point_entity(
    doc: &CadDocument,
    entity: &Entity,
    position: [f64; 3],
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_point_geometry(
        PointGeometry {
            position,
            thickness: entity.thickness,
            extrusion: entity.extrusion,
            x_axis_angle: 0.0,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_POINT, "POINT", &main, &handle)
}

fn encode_lwpolyline_entity(
    doc: &CadDocument,
    entity: &Entity,
    vertices: Vec<LwVertex>,
    closed: bool,
    constant_width: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_lwpolyline_geometry(
        LwPolylineGeometry {
            vertices,
            closed,
            constant_width,
            elevation: 0.0,
            thickness: entity.thickness,
            extrusion: entity.extrusion,
        },
        &mut main,
    )?;

    compose_entity_object_slice(
        entity,
        AC1015_OBJECT_TYPE_LWPOLYLINE,
        "LWPOLYLINE",
        &main,
        &handle,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_text_entity(
    doc: &CadDocument,
    entity: &Entity,
    insertion: [f64; 3],
    height: f64,
    value: String,
    rotation: f64,
    style_name: &str,
    width_factor: f64,
    oblique_angle: f64,
    horizontal_alignment: i16,
    vertical_alignment: i16,
    alignment_point: Option<[f64; 3]>,
) -> Result<Vec<u8>, DwgWriteError> {
    let style_handle = doc
        .text_styles
        .get(style_name)
        .map(|props| props.handle)
        .ok_or_else(|| {
            DwgWriteError::InvalidDocument(format!(
                "TEXT entity references unknown text style `{style_name}`"
            ))
        })?;

    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_text_geometry(
        TextGeometry {
            insertion,
            alignment_point,
            extrusion: entity.extrusion,
            thickness: entity.thickness,
            oblique_angle,
            rotation,
            height,
            width_factor,
            value,
            horizontal_alignment,
            vertical_alignment,
            style_handle,
        },
        &mut main,
        &mut handle,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_TEXT, "TEXT", &main, &handle)
}

fn encode_attrib_entity(
    doc: &CadDocument,
    entity: &Entity,
    tag: String,
    value: String,
    insertion: [f64; 3],
    height: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let style_handle = doc
        .text_styles
        .get("Standard")
        .map(|props| props.handle)
        .unwrap_or(Handle::NULL);

    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_attrib_geometry(
        AttribGeometry {
            tag,
            value,
            insertion,
            height,
            extrusion: entity.extrusion,
            thickness: entity.thickness,
            rotation: 0.0,
            style_handle,
        },
        &mut main,
        &mut handle,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_ATTRIB, "ATTRIB", &main, &handle)
}

fn encode_solid_entity(
    doc: &CadDocument,
    entity: &Entity,
    corners: [[f64; 3]; 4],
    normal: [f64; 3],
    thickness: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_solid_geometry(
        SolidGeometry {
            corners,
            thickness,
            extrusion: normal,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_SOLID, "SOLID", &main, &handle)
}

fn encode_face3d_entity(
    doc: &CadDocument,
    entity: &Entity,
    corners: [[f64; 3]; 4],
    invisible_edges: i16,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_face3d_geometry(
        Face3DGeometry {
            corners,
            invisible_edges,
        },
        &mut main,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_FACE3D, "3DFACE", &main, &handle)
}

fn encode_ray_like_entity(
    doc: &CadDocument,
    entity: &Entity,
    origin: [f64; 3],
    direction: [f64; 3],
    kind: EntityKind,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_ray_geometry(RayGeometry { origin, direction }, &mut main)?;

    let (object_type, type_name) = match kind {
        EntityKind::Ray => (AC1015_OBJECT_TYPE_RAY, "RAY"),
        EntityKind::XLine => (AC1015_OBJECT_TYPE_XLINE, "XLINE"),
    };
    compose_entity_object_slice(entity, object_type, type_name, &main, &handle)
}

fn encode_ellipse_entity(
    doc: &CadDocument,
    entity: &Entity,
    center: [f64; 3],
    major_axis: [f64; 3],
    ratio: f64,
    start_param: f64,
    end_param: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    // The body's `3BD extrusion` mirrors the common-header
    // `entity.extrusion` (reader assigns the body value to it during
    // `enrich_with_real_entities`). Reuse the common-header value to
    // keep the read-write contract straight.
    write_ellipse_geometry(
        EllipseGeometry {
            center,
            major_axis,
            extrusion: entity.extrusion,
            ratio,
            start_param,
            end_param,
        },
        &mut main,
    )?;

    compose_entity_object_slice(
        entity,
        AC1015_OBJECT_TYPE_ELLIPSE,
        "ELLIPSE",
        &main,
        &handle,
    )
}

fn encode_spline_entity(
    doc: &CadDocument,
    entity: &Entity,
    geom: SplineGeometry,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_spline_geometry(&geom, &mut main)?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_SPLINE, "SPLINE", &main, &handle)
}

#[allow(clippy::too_many_arguments)]
fn encode_mtext_entity(
    doc: &CadDocument,
    entity: &Entity,
    insertion: [f64; 3],
    height: f64,
    width: f64,
    rectangle_height: Option<f64>,
    value: String,
    rotation: f64,
    style_name: &str,
    attachment_point: i16,
    line_spacing_factor: f64,
    drawing_direction: i16,
) -> Result<Vec<u8>, DwgWriteError> {
    // Resolve style handle: prefer the named lookup, fall back to
    // "Standard" so any MTEXT can write even if the caller-supplied
    // style is unknown to the document's text_styles table. Failure
    // to find Standard yields `Handle::NULL`, which the reader will
    // surface as an empty style name.
    let style_handle = doc
        .text_styles
        .get(style_name)
        .or_else(|| doc.text_styles.get("Standard"))
        .map(|props| props.handle)
        .unwrap_or(Handle::NULL);

    // `EntityData::MText` stores only the scalar `rotation`; the
    // wire body needs the equivalent `x_direction` triple. The
    // reader recovers rotation via `atan2(x_direction[1], x_direction[0])`,
    // so the inverse here is `(cos, sin, 0)`. Note that `cos/sin →
    // atan2` is not bit-exact in f64; callers requiring lossless
    // rotation must compare with an epsilon.
    let x_direction = [rotation.cos(), rotation.sin(), 0.0];

    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_mtext_geometry(
        &MTextGeometry {
            insertion,
            extrusion: entity.extrusion,
            x_direction,
            rect_width: width,
            rect_height: rectangle_height.unwrap_or(0.0),
            height,
            attachment_point,
            drawing_direction,
            value,
            line_spacing_factor,
            style_handle,
            rotation,
        },
        &mut main,
        &mut handle,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_MTEXT, "MTEXT", &main, &handle)
}

fn encode_insert_entity(
    doc: &CadDocument,
    entity: &Entity,
    block_name: &str,
    insertion: [f64; 3],
    scale: [f64; 3],
    rotation: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    // Resolve block handle from the BLOCK_RECORD table by name.
    // `CadDocument.block_records` is keyed by handle, so a linear
    // scan is required. BLOCK_RECORD table serialisation is not
    // yet part of M5, so callers building docs from scratch must
    // seed `doc.block_records[<handle>].name` and the same handle
    // here. Missing blocks fall back to `Handle::NULL` rather than
    // failing — the reader will surface `$BLOCK_<HEX>` (or empty
    // for NULL) and higher layers can flag the inconsistency.
    let block_header_handle = doc
        .block_records
        .values()
        .find(|br| br.name == block_name)
        .map(|br| br.handle)
        .unwrap_or(Handle::NULL);

    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_insert_geometry(
        &InsertGeometry {
            insertion,
            scale,
            rotation,
            extrusion: entity.extrusion,
            // M5.E12 known limitation: writer always forces
            // has_attribs=false (see `entity_insert.rs` doc comment).
            // The caller-supplied value is preserved in
            // `EntityData::Insert` but ignored on serialisation.
            has_attribs: false,
            block_header_handle,
        },
        &mut main,
        &mut handle,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_INSERT, "INSERT", &main, &handle)
}

fn encode_viewport_entity(
    doc: &CadDocument,
    entity: &Entity,
    center: [f64; 3],
    width: f64,
    height: f64,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_viewport_geometry(
        ViewportGeometry {
            center,
            width,
            height,
        },
        &mut main,
    )?;

    compose_entity_object_slice(
        entity,
        AC1015_OBJECT_TYPE_VIEWPORT,
        "VIEWPORT",
        &main,
        &handle,
    )
}

fn encode_hatch_entity(
    doc: &CadDocument,
    entity: &Entity,
    pattern_name: String,
    solid_fill: bool,
    boundary_paths: Vec<h7cad_native_model::HatchBoundaryPath>,
) -> Result<Vec<u8>, DwgWriteError> {
    let mut main = BitWriter::new();
    let mut handle = BitWriter::new();
    write_common_entity_header(doc, entity, &mut main, &mut handle)?;
    write_hatch_geometry(
        &HatchGeometry {
            pattern_name,
            solid_fill,
            boundary_paths,
            extrusion: entity.extrusion,
        },
        &mut main,
        &mut handle,
    )?;

    compose_entity_object_slice(entity, AC1015_OBJECT_TYPE_HATCH, "HATCH", &main, &handle)
}

fn write_common_entity_header(
    doc: &CadDocument,
    entity: &Entity,
    main: &mut BitWriter,
    handle: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    let layer_handle = doc
        .layers
        .get(&entity.layer_name)
        .map(|props| props.handle)
        .ok_or_else(|| {
            DwgWriteError::InvalidDocument(format!(
                "entity references unknown layer `{}`",
                entity.layer_name
            ))
        })?;

    write_ac1015_entity_common_minimal(
        EntityCommonMinimal {
            owner_block_handle: entity.owner_handle,
            layer_handle,
            color_index: entity.color_index,
            linetype_scale: entity.linetype_scale,
            lineweight: entity.lineweight,
            invisible: entity.invisible,
        },
        main,
        handle,
    )
}

fn compose_entity_object_slice(
    entity: &Entity,
    object_type: i16,
    family: &'static str,
    main: &BitWriter,
    handle: &BitWriter,
) -> Result<Vec<u8>, DwgWriteError> {
    let header_bit_count = probe_object_header_bit_count(entity, object_type)?;
    let main_size_bits =
        u32::try_from(header_bit_count + main.position_in_bits()).map_err(|_| {
            DwgWriteError::InvalidValue(format!("{family} main_size_bits exceeds u32::MAX"))
        })?;
    let header = ObjectHeader {
        object_type,
        main_size_bits,
        handle: entity.handle,
        handle_code: HANDLE_CODE_HARD_OWNER,
    };
    compose_ac1015_object_slice(header, main, handle)
}

/// Probe the bit count produced by [`write_ac1015_object_header`]
/// for `entity`. Used to size `main_size_bits` so the object slice
/// composer's invariant check passes on the first try.
fn probe_object_header_bit_count(
    entity: &Entity,
    object_type: i16,
) -> Result<usize, DwgWriteError> {
    let probe_header = ObjectHeader {
        object_type,
        main_size_bits: 0,
        handle: entity.handle,
        handle_code: HANDLE_CODE_HARD_OWNER,
    };
    let mut probe = BitWriter::new();
    write_ac1015_object_header(probe_header, &mut probe)?;
    Ok(probe.position_in_bits())
}

/// Static `&'static str` view of `KnownSection::name` for the six
/// AC1015 well-known sections. Local helper so the
/// [`DwgWriteError::SectionTooLarge`] variant (which expects a
/// `&'static str`) can carry a stable label without leaking arbitrary
/// strings.
fn ac1015_section_name(section: KnownSection) -> &'static str {
    match section {
        KnownSection::Header => "AcDb:Header",
        KnownSection::Classes => "AcDb:Classes",
        KnownSection::Handles => "AcDb:Handles",
        KnownSection::ObjFreeSpace => "AcDb:ObjFreeSpace",
        KnownSection::Template => "AcDb:Template",
        KnownSection::AuxHeader => "AcDb:AuxHeader",
    }
}

/// AC1015 locator record number for the six well-known sections.
/// `KnownSection::record_number_from_name` provides the same mapping
/// at runtime via name lookup; this helper avoids that detour for the
/// fixed six-section ordering the writer always emits.
fn ac1015_section_record_number(section: KnownSection) -> u8 {
    match section {
        KnownSection::Header => 0,
        KnownSection::Classes => 1,
        KnownSection::Handles => 2,
        KnownSection::ObjFreeSpace => 3,
        KnownSection::Template => 4,
        KnownSection::AuxHeader => 5,
    }
}

/// Marker (private) helper kept around so the F5.M5 `encode_entity`
/// variants can land additional `EntityData::*` arms without
/// re-deriving the LINE wrapper boilerplate.
#[allow(dead_code)]
fn entity_handle_for_object_header(entity: &Entity) -> Handle {
    entity.handle
}
