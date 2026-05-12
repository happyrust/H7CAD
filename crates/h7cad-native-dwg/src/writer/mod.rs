//! Native DWG writer pipeline (F5.M2 onwards).
//!
//! This module is the writer-side mirror of the reader stack:
//! - `bit_writer.rs` (F5.M1, sibling of this `writer/` directory)
//!   emits the MSB-first bit stream.
//! - `writer/file_header.rs` (F5.M2.T2) emits the AC1015 file header
//!   prefix and section locator directory, the reverse of
//!   [`crate::DwgFileHeader::parse`] + [`crate::SectionMap::parse`].
//! - `writer/section_*.rs` (F5.M2.T3) emit the per-section payloads
//!   for the six AC1015 well-known sections (`Header / Classes /
//!   Handles / ObjFreeSpace / Template / AuxHeader`). At this
//!   milestone every payload is empty (the smallest byte sequence
//!   the reader still classifies as a valid section).
//! - `writer/document.rs` (F5.M2.T4) glues the pieces together as
//!   [`write_dwg`], the writer-side counterpart of [`crate::read_dwg`]
//!   for empty AC1015 documents.
//!
//! Future bricks expand this surface:
//! - F5.M3: real handle map writer (delta-encoded `parse_handle_map`
//!   inverse).
//! - F5.M4: object stream writer (`enrich_with_real_entities`
//!   inverse).
//! - F5.M5: per-entity body writers (22 families of
//!   `entity_*.rs` reader counterparts).
//! - F5.M6: facade switch + feature flag.
//! - F5.M7: AC1018 writer.
//!
//! See `docs/plans/2026-05-08-dwg-next-step-plan.md` §F5 for the
//! staged writer roadmap and
//! `docs/plans/2026-05-09-dwg-section-composer-empty-payloads-plan.md`
//! for this milestone's invariants.

pub mod document;
pub mod entity_arc;
pub mod entity_attrib;
pub mod entity_circle;
pub mod entity_common;
pub mod entity_ellipse;
pub mod entity_face3d;
pub mod entity_hatch;
pub mod entity_insert;
pub mod entity_line;
pub mod entity_lwpolyline;
pub mod entity_mtext;
pub mod entity_point;
pub mod entity_ray;
pub mod entity_solid;
pub mod entity_spline;
pub mod entity_text;
pub mod entity_viewport;
pub mod file_header;
pub mod handle_map;
pub mod object_header;
pub mod object_slice;
pub mod section_aux_header;
pub mod section_classes;
pub mod section_handles;
pub mod section_header;
pub mod section_obj_free_space;
pub mod section_template;

pub use document::{write_dwg, AC1015_EMPTY_DWG_MIN_LEN, AC1015_KNOWN_SECTION_COUNT};
pub use entity_arc::write_arc_geometry;
pub use entity_attrib::write_attrib_geometry;
pub use entity_circle::write_circle_geometry;
pub use entity_common::{write_ac1015_entity_common_minimal, EntityCommonMinimal};
pub use entity_ellipse::write_ellipse_geometry;
pub use entity_face3d::write_face3d_geometry;
pub use entity_hatch::write_hatch_geometry;
pub use entity_insert::write_insert_geometry;
pub use entity_line::write_line_geometry;
pub use entity_lwpolyline::write_lwpolyline_geometry;
pub use entity_mtext::write_mtext_geometry;
pub use entity_point::write_point_geometry;
pub use entity_ray::write_ray_geometry;
pub use entity_solid::write_solid_geometry;
pub use entity_spline::write_spline_geometry;
pub use entity_text::write_text_geometry;
pub use entity_viewport::write_viewport_geometry;
pub use file_header::{
    write_ac1015_file_header_prefix, write_ac1015_section_locator_directory,
    AC1015_FILE_HEADER_PREFIX_LEN, AC1015_SECTION_LOCATOR_ENTRY_LEN,
};
pub use handle_map::{write_ac1015_handle_map_payload, MAX_CHUNK_PAYLOAD};
pub use object_header::{write_ac1015_object_header, write_ac1015_object_self_header};
pub use object_slice::{compose_ac1015_object_slice, CRC_STUB};
pub use section_aux_header::write_ac1015_aux_header_section;
pub use section_classes::write_ac1015_classes_section;
pub use section_handles::write_ac1015_handles_section;
pub use section_header::write_ac1015_header_section;
pub use section_obj_free_space::write_ac1015_obj_free_space_section;
pub use section_template::write_ac1015_template_section;
