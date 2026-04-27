mod bit_reader;
mod entity_arc;
mod entity_attrib;
mod entity_circle;
mod entity_common;
mod entity_dimension;
mod entity_ellipse;
mod entity_hatch;
mod entity_insert;
mod entity_line;
mod entity_lwpolyline;
mod entity_mtext;
mod entity_point;
mod entity_ray;
mod entity_solid;
mod entity_spline;
mod entity_text;
mod entity_viewport;
mod error;
mod file_header;
mod handle_map;
mod known_section;
mod modular;
mod object_header;
mod object_reader;
mod object_stream;
mod pending;
mod reader;
mod resolver;
mod section_map;
mod version;

use h7cad_native_model::CadDocument;
use h7cad_native_model::Entity;
use h7cad_native_model::EntityData;
use h7cad_native_model::Handle;

pub use bit_reader::BitReader;
pub use entity_arc::{read_arc_geometry, ArcGeometry};
pub use entity_circle::{read_circle_geometry, CircleGeometry};
pub use entity_common::{
    dwg_lineweight_from_index, parse_ac1015_entity_common, parse_ac1015_non_entity_common,
    probe_ac1015_entity_common, skip_ac1015_entity_common_main_stream, Ac1015EntityCommonData,
    Ac1015EntityCommonProbeFailure, Ac1015EntityCommonProbeStage, Ac1015NonEntityCommonData,
};
pub use entity_attrib::{
    read_attdef_geometry, read_attrib_geometry, AttDefGeometry, AttribGeometry,
};
pub use entity_dimension::{read_dimension_geometry, DimensionGeometry};
pub use entity_ellipse::{read_ellipse_geometry, EllipseGeometry};
pub use entity_hatch::{read_hatch_geometry, HatchGeometry};
pub use entity_insert::{read_insert_geometry, InsertGeometry};
pub use entity_line::{read_line_geometry, LineGeometry};
pub use entity_lwpolyline::{read_lwpolyline_geometry, LwPolylineGeometry};
pub use entity_mtext::{read_mtext_geometry, MTextGeometry};
pub use entity_point::{read_point_geometry, PointGeometry};
pub use entity_ray::{read_ray_geometry, RayGeometry};
pub use entity_solid::{read_face3d_geometry, read_solid_geometry, Face3DGeometry, SolidGeometry};
pub use entity_spline::{read_spline_geometry, SplineGeometry};
pub use entity_text::{read_text_geometry, TextGeometry};
pub use entity_viewport::{read_viewport_geometry, ViewportGeometry};
pub use error::DwgReadError;
pub use file_header::DwgFileHeader;
pub use handle_map::{parse_handle_map, HandleMapEntry};
pub use known_section::KnownSection;
pub use object_header::{
    read_ac1015_object_header, split_ac1015_object_streams, ObjectHeader,
    HANDLE_CODE_HARD_OWNER,
};
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
    let mut doc = resolve_document(&pending)?;
    enrich_with_real_entities(&mut doc, bytes, &pending);
    Ok(doc)
}

/// Built-in AC1015 object type codes this enrichment pipeline can
/// decode today. Widening the list should only require:
/// 1. a new `entity_<kind>.rs` module with a pure-function decoder,
/// 2. an additional arm in [`try_decode_entity_body`],
/// 3. a new case in the `EntityData` construction below.
const TEXT_OBJECT_TYPE: i16 = 1;
const ATTRIB_OBJECT_TYPE: i16 = 2;
const ATTDEF_OBJECT_TYPE: i16 = 3;
const INSERT_OBJECT_TYPE: i16 = 7;
const ARC_OBJECT_TYPE: i16 = 17;
const CIRCLE_OBJECT_TYPE: i16 = 18;
const LINE_OBJECT_TYPE: i16 = 19;
const DIM_ORDINATE_OBJECT_TYPE: i16 = 20;
const DIM_LINEAR_OBJECT_TYPE: i16 = 21;
const DIM_ALIGNED_OBJECT_TYPE: i16 = 22;
const DIM_ANG3PT_OBJECT_TYPE: i16 = 23;
const DIM_ANG2LN_OBJECT_TYPE: i16 = 24;
const DIM_RADIUS_OBJECT_TYPE: i16 = 25;
const DIM_DIAMETER_OBJECT_TYPE: i16 = 26;
const POINT_OBJECT_TYPE: i16 = 27;
const FACE3D_OBJECT_TYPE: i16 = 28;
const SOLID_OBJECT_TYPE: i16 = 31;
const VIEWPORT_OBJECT_TYPE: i16 = 34;
const ELLIPSE_OBJECT_TYPE: i16 = 35;
const SPLINE_OBJECT_TYPE: i16 = 36;
const RAY_OBJECT_TYPE: i16 = 38;
const XLINE_OBJECT_TYPE: i16 = 40;
const MTEXT_OBJECT_TYPE: i16 = 44;
const LWPOLYLINE_OBJECT_TYPE: i16 = 77;
const HATCH_OBJECT_TYPE: i16 = 78;

#[derive(Debug, Default)]
struct SymbolNameMaps {
    layer_by_handle: std::collections::BTreeMap<Handle, String>,
    style_by_handle: std::collections::BTreeMap<Handle, String>,
    linetype_by_handle: std::collections::BTreeMap<Handle, String>,
    block_by_handle: std::collections::BTreeMap<Handle, String>,
}

#[derive(Debug, Clone)]
struct DecodedEntity {
    data: EntityData,
    owner_handle: Handle,
    layer_name: String,
    linetype_name: String,
    linetype_scale: f64,
    color_index: i16,
    lineweight: i16,
    invisible: bool,
    thickness: f64,
    extrusion: [f64; 3],
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ac1015RecoveryFailureKind {
    SliceMiss,
    HeaderFail,
    HandleMismatch,
    CommonDecodeFail,
    BodyDecodeFail,
    UnsupportedType,
}

impl Ac1015RecoveryFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SliceMiss => "slice_miss",
            Self::HeaderFail => "header_fail",
            Self::HandleMismatch => "handle_mismatch",
            Self::CommonDecodeFail => "common_decode_fail",
            Self::BodyDecodeFail => "body_decode_fail",
            Self::UnsupportedType => "unsupported_type",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ac1015RecoveryFailure {
    pub handle: Handle,
    pub object_type: Option<i16>,
    pub family: Option<&'static str>,
    pub kind: Ac1015RecoveryFailureKind,
    pub stage: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ac1015TargetedTraceFirstMissingRecord {
    SplitObjectStreams,
    CommonEntityDecode,
    EntityBodyDecode,
}

impl Ac1015TargetedTraceFirstMissingRecord {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SplitObjectStreams => "split_ac1015_object_streams",
            Self::CommonEntityDecode => "parse_ac1015_entity_common",
            Self::EntityBodyDecode => "try_decode_entity_body_with_reason",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ac1015TargetedFailureTrace {
    pub handle: Handle,
    pub object_type_hint: Option<i16>,
    pub family_hint: Option<&'static str>,
    pub stage_before_fallback: Option<&'static str>,
    pub first_missing_record: Option<Ac1015TargetedTraceFirstMissingRecord>,
    pub common_probe_stage: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ac1015RecoveryDiagnostics {
    pub recovered_total: usize,
    pub recovered_by_family: std::collections::BTreeMap<&'static str, usize>,
    pub failure_counts: std::collections::BTreeMap<Ac1015RecoveryFailureKind, usize>,
    pub failure_counts_by_family: std::collections::BTreeMap<&'static str, std::collections::BTreeMap<Ac1015RecoveryFailureKind, usize>>,
    pub failures: Vec<Ac1015RecoveryFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ac1015FailureAttributionHint {
    object_type: Option<i16>,
    family: Option<&'static str>,
    probe_stage: Option<&'static str>,
}

impl Ac1015FailureAttributionHint {
    fn unresolved() -> Self {
        Self {
            object_type: None,
            family: None,
            probe_stage: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ac1015PreheaderObjectTypeHint {
    pub handle: Handle,
    pub offset: i64,
    pub object_type: Option<i16>,
    pub family: Option<&'static str>,
    pub source: &'static str,
}

impl Ac1015RecoveryDiagnostics {
    fn record_recovered(&mut self, family: &'static str) {
        self.recovered_total += 1;
        *self.recovered_by_family.entry(family).or_insert(0) += 1;
    }

    fn record_failure(
        &mut self,
        handle: Handle,
        object_type: Option<i16>,
        family: Option<&'static str>,
        kind: Ac1015RecoveryFailureKind,
        stage: Option<&'static str>,
    ) {
        *self.failure_counts.entry(kind).or_insert(0) += 1;
        if let Some(family) = family {
            *self.failure_counts_by_family.entry(family).or_default().entry(kind).or_insert(0) += 1;
        }
        self.failures.push(Ac1015RecoveryFailure {
            handle,
            object_type,
            family,
            kind,
            stage,
        });
    }

    pub fn representative_failures_by_family_and_kind(
        &self,
        families: &[&'static str],
        kinds: &[Ac1015RecoveryFailureKind],
        per_bucket: usize,
    ) -> std::collections::BTreeMap<
        &'static str,
        std::collections::BTreeMap<Ac1015RecoveryFailureKind, Vec<Ac1015RecoveryFailure>>,
    > {
        let family_filter: std::collections::BTreeSet<&'static str> = families.iter().copied().collect();
        let kind_filter: std::collections::BTreeSet<Ac1015RecoveryFailureKind> = kinds.iter().copied().collect();
        let mut grouped = std::collections::BTreeMap::<
            &'static str,
            std::collections::BTreeMap<Ac1015RecoveryFailureKind, Vec<Ac1015RecoveryFailure>>,
        >::new();

        for failure in &self.failures {
            let Some(family) = failure.family else {
                continue;
            };
            if !family_filter.contains(family) || !kind_filter.contains(&failure.kind) {
                continue;
            }
            let bucket = grouped
                .entry(family)
                .or_default()
                .entry(failure.kind)
                .or_default();
            if bucket.len() < per_bucket {
                bucket.push(failure.clone());
            }
        }

        grouped
    }

    fn promote_header_failures_to_supported_families(
        &mut self,
        family_hints_by_handle: &std::collections::BTreeMap<Handle, Ac1015FailureAttributionHint>,
    ) {
        for failure in &mut self.failures {
            if failure.family.is_some() {
                continue;
            }
            if let Some(hint) = family_hints_by_handle.get(&failure.handle).copied() {
                if failure.object_type.is_none() {
                    failure.object_type = hint.object_type;
                }
                let Some(family) = hint.family else {
                    continue;
                };
                failure.family = Some(family);
                *self
                    .failure_counts_by_family
                    .entry(family)
                    .or_default()
                    .entry(failure.kind)
                    .or_insert(0) += 1;
            }
        }
    }
}


/// Walk the pending handle map on the real file bytes and append any
/// successfully decoded built-in entities to `doc.entities`.
///
/// This path is **best-effort**: handles whose object slice is
/// out-of-range, whose header fails to decode, whose header handle
/// does not match the map entry, or whose common-entity/entity body
/// decoders run off the declared bit ranges are skipped. The goal is
/// to keep the synthetic-fixture suite fail-closed while steadily
/// improving recovery on real AC1015 samples.

pub fn collect_ac1015_recovery_diagnostics(
    bytes: &[u8],
    pending: &pending::PendingDocument,
) -> Ac1015RecoveryDiagnostics {
    collect_ac1015_recovery_diagnostics_with_known_successes(bytes, pending, std::iter::empty())
}

pub fn collect_ac1015_recovery_diagnostics_with_known_successes(
    bytes: &[u8],
    pending: &pending::PendingDocument,
    known_successes: impl IntoIterator<Item = &'static str>,
) -> Ac1015RecoveryDiagnostics {
    let mut diagnostics = Ac1015RecoveryDiagnostics::default();
    for family in known_successes {
        diagnostics.record_recovered(family);
    }
    if pending.handle_offsets.is_empty() {
        return diagnostics;
    }

    let cursor = object_stream::ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let symbol_names = collect_symbol_name_maps(bytes, pending);
    let supported_family_hints =
        collect_supported_family_hints(bytes, pending, &cursor, &symbol_names);
    let mut traced_fallback_failures =
        std::collections::BTreeMap::<Handle, Ac1015FallbackFailureStage>::new();
    for entry in pending.handle_offsets.iter() {
        let hint = supported_family_hints
            .get(&entry.handle)
            .copied()
            .unwrap_or_else(Ac1015FailureAttributionHint::unresolved);
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            diagnostics.record_failure(
                entry.handle,
                hint.object_type,
                hint.family,
                Ac1015RecoveryFailureKind::SliceMiss,
                hint.probe_stage.or(Some("object_stream_split")),
            );
            continue;
        };
        let Ok((obj_header, mut main_reader, mut handle_reader)) = object_header::split_ac1015_object_streams(slice) else {
            diagnostics.record_failure(
                entry.handle,
                hint.object_type,
                hint.family,
                Ac1015RecoveryFailureKind::HeaderFail,
                hint.probe_stage.or(Some("object_header_decode")),
            );
            continue;
        };
        if obj_header.handle != entry.handle {
            diagnostics.record_failure(
                entry.handle,
                hint.object_type.or(Some(obj_header.object_type)),
                hint.family.or_else(|| object_type_family(obj_header.object_type)),
                Ac1015RecoveryFailureKind::HandleMismatch,
                hint.probe_stage.or(Some("object_header_decode")),
            );
            continue;
        }
        match try_decode_entity_body_with_reason(
            obj_header.object_type,
            obj_header.handle,
            &mut main_reader,
            &mut handle_reader,
            &symbol_names,
        ) {
            Ok(_) => {}
            Err(kind) => diagnostics.record_failure(
                entry.handle,
                hint.object_type.or(Some(obj_header.object_type)),
                hint.family.or_else(|| object_type_family(obj_header.object_type)),
                kind,
                hint.probe_stage.or(Some(ac1015_failure_stage(kind))),
            ),
        }
    }
    diagnostics.promote_header_failures_to_supported_families(&supported_family_hints);
    for (handle, hint) in supported_family_hints.iter() {
        let Some(family) = hint.family else {
            continue;
        };
        let has_family_failure = diagnostics
            .failures
            .iter()
            .any(|failure| failure.handle == *handle && failure.family == Some(family));
        if has_family_failure {
            continue;
        }
        let fallback_stage = trace_ac1015_supported_family_failure_stage(
            *handle,
            hint.object_type,
            hint.family,
            &cursor,
            &symbol_names,
        );
        traced_fallback_failures.insert(*handle, fallback_stage);
        diagnostics.record_failure(
            *handle,
            fallback_stage.object_type.or(hint.object_type),
            Some(family),
            fallback_stage.kind.unwrap_or(Ac1015RecoveryFailureKind::CommonDecodeFail),
            fallback_stage
                .stage
                .or(hint.probe_stage)
                .or(Some("preheader_supported_hint")),
        );
    }
    for failure in &mut diagnostics.failures {
        if let Some(traced) = traced_fallback_failures.get(&failure.handle).copied() {
            if traced.stage.is_some() {
                failure.stage = traced.stage;
            }
            if traced.kind.is_some() {
                failure.kind = traced.kind.unwrap();
            }
            if traced.object_type.is_some() {
                failure.object_type = traced.object_type;
            }
        }
    }
    diagnostics
}

pub fn trace_ac1015_targeted_failure_before_fallback(
    bytes: &[u8],
    pending: &pending::PendingDocument,
    handles: &[Handle],
) -> Vec<Ac1015TargetedFailureTrace> {
    if pending.handle_offsets.is_empty() {
        return handles
            .iter()
            .copied()
            .map(|handle| Ac1015TargetedFailureTrace {
                handle,
                object_type_hint: None,
                family_hint: None,
                stage_before_fallback: None,
                first_missing_record: Some(Ac1015TargetedTraceFirstMissingRecord::SplitObjectStreams),
                common_probe_stage: None,
            })
            .collect();
    }

    let cursor = object_stream::ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let symbol_names = collect_symbol_name_maps(bytes, pending);
    let supported_family_hints =
        collect_supported_family_hints(bytes, pending, &cursor, &symbol_names);

    handles
        .iter()
        .copied()
        .map(|handle| {
            let hint = supported_family_hints
                .get(&handle)
                .copied()
                .unwrap_or_else(Ac1015FailureAttributionHint::unresolved);
            let mut trace = Ac1015TargetedFailureTrace {
                handle,
                object_type_hint: hint.object_type,
                family_hint: hint.family,
                stage_before_fallback: None,
                first_missing_record: None,
                common_probe_stage: None,
            };

            let Some(slice) = cursor.object_slice_by_handle(handle) else {
                trace.first_missing_record =
                    Some(Ac1015TargetedTraceFirstMissingRecord::SplitObjectStreams);
                return trace;
            };
            let Ok((obj_header, mut main_reader, mut handle_reader)) =
                object_header::split_ac1015_object_streams(slice)
            else {
                trace.first_missing_record =
                    Some(Ac1015TargetedTraceFirstMissingRecord::SplitObjectStreams);
                return trace;
            };
            trace.object_type_hint = trace.object_type_hint.or(Some(obj_header.object_type));
            trace.family_hint = trace
                .family_hint
                .or_else(|| object_type_family(obj_header.object_type));
            if obj_header.handle != handle {
                trace.first_missing_record =
                    Some(Ac1015TargetedTraceFirstMissingRecord::SplitObjectStreams);
                return trace;
            }

            let probe_result =
                probe_ac1015_entity_common(&mut main_reader, &mut handle_reader, handle);
            let common_probe_failed = match probe_result {
                Ok(_) => {
                    trace.common_probe_stage = Some("ok");
                    false
                }
                Err(probe) => {
                    trace.common_probe_stage = Some(probe.stage.as_str());
                    true
                }
            };

            match try_decode_entity_body_with_reason(
                obj_header.object_type,
                obj_header.handle,
                &mut main_reader,
                &mut handle_reader,
                &symbol_names,
            ) {
                Ok(_) => {}
        Err(Ac1015RecoveryFailureKind::CommonDecodeFail) => {
            trace.stage_before_fallback = Some(if common_probe_failed {
                "common_entity_decode"
            } else {
                "entity_body_decode"
            });
            trace.first_missing_record = Some(if common_probe_failed {
                Ac1015TargetedTraceFirstMissingRecord::CommonEntityDecode
            } else {
                Ac1015TargetedTraceFirstMissingRecord::EntityBodyDecode
            });
        }
                Err(Ac1015RecoveryFailureKind::BodyDecodeFail) => {
                    trace.stage_before_fallback = Some("entity_body_decode");
                    trace.first_missing_record =
                        Some(Ac1015TargetedTraceFirstMissingRecord::EntityBodyDecode);
                }
                Err(_) => {}
            }

            trace
        })
        .collect()
}

fn ac1015_failure_stage(kind: Ac1015RecoveryFailureKind) -> &'static str {
    match kind {
        Ac1015RecoveryFailureKind::SliceMiss => "object_stream_split",
        Ac1015RecoveryFailureKind::HeaderFail | Ac1015RecoveryFailureKind::HandleMismatch => {
            "object_header_decode"
        }
        Ac1015RecoveryFailureKind::CommonDecodeFail => "common_entity_decode",
        Ac1015RecoveryFailureKind::BodyDecodeFail => "entity_body_decode",
        Ac1015RecoveryFailureKind::UnsupportedType => "body_dispatch",
    }
}

fn enrich_with_real_entities(
    doc: &mut CadDocument,
    bytes: &[u8],
    pending: &pending::PendingDocument,
) {
    if pending.handle_offsets.is_empty() {
        return;
    }
    let cursor = object_stream::ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let symbol_names = collect_symbol_name_maps(bytes, pending);
    for entry in pending.handle_offsets.iter() {
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            continue;
        };
        let Ok((obj_header, mut main_reader, mut handle_reader)) =
            object_header::split_ac1015_object_streams(slice)
        else {
            continue;
        };
        if obj_header.handle != entry.handle {
            continue;
        }
        let Some(decoded) = try_decode_entity_body(
            obj_header.object_type,
            obj_header.handle,
            &mut main_reader,
            &mut handle_reader,
            &symbol_names,
        ) else {
            continue;
        };
        let mut entity = Entity::new(decoded.data);
        entity.handle = entry.handle;
        entity.owner_handle = decoded.owner_handle;
        entity.layer_name = decoded.layer_name;
        entity.linetype_name = decoded.linetype_name;
        entity.linetype_scale = decoded.linetype_scale;
        entity.color_index = decoded.color_index;
        entity.lineweight = decoded.lineweight;
        entity.invisible = decoded.invisible;
        entity.thickness = decoded.thickness;
        entity.extrusion = decoded.extrusion;
        let _ = doc.add_entity(entity);
    }
}

fn try_decode_entity_body(
    object_type: i16,
    object_handle: Handle,
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    symbol_names: &SymbolNameMaps,
) -> Option<DecodedEntity> {
    try_decode_entity_body_with_reason(
        object_type,
        object_handle,
        main_reader,
        handle_reader,
        symbol_names,
    )
    .ok()
}

fn try_decode_entity_body_with_reason(
    object_type: i16,
    object_handle: Handle,
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    symbol_names: &SymbolNameMaps,
) -> Result<DecodedEntity, Ac1015RecoveryFailureKind> {
    let common = entity_common::parse_ac1015_entity_common(main_reader, handle_reader, object_handle)
        .map_err(|_| Ac1015RecoveryFailureKind::CommonDecodeFail)?;
    let layer_name = resolve_layer_name(common.layer_handle, symbol_names);
    let linetype_name = resolve_linetype_name(
        common.linetype_flags,
        common.linetype_handle,
        symbol_names,
    );
    object_type_family(object_type).ok_or(Ac1015RecoveryFailureKind::UnsupportedType)?;
    let (data, thickness, extrusion) = match object_type {
        LINE_OBJECT_TYPE => {
            let geom = entity_line::read_line_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Line {
                    start: geom.start,
                    end: geom.end,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        ARC_OBJECT_TYPE => {
            let geom = entity_arc::read_arc_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Arc {
                    center: geom.center,
                    radius: geom.radius,
                    start_angle: geom.start_angle,
                    end_angle: geom.end_angle,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        CIRCLE_OBJECT_TYPE => {
            let geom = entity_circle::read_circle_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Circle {
                    center: geom.center,
                    radius: geom.radius,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        POINT_OBJECT_TYPE => {
            let geom = entity_point::read_point_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Point {
                    position: geom.position,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        TEXT_OBJECT_TYPE => {
            let geom = entity_text::read_text_geometry(main_reader, handle_reader, object_handle)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Text {
                    insertion: geom.insertion,
                    height: geom.height,
                    value: geom.value,
                    rotation: geom.rotation,
                    style_name: resolve_style_name(geom.style_handle, symbol_names),
                    width_factor: geom.width_factor,
                    oblique_angle: geom.oblique_angle,
                    horizontal_alignment: geom.horizontal_alignment,
                    vertical_alignment: geom.vertical_alignment,
                    alignment_point: geom.alignment_point,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        LWPOLYLINE_OBJECT_TYPE => {
            let geom = entity_lwpolyline::read_lwpolyline_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::LwPolyline {
                    vertices: geom.vertices,
                    closed: geom.closed,
                    constant_width: geom.constant_width,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        HATCH_OBJECT_TYPE => {
            let geom = entity_hatch::read_hatch_geometry(main_reader, handle_reader, object_handle)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Hatch {
                    pattern_name: geom.pattern_name,
                    solid_fill: geom.solid_fill,
                    boundary_paths: geom.boundary_paths,
                },
                0.0,
                geom.extrusion,
            )
        }
        ATTRIB_OBJECT_TYPE => {
            let geom =
                entity_attrib::read_attrib_geometry(main_reader, handle_reader, object_handle)
                    .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Attrib {
                    tag: geom.tag,
                    value: geom.value,
                    insertion: geom.insertion,
                    height: geom.height,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        ATTDEF_OBJECT_TYPE => {
            let geom =
                entity_attrib::read_attdef_geometry(main_reader, handle_reader, object_handle)
                    .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::AttDef {
                    tag: geom.tag,
                    prompt: geom.prompt,
                    default_value: geom.default_value,
                    insertion: geom.insertion,
                    height: geom.height,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        DIM_ORDINATE_OBJECT_TYPE
        | DIM_LINEAR_OBJECT_TYPE
        | DIM_ALIGNED_OBJECT_TYPE
        | DIM_ANG3PT_OBJECT_TYPE
        | DIM_ANG2LN_OBJECT_TYPE
        | DIM_RADIUS_OBJECT_TYPE
        | DIM_DIAMETER_OBJECT_TYPE => {
            let geom = entity_dimension::read_dimension_geometry(
                object_type,
                main_reader,
                handle_reader,
                object_handle,
            )
            .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            let block_name = resolve_block_name(geom.block_handle, symbol_names);
            let style_name = resolve_style_name(geom.style_handle, symbol_names);
            (
                EntityData::Dimension {
                    dim_type: geom.dim_type,
                    block_name,
                    style_name,
                    definition_point: geom.definition_point,
                    text_midpoint: geom.text_midpoint,
                    text_override: geom.text_override,
                    attachment_point: geom.attachment_point,
                    measurement: geom.measurement,
                    text_rotation: geom.text_rotation,
                    horizontal_direction: geom.horizontal_direction,
                    flip_arrow1: geom.flip_arrow1,
                    flip_arrow2: geom.flip_arrow2,
                    first_point: geom.first_point,
                    second_point: geom.second_point,
                    angle_vertex: geom.angle_vertex,
                    dimension_arc: geom.dimension_arc,
                    leader_length: geom.leader_length,
                    rotation: geom.rotation,
                    ext_line_rotation: geom.ext_line_rotation,
                },
                0.0,
                geom.extrusion,
            )
        }
        INSERT_OBJECT_TYPE => {
            let geom =
                entity_insert::read_insert_geometry(main_reader, handle_reader, object_handle)
                    .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            let block_name = resolve_block_name(geom.block_header_handle, symbol_names);
            (
                EntityData::Insert {
                    block_name,
                    insertion: geom.insertion,
                    scale: geom.scale,
                    rotation: geom.rotation,
                    has_attribs: geom.has_attribs,
                    attribs: Vec::new(),
                },
                0.0,
                geom.extrusion,
            )
        }
        MTEXT_OBJECT_TYPE => {
            let geom =
                entity_mtext::read_mtext_geometry(main_reader, handle_reader, object_handle)
                    .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::MText {
                    insertion: geom.insertion,
                    height: geom.height,
                    width: geom.rect_width,
                    rectangle_height: if geom.rect_height > 0.0 {
                        Some(geom.rect_height)
                    } else {
                        None
                    },
                    value: geom.value,
                    rotation: geom.rotation,
                    style_name: resolve_style_name(geom.style_handle, symbol_names),
                    attachment_point: geom.attachment_point,
                    line_spacing_factor: geom.line_spacing_factor,
                    drawing_direction: geom.drawing_direction,
                },
                0.0,
                geom.extrusion,
            )
        }
        ELLIPSE_OBJECT_TYPE => {
            let geom = entity_ellipse::read_ellipse_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Ellipse {
                    center: geom.center,
                    major_axis: geom.major_axis,
                    ratio: geom.ratio,
                    start_param: geom.start_param,
                    end_param: geom.end_param,
                },
                0.0,
                geom.extrusion,
            )
        }
        RAY_OBJECT_TYPE => {
            let geom = entity_ray::read_ray_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Ray {
                    origin: geom.origin,
                    direction: geom.direction,
                },
                0.0,
                [0.0, 0.0, 1.0],
            )
        }
        XLINE_OBJECT_TYPE => {
            let geom = entity_ray::read_ray_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::XLine {
                    origin: geom.origin,
                    direction: geom.direction,
                },
                0.0,
                [0.0, 0.0, 1.0],
            )
        }
        FACE3D_OBJECT_TYPE => {
            let geom = entity_solid::read_face3d_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Face3D {
                    corners: geom.corners,
                    invisible_edges: geom.invisible_edges,
                },
                0.0,
                [0.0, 0.0, 1.0],
            )
        }
        SOLID_OBJECT_TYPE => {
            let geom = entity_solid::read_solid_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Solid {
                    corners: geom.corners,
                    normal: geom.extrusion,
                    thickness: geom.thickness,
                },
                geom.thickness,
                geom.extrusion,
            )
        }
        VIEWPORT_OBJECT_TYPE => {
            let geom = entity_viewport::read_viewport_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Viewport {
                    center: geom.center,
                    width: geom.width,
                    height: geom.height,
                },
                0.0,
                [0.0, 0.0, 1.0],
            )
        }
        SPLINE_OBJECT_TYPE => {
            let geom = entity_spline::read_spline_geometry(main_reader)
                .map_err(|_| Ac1015RecoveryFailureKind::BodyDecodeFail)?;
            (
                EntityData::Spline {
                    degree: geom.degree,
                    closed: geom.closed,
                    knots: geom.knots,
                    control_points: geom.control_points,
                    weights: geom.weights,
                    fit_points: geom.fit_points,
                    start_tangent: geom.start_tangent,
                    end_tangent: geom.end_tangent,
                },
                0.0,
                [0.0, 0.0, 1.0],
            )
        }
        _ => return Err(Ac1015RecoveryFailureKind::UnsupportedType),
    };

    Ok(DecodedEntity {
        data,
        owner_handle: common.owner_handle,
        layer_name,
        linetype_name,
        linetype_scale: common.linetype_scale,
        color_index: common.color_index,
        lineweight: common.lineweight,
        invisible: common.invisible,
        thickness,
        extrusion,
    })
}

fn object_type_family(object_type: i16) -> Option<&'static str> {
    match object_type {
        TEXT_OBJECT_TYPE => Some("TEXT"),
        ATTRIB_OBJECT_TYPE => Some("ATTRIB"),
        ATTDEF_OBJECT_TYPE => Some("ATTDEF"),
        INSERT_OBJECT_TYPE => Some("INSERT"),
        LINE_OBJECT_TYPE => Some("LINE"),
        CIRCLE_OBJECT_TYPE => Some("CIRCLE"),
        ARC_OBJECT_TYPE => Some("ARC"),
        POINT_OBJECT_TYPE => Some("POINT"),
        DIM_ORDINATE_OBJECT_TYPE
        | DIM_LINEAR_OBJECT_TYPE
        | DIM_ALIGNED_OBJECT_TYPE
        | DIM_ANG3PT_OBJECT_TYPE
        | DIM_ANG2LN_OBJECT_TYPE
        | DIM_RADIUS_OBJECT_TYPE
        | DIM_DIAMETER_OBJECT_TYPE => Some("DIMENSION"),
        FACE3D_OBJECT_TYPE => Some("3DFACE"),
        SOLID_OBJECT_TYPE => Some("SOLID"),
        VIEWPORT_OBJECT_TYPE => Some("VIEWPORT"),
        ELLIPSE_OBJECT_TYPE => Some("ELLIPSE"),
        SPLINE_OBJECT_TYPE => Some("SPLINE"),
        RAY_OBJECT_TYPE => Some("RAY"),
        XLINE_OBJECT_TYPE => Some("XLINE"),
        MTEXT_OBJECT_TYPE => Some("MTEXT"),
        LWPOLYLINE_OBJECT_TYPE => Some("LWPOLYLINE"),
        HATCH_OBJECT_TYPE => Some("HATCH"),
        _ => None,
    }
}

fn collect_supported_family_hints(
    bytes: &[u8],
    pending: &pending::PendingDocument,
    cursor: &object_stream::ObjectStreamCursor<'_>,
    symbol_names: &SymbolNameMaps,
) -> std::collections::BTreeMap<Handle, Ac1015FailureAttributionHint> {
    let mut hinted = std::collections::BTreeMap::new();

    for entry in pending.handle_offsets.iter() {
        let mut hint = object_type_hint_from_offset(bytes, entry.offset)
            .map(|object_type| Ac1015FailureAttributionHint {
                object_type: Some(object_type),
                family: object_type_family(object_type),
                probe_stage: None,
            })
            .unwrap_or_else(Ac1015FailureAttributionHint::unresolved);
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            hinted.insert(entry.handle, hint);
            continue;
        };
        let Ok((obj_header, mut main_reader)) = object_header::read_ac1015_object_header(slice) else {
            hinted.insert(entry.handle, hint);
            continue;
        };
        if hint.object_type.is_none() {
            hint.object_type = Some(obj_header.object_type);
        }
        if hint.family.is_none() {
            hint.family = object_type_family(obj_header.object_type);
        }
        if obj_header.handle != entry.handle {
            hinted.insert(entry.handle, hint);
            continue;
        }
        let family = match semantic_supported_family_hint(
            obj_header.object_type,
            entry.handle,
            &mut main_reader,
            symbol_names,
        ) {
            Some(family) => Some(family),
            None => object_type_family(obj_header.object_type),
        };
        hinted.insert(
            entry.handle,
            Ac1015FailureAttributionHint {
                object_type: hint.object_type.or(Some(obj_header.object_type)),
                family: family.or(hint.family),
                probe_stage: common_body_failure_stage_for_supported_family(
                    obj_header.object_type,
                    obj_header.handle,
                    slice,
                    symbol_names,
                )
                .or(hint.probe_stage),
            },
        );
    }

    hinted
}

fn common_body_failure_stage_for_supported_family(
    object_type: i16,
    object_handle: Handle,
    slice: &[u8],
    symbol_names: &SymbolNameMaps,
) -> Option<&'static str> {
    object_type_family(object_type)?;
    let (_, mut main_reader, mut handle_reader) =
        object_header::split_ac1015_object_streams(slice).ok()?;
    let probe_result =
        probe_ac1015_entity_common(&mut main_reader, &mut handle_reader, object_handle);
    let common_probe_failed = probe_result.is_err();
    match try_decode_entity_body_with_reason(
        object_type,
        object_handle,
        &mut main_reader,
        &mut handle_reader,
        symbol_names,
    ) {
        Ok(_) => None,
        Err(Ac1015RecoveryFailureKind::CommonDecodeFail) => Some(if common_probe_failed {
            "common_entity_decode"
        } else {
            "entity_body_decode"
        }),
        Err(kind @ Ac1015RecoveryFailureKind::BodyDecodeFail) => Some(ac1015_failure_stage(kind)),
        Err(kind @ Ac1015RecoveryFailureKind::UnsupportedType) => Some(ac1015_failure_stage(kind)),
        Err(_) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ac1015FallbackFailureStage {
    object_type: Option<i16>,
    kind: Option<Ac1015RecoveryFailureKind>,
    stage: Option<&'static str>,
}

fn trace_ac1015_supported_family_failure_stage(
    handle: Handle,
    hinted_object_type: Option<i16>,
    hinted_family: Option<&'static str>,
    cursor: &object_stream::ObjectStreamCursor<'_>,
    symbol_names: &SymbolNameMaps,
) -> Ac1015FallbackFailureStage {
    let Some(slice) = cursor.object_slice_by_handle(handle) else {
        return Ac1015FallbackFailureStage {
            object_type: hinted_object_type,
            kind: None,
            stage: None,
        };
    };
    let Ok((obj_header, mut main_reader, mut handle_reader)) =
        object_header::split_ac1015_object_streams(slice)
    else {
        return Ac1015FallbackFailureStage {
            object_type: hinted_object_type,
            kind: None,
            stage: None,
        };
    };
    if obj_header.handle != handle {
        return Ac1015FallbackFailureStage {
            object_type: Some(obj_header.object_type).or(hinted_object_type),
            kind: None,
            stage: None,
        };
    }
    let object_type = Some(obj_header.object_type).or(hinted_object_type);
    let family = hinted_family.or_else(|| object_type_family(obj_header.object_type));
    if family.is_none() {
        return Ac1015FallbackFailureStage {
            object_type,
            kind: None,
            stage: None,
        };
    }
    let probe_result =
        probe_ac1015_entity_common(&mut main_reader, &mut handle_reader, obj_header.handle);
    let common_probe_failed = probe_result.is_err();
    match try_decode_entity_body_with_reason(
        obj_header.object_type,
        obj_header.handle,
        &mut main_reader,
        &mut handle_reader,
        symbol_names,
    ) {
        Ok(_) => Ac1015FallbackFailureStage {
            object_type,
            kind: None,
            stage: None,
        },
        Err(kind @ Ac1015RecoveryFailureKind::CommonDecodeFail) => Ac1015FallbackFailureStage {
            object_type,
            kind: Some(if common_probe_failed {
                kind
            } else {
                Ac1015RecoveryFailureKind::BodyDecodeFail
            }),
            stage: Some(if common_probe_failed {
                "common_entity_decode"
            } else {
                "entity_body_decode"
            }),
        },
        Err(kind @ Ac1015RecoveryFailureKind::BodyDecodeFail) => Ac1015FallbackFailureStage {
            object_type,
            kind: Some(kind),
            stage: Some(ac1015_failure_stage(kind)),
        },
        Err(kind @ Ac1015RecoveryFailureKind::UnsupportedType) => Ac1015FallbackFailureStage {
            object_type,
            kind: Some(if common_probe_failed {
                Ac1015RecoveryFailureKind::CommonDecodeFail
            } else {
                kind
            }),
            stage: Some(if common_probe_failed {
                "common_entity_decode"
            } else {
                ac1015_failure_stage(kind)
            }),
        },
        Err(_) => Ac1015FallbackFailureStage {
            object_type,
            kind: None,
            stage: None,
        },
    }
}

pub fn collect_ac1015_preheader_object_type_hints(
    bytes: &[u8],
    pending: &pending::PendingDocument,
) -> Vec<Ac1015PreheaderObjectTypeHint> {
    if pending.handle_offsets.is_empty() {
        return Vec::new();
    }

    let cursor = object_stream::ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let symbol_names = collect_symbol_name_maps(bytes, pending);
    pending
        .handle_offsets
        .iter()
        .map(|entry| {
            let hint = if let Some(object_type) = object_type_hint_from_offset(bytes, entry.offset) {
                Ac1015PreheaderObjectTypeHint {
                    handle: entry.handle,
                    offset: entry.offset,
                    object_type: Some(object_type),
                    family: object_type_family(object_type),
                    source: "offset_window_le_type",
                }
            } else if let Some(slice) = cursor.object_slice_by_handle(entry.handle) {
                match object_header::read_ac1015_object_header(slice) {
                    Ok((obj_header, mut main_reader)) if obj_header.handle == entry.handle => {
                        let family = semantic_supported_family_hint(
                            obj_header.object_type,
                            entry.handle,
                            &mut main_reader,
                            &symbol_names,
                        )
                        .or_else(|| object_type_family(obj_header.object_type));
                        Ac1015PreheaderObjectTypeHint {
                            handle: entry.handle,
                            offset: entry.offset,
                            object_type: Some(obj_header.object_type),
                            family,
                            source: "object_header",
                        }
                    }
                    _ => Ac1015PreheaderObjectTypeHint {
                        handle: entry.handle,
                        offset: entry.offset,
                        object_type: None,
                        family: None,
                        source: "unresolved",
                    },
                }
            } else {
                Ac1015PreheaderObjectTypeHint {
                    handle: entry.handle,
                    offset: entry.offset,
                    object_type: None,
                    family: None,
                    source: "unresolved",
                }
            };
            hint
        })
        .collect()
}

fn object_type_hint_from_offset(bytes: &[u8], offset: i64) -> Option<i16> {
    let offset = usize::try_from(offset).ok()?;
    let start = offset.checked_sub(4)?;
    let marker = bytes.get(start..offset)?;
    if marker != b"\r\0\0\0" {
        return None;
    }
    let type_bytes = bytes.get(offset..offset + 2)?;
    Some(i16::from_le_bytes([type_bytes[0], type_bytes[1]]))
}

fn semantic_supported_family_hint(
    object_type: i16,
    object_handle: Handle,
    reader: &mut BitReader<'_>,
    symbol_names: &SymbolNameMaps,
) -> Option<&'static str> {
    let family = object_type_family(object_type)?;
    if matches!(
        family,
        "TEXT" | "HATCH" | "MTEXT" | "INSERT" | "DIMENSION" | "ATTRIB" | "ATTDEF"
    ) {
        return None;
    }

    let _ = (object_handle, reader, symbol_names);
    match object_type {
        LINE_OBJECT_TYPE => Some("LINE"),
        CIRCLE_OBJECT_TYPE => Some("CIRCLE"),
        ARC_OBJECT_TYPE => Some("ARC"),
        POINT_OBJECT_TYPE => Some("POINT"),
        FACE3D_OBJECT_TYPE => Some("3DFACE"),
        SOLID_OBJECT_TYPE => Some("SOLID"),
        VIEWPORT_OBJECT_TYPE => Some("VIEWPORT"),
        ELLIPSE_OBJECT_TYPE => Some("ELLIPSE"),
        SPLINE_OBJECT_TYPE => Some("SPLINE"),
        RAY_OBJECT_TYPE => Some("RAY"),
        XLINE_OBJECT_TYPE => Some("XLINE"),
        LWPOLYLINE_OBJECT_TYPE => Some("LWPOLYLINE"),
        _ => None,
    }
}

fn collect_symbol_name_maps(bytes: &[u8], pending: &pending::PendingDocument) -> SymbolNameMaps {
    let cursor = object_stream::ObjectStreamCursor::new(bytes, &pending.handle_offsets);
    let mut maps = SymbolNameMaps::default();

    for entry in pending.handle_offsets.iter() {
        let Some(slice) = cursor.object_slice_by_handle(entry.handle) else {
            continue;
        };
        let Ok((header, mut main_reader, mut handle_reader)) =
            object_header::split_ac1015_object_streams(slice)
        else {
            continue;
        };
        if header.handle != entry.handle {
            continue;
        }
        match header.object_type {
            51 => {
                if let Ok(name) =
                    read_layer_table_name(&mut main_reader, &mut handle_reader, header.handle)
                {
                    maps.layer_by_handle.insert(header.handle, name);
                }
            }
            53 => {
                if let Ok(name) =
                    read_text_style_name(&mut main_reader, &mut handle_reader, header.handle)
                {
                    maps.style_by_handle.insert(header.handle, name);
                }
            }
            49 => {
                if let Ok(name) =
                    read_block_header_name(&mut main_reader, &mut handle_reader, header.handle)
                {
                    maps.block_by_handle.insert(header.handle, name);
                }
            }
            57 => {
                if let Ok(name) =
                    read_linetype_name(&mut main_reader, &mut handle_reader, header.handle)
                {
                    maps.linetype_by_handle.insert(header.handle, name);
                }
            }
            _ => {}
        }
    }

    maps
}

fn read_pre_r2007_xref_dependent_bits(reader: &mut BitReader<'_>) -> Result<bool, DwgReadError> {
    let _xref_64 = reader.read_bit()?;
    let _xref_index = reader.read_bit_short()?;
    Ok(reader.read_bit()? == 1)
}

fn read_layer_table_name(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<String, DwgReadError> {
    let _common =
        entity_common::parse_ac1015_non_entity_common(main_reader, handle_reader, object_handle)?;
    let name = main_reader.read_text_ascii()?;
    let _xref_dependent = read_pre_r2007_xref_dependent_bits(main_reader)?;
    let _values = main_reader.read_bit_short()?;
    let _color = main_reader.read_bit_short()?;
    Ok(name)
}

fn read_text_style_name(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<String, DwgReadError> {
    let _common =
        entity_common::parse_ac1015_non_entity_common(main_reader, handle_reader, object_handle)?;
    Ok(main_reader.read_text_ascii()?)
}

fn read_block_header_name(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<String, DwgReadError> {
    let _common =
        entity_common::parse_ac1015_non_entity_common(main_reader, handle_reader, object_handle)?;
    Ok(main_reader.read_text_ascii()?)
}

fn read_linetype_name(
    main_reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    object_handle: Handle,
) -> Result<String, DwgReadError> {
    let _common =
        entity_common::parse_ac1015_non_entity_common(main_reader, handle_reader, object_handle)?;
    Ok(main_reader.read_text_ascii()?)
}

fn resolve_layer_name(handle: Handle, symbol_names: &SymbolNameMaps) -> String {
    if handle == Handle::NULL {
        "0".to_string()
    } else {
        symbol_names
            .layer_by_handle
            .get(&handle)
            .cloned()
            .unwrap_or_else(|| format!("$LAYER_{:X}", handle.value()))
    }
}

fn resolve_style_name(handle: Handle, symbol_names: &SymbolNameMaps) -> String {
    if handle == Handle::NULL {
        String::new()
    } else {
        symbol_names
            .style_by_handle
            .get(&handle)
            .cloned()
            .unwrap_or_else(|| format!("$STYLE_{:X}", handle.value()))
    }
}

fn resolve_block_name(handle: Handle, symbol_names: &SymbolNameMaps) -> String {
    if handle == Handle::NULL {
        String::new()
    } else {
        symbol_names
            .block_by_handle
            .get(&handle)
            .cloned()
            .unwrap_or_else(|| format!("$BLOCK_{:X}", handle.value()))
    }
}

fn resolve_linetype_name(
    linetype_flags: u8,
    linetype_handle: Handle,
    symbol_names: &SymbolNameMaps,
) -> String {
    match linetype_flags {
        0 => "BYLAYER".to_string(),
        1 => "BYBLOCK".to_string(),
        2 => "CONTINUOUS".to_string(),
        3 => symbol_names
            .linetype_by_handle
            .get(&linetype_handle)
            .cloned()
            .unwrap_or_else(|| format!("$LTYPE_{:X}", linetype_handle.value())),
        _ => String::new(),
    }
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
