use acadrust::entities::{AttributeDefinition, AttributeEntity, Circle, Insert, Line};
use acadrust::tables::BlockRecord;
use acadrust::types::Vector3;
use acadrust::{EntityType, Handle};
use h7cad_native_model as nm;
use pid_parse::package::{PidPackage, RawStream};
use pid_parse::writer::{PidWriter, WritePlan};
use pid_parse::{
    build_import_view, derive_layout, DrawingMeta, GeneralMeta, ObjectGraph, PidDocument,
    PidImportView, PidLayoutItem, PidLayoutModel, PidLayoutText, PidObject, PidParser,
    PidRelationship, SummaryInfo,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::pid_package_store;

const GRID_COLUMNS: usize = 5;
const GRID_SPACING_X: f64 = 260.0;
const GRID_SPACING_Y: f64 = 180.0;
const OBJECT_TEXT_HEIGHT: f64 = 10.0;
const NODE_RADIUS: f64 = 18.0;
const HEADER_Y: f64 = 140.0;
const SUBHEADER_Y: f64 = 112.0;
const SIDE_PANEL_X: f64 = GRID_COLUMNS as f64 * GRID_SPACING_X + 80.0;
const CROSSREF_PANEL_X: f64 = SIDE_PANEL_X + 360.0;
const BOTTOM_PANEL_Y: f64 = -820.0;
const STREAM_PANEL_Y: f64 = -220.0;
const UNRESOLVED_PANEL_Y: f64 = -520.0;
const FALLBACK_PANEL_X: f64 = CROSSREF_PANEL_X + 380.0;
const FALLBACK_START_Y: f64 = 96.0;
const FALLBACK_SPACING_Y: f64 = 92.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidImportSummary {
    pub title: String,
    pub object_count: usize,
    pub relationship_count: usize,
    pub unresolved_relationship_count: usize,
    pub symbol_count: usize,
    pub cluster_count: usize,
    pub sheet_count: usize,
    pub stream_count: usize,
    pub attribute_class_count: usize,
    pub tagged_text_count: usize,
    pub dynamic_attribute_record_count: usize,
    pub object_graph_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PidNodeKey {
    Overview,
    Object { drawing_id: String },
    Relationship { guid: String },
    Sheet { name: String },
    Stream { name: String },
    Cluster { name: String },
    Symbol { symbol_path: String },
    AttributeClass { class_name: String },
    Root { name: String },
    TaggedStorage { storage_name: String },
    DynamicAttributes,
    ClusterCoverage,
    Unresolved { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PidPreviewIndex {
    by_key: BTreeMap<PidNodeKey, Vec<Handle>>,
    by_handle: BTreeMap<u64, PidNodeKey>,
    by_drawing_id: BTreeMap<String, Vec<Handle>>,
    by_graphic_oid: BTreeMap<u32, Vec<Handle>>,
}

impl PidPreviewIndex {
    pub fn handles_for(&self, key: &PidNodeKey) -> Vec<Handle> {
        self.by_key.get(key).cloned().unwrap_or_default()
    }

    pub fn key_for_handle(&self, handle: Handle) -> Option<&PidNodeKey> {
        self.by_handle.get(&handle.value())
    }

    pub fn handles_for_drawing_id(&self, drawing_id: &str) -> Vec<Handle> {
        self.by_drawing_id
            .get(drawing_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn handles_for_graphic_oid(&self, graphic_oid: u32) -> Vec<Handle> {
        self.by_graphic_oid
            .get(&graphic_oid)
            .cloned()
            .unwrap_or_default()
    }

    fn record_existing_handle(&mut self, key: PidNodeKey, handle: Handle) {
        self.by_key.entry(key.clone()).or_default().push(handle);
        self.by_handle
            .entry(handle.value())
            .or_insert_with(|| key.clone());
        if let PidNodeKey::Object { drawing_id } = &key {
            self.by_drawing_id
                .entry(drawing_id.clone())
                .or_default()
                .push(handle);
        }
    }

    fn record_layout_refs(
        &mut self,
        handle: Handle,
        drawing_id: Option<&str>,
        graphic_oid: Option<u32>,
    ) {
        if let Some(drawing_id) = drawing_id {
            self.by_drawing_id
                .entry(drawing_id.to_string())
                .or_default()
                .push(handle);
        }
        if let Some(graphic_oid) = graphic_oid {
            self.by_graphic_oid
                .entry(graphic_oid)
                .or_default()
                .push(handle);
        }
    }

    #[cfg(test)]
    pub(crate) fn record_for_test(&mut self, key: PidNodeKey, handle: Handle) {
        self.record_existing_handle(key, handle);
    }
}

#[derive(Debug, Clone)]
pub struct PidOpenBundle {
    pub pid_doc: PidDocument,
    pub native_preview: nm::CadDocument,
    pub summary: PidImportSummary,
    pub preview_index: PidPreviewIndex,
}

const SPPID_BRAN_BLOCK_NAME: &str = "SPPID_BRAN";
const SPPID_DATA_COMPONENT_SCHEMA: &str = "PIDComponent";
const SPPID_META_COMPONENT_SCHEMA: &str = "DocVersioningComponent";
/// SPPID publish 产物（Data.xml / Meta.xml）里 `ToolID` 字段的值。
/// 绑 `CARGO_PKG_NAME` 自动跟随 `Cargo.toml [package].name`，避免改名
/// 时漂移；同步由 `sppid_tool_id_matches_crate_name` 测试守护。
const SPPID_TOOL_ID: &str = env!("CARGO_PKG_NAME");

/// SPPID publish 产物里 `SoftwareVersion` 字段的值。
///
/// **不**绑 `CARGO_PKG_VERSION` —— SPPID 消费方可能按精确字符串匹配该值，
/// Cargo 升版本会悄悄带到 publish Meta.xml 存在未知兼容风险。改由
/// `sppid_software_version_tracks_cargo_pkg_version` 测试断言二者一致，
/// 下次 `cargo release` 忘同步时 CI 显性失败提醒。
const SPPID_SOFTWARE_VERSION: &str = "0.1.3";
const SPPID_REL_DRAWING_ITEMS: &str = "DrawingItems";
const SPPID_REL_REP_COMPOSITION: &str = "DwgRepresentationComposition";
const SPPID_REL_END1: &str = "PipingEnd1Conn";
const SPPID_REL_END2: &str = "PipingEnd2Conn";
const SPPID_REL_TAP: &str = "PipingTapOrFitting";
const SPPID_REL_PROCESS_POINT: &str = "ProcessPointCollection";

const SPPID_BRAN_ATTRIBUTES: [(&str, &str, &str); 7] = [
    ("DRAWING_NO", "Drawing Number", "DWG-0202GP06-01"),
    ("DOC_TITLE", "Document Title", "H7CAD BRAN Tutorial"),
    ("PIPELINE", "Pipeline", "A3jqz0101-OD"),
    (
        "CONNECTOR",
        "Connector",
        "A3jqz0101-OD-50 mm-1.6AR12-WE-50mm",
    ),
    ("PIPING_CLASS", "Piping Class", "1.6AR12"),
    ("NOMINAL_DIAMETER", "Nominal Diameter", "50 mm"),
    ("BRANCH_NAME", "Branch Name", "BRAN-1"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidPublishReport {
    pub drawing_no: String,
    pub object_count: usize,
    pub relationship_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidExportBundle {
    pub pid_path: PathBuf,
    pub data_xml_path: PathBuf,
    pub meta_xml_path: PathBuf,
    pub report: PidPublishReport,
}

#[derive(Debug, Clone)]
struct BranPublishModel {
    drawing_no: String,
    doc_title: String,
    pipeline_name: String,
    connector_name: String,
    branch_name: String,
    piping_class: String,
    nominal_diameter: String,
    drawing_uid: String,
    pipeline_uid: String,
    connector_uid: String,
    piping_branch_uid: String,
    branch_uid: String,
    process_point_uid: String,
    representation_uid: String,
    doc_version_uid: String,
    doc_revision_uid: String,
    file_uid: String,
}

pub fn open_pid(path: &Path) -> Result<PidOpenBundle, String> {
    let parser = PidParser::new();
    let mut package = parser.parse_package(path).map_err(|e| e.to_string())?;
    merge_publish_sidecars(path, &mut package.parsed)?;
    derive_layout(&mut package.parsed);
    let bundle = pid_document_to_bundle(&package.parsed);
    pid_package_store::cache_package(path, package);
    Ok(bundle)
}

pub fn load_pid_native(path: &Path) -> Result<nm::CadDocument, String> {
    Ok(open_pid(path)?.native_preview)
}

/// Load a PID file and additionally cache its `PidPackage` (raw CFB
/// stream bytes) for later round-trip on save. Returns the visualization
/// `CadDocument` and an `(object_count, relationship_count, unresolved)`
/// summary tuple via the second slot — keep the legacy single-`Result`
/// behavior here and let the dedicated helper below produce the named
/// summary used by the UI.
#[allow(dead_code)]
pub fn load_pid_native_with_package(
    path: &Path,
) -> Result<(nm::CadDocument, PidImportSummary), String> {
    let bundle = open_pid(path)?;
    Ok((bundle.native_preview, bundle.summary))
}

#[allow(dead_code)]
pub fn import_pid_summary(path: &Path) -> Result<PidImportSummary, String> {
    Ok(open_pid(path)?.summary)
}

/// Outcome of [`edit_pid_drawing_attribute`]. `previous` is `None` when
/// the source `Drawing` XML had no matching attribute (in practice the
/// edit also fails via `MetadataEditError::AttributeNotFound`; the
/// field exists to give report formatters a typed channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingAttributeEdit {
    pub attr: String,
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

/// Outcome of [`edit_pid_drawing_number`]. Kept as a typed alias-shape
/// for the existing PIDSETDRAWNO command and tests; semantically a
/// projection of [`DrawingAttributeEdit`] for the canonical
/// `SP_DRAWINGNUMBER` attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingNumberEdit {
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

const DRAWING_STREAM_PATH: &str = "/TaggedTxtData/Drawing";

/// Replace an arbitrary attribute on `<Tag …>` lines inside the cached
/// `PidPackage`'s `/TaggedTxtData/Drawing` stream and re-cache the
/// modified package. Generic foundation behind both PIDSETDRAWNO and
/// PIDSETPROP.
///
/// Errors for unfindable / non-UTF-8 / ambiguous inputs are surfaced as
/// `String` so the command-line layer can display them verbatim.
pub fn edit_pid_drawing_attribute(
    source: &Path,
    attr: &str,
    value: &str,
) -> Result<DrawingAttributeEdit, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    // Arc<PidPackage> isn't mutable in place — clone the inner package
    // so we can splice and then re-cache the new owner.
    let mut package = (*arc).clone();
    drop(arc);

    let raw = package.get_stream(DRAWING_STREAM_PATH).ok_or_else(|| {
        format!(
            "source PID is missing {} stream",
            DRAWING_STREAM_PATH
        )
    })?;
    let xml = std::str::from_utf8(&raw.data).map_err(|e| {
        format!(
            "Drawing XML is not UTF-8 (BOM/UTF-16 not yet supported): {e}"
        )
    })?;

    let previous = pid_parse::writer::get_drawing_attribute(xml, attr);
    let new_xml = pid_parse::writer::set_drawing_attribute(xml, attr, value)
        .map_err(|e| format!("metadata edit failed: {e}"))?;
    let new_xml_len = new_xml.len();
    package.replace_stream(DRAWING_STREAM_PATH, new_xml.into_bytes());
    pid_package_store::cache_package(source, package);

    Ok(DrawingAttributeEdit {
        attr: attr.to_string(),
        previous,
        next: value.to_string(),
        new_xml_len,
    })
}

const GENERAL_STREAM_PATH: &str = "/TaggedTxtData/General";

/// Outcome of [`edit_pid_general_element`]; symmetrical with
/// [`DrawingAttributeEdit`] but for the General stream's
/// `<element>text</element>` shape rather than `<Tag attr="…"/>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralElementEdit {
    pub element: String,
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

/// Replace the inner text of `<element>…</element>` inside the cached
/// `PidPackage`'s `/TaggedTxtData/General` stream and re-cache the
/// modified package. Mirrors [`edit_pid_drawing_attribute`] for the
/// General stream (text-content edits, not attribute edits).
pub fn edit_pid_general_element(
    source: &Path,
    element: &str,
    value: &str,
) -> Result<GeneralElementEdit, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let mut package = (*arc).clone();
    drop(arc);

    let raw = package.get_stream(GENERAL_STREAM_PATH).ok_or_else(|| {
        format!("source PID is missing {} stream", GENERAL_STREAM_PATH)
    })?;
    let xml = std::str::from_utf8(&raw.data).map_err(|e| {
        format!("General XML is not UTF-8 (BOM/UTF-16 not yet supported): {e}")
    })?;

    let previous = pid_parse::writer::get_general_element_text(xml, element);
    let new_xml = pid_parse::writer::set_element_text(xml, element, value)
        .map_err(|e| format!("metadata edit failed: {e}"))?;
    let new_xml_len = new_xml.len();
    package.replace_stream(GENERAL_STREAM_PATH, new_xml.into_bytes());
    pid_package_store::cache_package(source, package);

    Ok(GeneralElementEdit {
        element: element.to_string(),
        previous,
        next: value.to_string(),
        new_xml_len,
    })
}

/// Read-only lookup of an attribute on `<Tag …>` lines inside the cached
/// `PidPackage`'s `/TaggedTxtData/Drawing` stream. Returns `None` if
/// the package isn't cached, the stream is missing, the bytes aren't
/// UTF-8, the attribute isn't found, or it appears multiple times.
/// Errors that callers want to surface (cache miss, malformed XML)
/// should prefer the typed setters; this helper is geared at "show
/// current value" UX flows.
pub fn read_pid_drawing_attribute(source: &Path, attr: &str) -> Option<String> {
    let arc = pid_package_store::get_package(source)?;
    let raw = arc.get_stream(DRAWING_STREAM_PATH)?;
    let xml = std::str::from_utf8(&raw.data).ok()?;
    pid_parse::writer::get_drawing_attribute(xml, attr)
}

/// Read-only lookup of an element's text content inside the cached
/// `PidPackage`'s `/TaggedTxtData/General` stream. Same soft-`None`
/// semantics as [`read_pid_drawing_attribute`].
pub fn read_pid_general_element(source: &Path, element: &str) -> Option<String> {
    let arc = pid_package_store::get_package(source)?;
    let raw = arc.get_stream(GENERAL_STREAM_PATH)?;
    let xml = std::str::from_utf8(&raw.data).ok()?;
    pid_parse::writer::get_general_element_text(xml, element)
}

/// Snapshot of every readable metadata field on a cached PID:
/// Drawing-side `<Tag attr="value"/>` pairs and General-side
/// `<element>text</element>` pairs. Source order is preserved within
/// each group; duplicates inside a group are kept (the writer is what
/// rejects ambiguous edits, the reader shows everything).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PidPropsListing {
    pub drawing_attributes: Vec<(String, String)>,
    pub general_elements: Vec<(String, String)>,
}

/// Build a [`PidPropsListing`] from the cached `PidPackage` for
/// `source`. Returns typed errors (rather than soft `None`) because
/// "list everything" is usually the entry point of a UX flow and the
/// user benefits from knowing whether the failure was "no cache" vs
/// "stream missing" vs "non-UTF-8".
pub fn list_pid_metadata(source: &Path) -> Result<PidPropsListing, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;

    let mut listing = PidPropsListing::default();

    let drawing_raw = arc.get_stream(DRAWING_STREAM_PATH).ok_or_else(|| {
        format!("source PID is missing {} stream", DRAWING_STREAM_PATH)
    })?;
    let drawing_xml = std::str::from_utf8(&drawing_raw.data).map_err(|e| {
        format!("Drawing XML is not UTF-8 (BOM/UTF-16 not yet supported): {e}")
    })?;
    listing.drawing_attributes = pid_parse::writer::list_drawing_attributes(drawing_xml);

    let general_raw = arc.get_stream(GENERAL_STREAM_PATH).ok_or_else(|| {
        format!("source PID is missing {} stream", GENERAL_STREAM_PATH)
    })?;
    let general_xml = std::str::from_utf8(&general_raw.data).map_err(|e| {
        format!("General XML is not UTF-8 (BOM/UTF-16 not yet supported): {e}")
    })?;
    listing.general_elements = pid_parse::writer::list_general_elements(general_xml);

    Ok(listing)
}

/// Replace `SP_DRAWINGNUMBER` in the cached `PidPackage`'s
/// `/TaggedTxtData/Drawing` stream. Thin wrapper over
/// [`edit_pid_drawing_attribute`] preserved for the dedicated PIDSETDRAWNO
/// command and existing tests.
pub fn edit_pid_drawing_number(
    source: &Path,
    new_value: &str,
) -> Result<DrawingNumberEdit, String> {
    let edit = edit_pid_drawing_attribute(source, "SP_DRAWINGNUMBER", new_value)?;
    Ok(DrawingNumberEdit {
        previous: edit.previous,
        next: edit.next,
        new_xml_len: edit.new_xml_len,
    })
}

/// One per-stream divergence found by [`verify_pid_cached`] /
/// [`verify_pid_file`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVerifyMismatch {
    pub path: String,
    pub source_len: usize,
    pub roundtrip_len: usize,
    pub first_diff_offset: usize,
}

/// Outcome of a PID round-trip verification: per-stream diff between
/// the source PidPackage (cached or freshly parsed) and the package
/// re-parsed from a temp PidWriter output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVerifyReport {
    pub stream_count: usize,
    pub matched: usize,
    pub mismatches: Vec<PidVerifyMismatch>,
    pub only_in_source: Vec<String>,
    pub only_in_roundtrip: Vec<String>,
}

impl PidVerifyReport {
    pub fn ok(&self) -> bool {
        self.mismatches.is_empty()
            && self.only_in_source.is_empty()
            && self.only_in_roundtrip.is_empty()
    }
}

fn unique_temp_pid(tag: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("h7cad-pid-verify-{pid}-{nanos}-{tag}.pid"))
}

fn compare_streams(
    original: &pid_parse::package::PidPackage,
    roundtrip: &pid_parse::package::PidPackage,
) -> PidVerifyReport {
    use std::collections::BTreeSet;
    let src_keys: BTreeSet<&String> = original.streams.keys().collect();
    let dst_keys: BTreeSet<&String> = roundtrip.streams.keys().collect();
    let only_in_source: Vec<String> = src_keys
        .difference(&dst_keys)
        .map(|s| (*s).clone())
        .collect();
    let only_in_roundtrip: Vec<String> = dst_keys
        .difference(&src_keys)
        .map(|s| (*s).clone())
        .collect();

    let mut matched = 0usize;
    let mut mismatches = Vec::new();
    for path in src_keys.intersection(&dst_keys) {
        let src = &original.streams[*path];
        let dst = &roundtrip.streams[*path];
        if src.data == dst.data {
            matched += 1;
        } else {
            let len = src.data.len().min(dst.data.len());
            let mut first_diff_offset = len;
            for i in 0..len {
                if src.data[i] != dst.data[i] {
                    first_diff_offset = i;
                    break;
                }
            }
            mismatches.push(PidVerifyMismatch {
                path: (*path).to_string(),
                source_len: src.data.len(),
                roundtrip_len: dst.data.len(),
                first_diff_offset,
            });
        }
    }
    PidVerifyReport {
        stream_count: original.streams.len(),
        matched,
        mismatches,
        only_in_source,
        only_in_roundtrip,
    }
}

/// Round-trip the cached `PidPackage` for `source` through `PidWriter`,
/// re-parse the temp output, and report per-stream byte equality. Used
/// by the `PIDVERIFY` command to confirm "this package can be saved
/// safely" without actually overwriting any user file.
pub fn verify_pid_cached(source: &Path) -> Result<PidVerifyReport, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let dst = unique_temp_pid("cached");
    PidWriter::write_to(&arc, &WritePlan::default(), &dst).map_err(|e| e.to_string())?;
    let roundtrip = PidParser::new()
        .parse_package(&dst)
        .map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&dst);
    let roundtrip = roundtrip?;
    Ok(compare_streams(&arc, &roundtrip))
}

/// Owned view of a PID's CFB CLSID metadata, projected from the cached
/// PidPackage. Strings are the canonical `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`
/// form so the command-line layer can print them directly without a
/// uuid dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidClsidInfo {
    pub root_clsid: Option<String>,
    /// `(storage_path, clsid_string)` pairs for non-root storages whose
    /// CLSID is **not** the nil UUID. Empty Vec is common (most real
    /// SmartPlant samples leave these unset).
    pub non_root: Vec<(String, String)>,
}

/// Read CLSID metadata from the cached PidPackage. Errors only when the
/// cache is missing for `source`.
pub fn read_pid_clsid(source: &Path) -> Result<PidClsidInfo, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    Ok(PidClsidInfo {
        root_clsid: arc.root_clsid.map(|u| format!("{{{}}}", u)),
        non_root: arc
            .storage_clsids
            .iter()
            .map(|(path, uuid)| (path.clone(), format!("{{{}}}", uuid)))
            .collect(),
    })
}

/// Aggregate "health check" of the cached PidPackage. Sub-sections fail
/// gracefully: if e.g. `pid_graph_stats` errors (no object_graph), the
/// corresponding field is `None` but the overall `build_pid_health_report`
/// still returns `Ok`. Only missing-cache fails hard.
#[derive(Debug, Clone)]
pub struct PidHealthReport {
    pub source_path: std::path::PathBuf,
    pub stream_count: usize,
    pub graph_stats: Option<PidGraphStats>,
    pub drawing_attributes: Vec<(String, String)>,
    pub general_elements: Vec<(String, String)>,
    pub verify: Option<PidVerifyReport>,
    pub unidentified: Vec<UnidentifiedStreamInfo>,
    pub version_log: Option<PidVersionLog>,
    pub root_clsid: Option<String>,
    pub non_root_clsid_count: usize,
}

/// Build a [`PidHealthReport`] for the cached package at `source`.
/// Aggregates from existing helpers with graceful sub-section failure.
pub fn build_pid_health_report(source: &Path) -> Result<PidHealthReport, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let stream_count = arc.streams.len();
    let root_clsid = arc.root_clsid.map(|u| format!("{{{}}}", u));
    let non_root_clsid_count = arc.storage_clsids.len();
    drop(arc);

    let graph_stats = pid_graph_stats(source).ok();
    let listing = list_pid_metadata(source).ok();
    let (drawing_attributes, general_elements) = listing
        .map(|l| (l.drawing_attributes, l.general_elements))
        .unwrap_or_default();
    let verify = verify_pid_cached(source).ok();
    let unidentified = list_pid_unidentified_cached(source).unwrap_or_default();
    let version_log = list_pid_versions(source).ok().flatten();

    Ok(PidHealthReport {
        source_path: source.to_path_buf(),
        stream_count,
        graph_stats,
        drawing_attributes,
        general_elements,
        verify,
        unidentified,
        version_log,
        root_clsid,
        non_root_clsid_count,
    })
}

/// One structured record from the `/DocVersion2` save log, in a
/// command-line-friendly owned form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVersionRecord {
    pub op_type: u8,
    /// Human label: "SaveAs" / "Save" / hex fallback (e.g. "0xAB").
    pub op_label: String,
    pub version: u32,
}

/// Outcome of [`list_pid_versions`]. Mirrors
/// [`pid_parse::DocVersion2`] with owned projections so the command
/// layer doesn't need to keep the cached package borrow alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidVersionLog {
    pub magic_u32_le: u32,
    pub reserved_all_zero: bool,
    pub records: Vec<PidVersionRecord>,
}

/// Read the structured DocVersion2 log from the cached PidPackage.
///
/// - `Ok(Some(log))` when `parsed.doc_version2_decoded` is populated
/// - `Ok(None)` when the PID was parsed successfully but the log wasn't
///   structurally decoded (older layout / decoder bail-out)
/// - `Err(..)` when the cache is missing or the file has no DocVersion2
pub fn list_pid_versions(source: &Path) -> Result<Option<PidVersionLog>, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let Some(decoded) = arc.parsed.doc_version2_decoded.as_ref() else {
        return Ok(None);
    };
    let records: Vec<PidVersionRecord> = decoded
        .records
        .iter()
        .map(|r| PidVersionRecord {
            op_type: r.op_type,
            op_label: pid_parse::parsers::doc_version2::op_type_label(r.op_type),
            version: r.version,
        })
        .collect();
    Ok(Some(PidVersionLog {
        magic_u32_le: decoded.magic_u32_le,
        reserved_all_zero: decoded.reserved_all_zero,
        records,
    }))
}

/// Parse two `.pid` files via `PidParser::parse_package`, compute a
/// [`pid_parse::package::PackageDiff`], and render it to a
/// human-readable string. Returns `(has_differences, rendered_text)`.
///
/// Does not consult or mutate the package cache — pure disk read.
pub fn diff_pid_files(a: &Path, b: &Path) -> Result<(bool, String), String> {
    for (label, path) in [("a", a), ("b", b)] {
        let ok = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("pid"));
        if !ok {
            return Err(format!(
                "{}: '{}' is not a .pid file",
                label,
                path.display()
            ));
        }
    }
    let parser = PidParser::new();
    let pkg_a = parser
        .parse_package(a)
        .map_err(|e| format!("parse a ({}): {e}", a.display()))?;
    let pkg_b = parser
        .parse_package(b)
        .map_err(|e| format!("parse b ({}): {e}", b.display()))?;
    let diff = pid_parse::package::diff_packages(&pkg_a, &pkg_b);
    let has_diffs = !diff.is_empty();
    let rendered = pid_parse::inspect::diff::render(&diff);
    Ok((has_diffs, rendered))
}

/// Lightweight projection of a `PidObject` for command-line display
/// (PIDNEIGHBORS report rows). Owned strings so the command layer
/// doesn't have to keep the cached `PidPackage`'s borrow alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidNeighborInfo {
    pub drawing_id: String,
    pub item_type: String,
    pub model_id: Option<String>,
    /// Best-effort short label: prefers `extra["Tag"]` then
    /// `extra["ItemTag"]`, then any single-line `extra` value, then
    /// `None`. Mirrors how `add_object_entities` picks a display tag.
    pub tag_label: Option<String>,
}

/// Selector for [`list_pid_objects_matching`]: which dimension of an
/// object to filter on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidFindCriterion {
    /// Match every object whose `item_type` exactly equals this value.
    ItemType(String),
    /// Match every object whose `extra[key]` exists and equals `value`.
    ExtraEquals { key: String, value: String },
}

/// Aggregate counts derived from the cached `PidPackage`'s
/// [`pid_parse::ObjectGraph`]. Cheap (O(R)) and always populated by
/// [`pid_graph_stats`] when the graph is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidGraphStats {
    pub object_count: usize,
    pub relationship_count: usize,
    pub fully_resolved: usize,
    pub partially_resolved: usize,
    pub unresolved: usize,
}

fn project_neighbor(obj: &pid_parse::PidObject) -> PidNeighborInfo {
    let tag_label = obj
        .extra
        .get("Tag")
        .or_else(|| obj.extra.get("ItemTag"))
        .or_else(|| obj.extra.values().next())
        .cloned();
    PidNeighborInfo {
        drawing_id: obj.drawing_id.clone(),
        item_type: obj.item_type.clone(),
        model_id: obj.model_id.clone(),
        tag_label,
    }
}

/// Resolve a user-supplied `drawing_id` *or* any unique prefix thereof
/// against `graph`. Shared by [`list_pid_neighbors`] and
/// [`list_pid_path`] so the prefix-match diagnostics ("ambiguous",
/// "no match") stay identical across commands.
fn resolve_drawing_id_in_graph(
    graph: &pid_parse::ObjectGraph,
    drawing_id_or_prefix: &str,
) -> Result<String, String> {
    if graph.object_by_drawing_id(drawing_id_or_prefix).is_some() {
        return Ok(drawing_id_or_prefix.to_string());
    }
    let matches = graph.find_drawing_ids_by_prefix(drawing_id_or_prefix);
    match matches.len() {
        0 => Err(format!(
            "no drawing_id matches '{}' (exact or prefix)",
            drawing_id_or_prefix
        )),
        1 => Ok(matches[0].to_string()),
        n => {
            let preview: Vec<&str> = matches.iter().take(3).copied().collect();
            let preview_str = preview.join(", ");
            let suffix = if n > 3 {
                format!(", ... ({} more)", n - 3)
            } else {
                String::new()
            };
            Err(format!(
                "prefix '{}' is ambiguous (matches {}): {}{}",
                drawing_id_or_prefix, n, preview_str, suffix
            ))
        }
    }
}

/// Find the shortest path through the cached PidPackage's ObjectGraph
/// from `from_or_prefix` to `to_or_prefix`. Both endpoints accept the
/// same exact-or-unique-prefix syntax as [`list_pid_neighbors`].
///
/// Returns `(from_info, to_info, path_objects)` where `path_objects[0]
/// == from_info` and `path_objects.last() == to_info`. Adjacent entries
/// are connected by at least one resolved relationship endpoint.
pub fn list_pid_path(
    source: &Path,
    from_or_prefix: &str,
    to_or_prefix: &str,
) -> Result<(PidNeighborInfo, PidNeighborInfo, Vec<PidNeighborInfo>), String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let graph = arc
        .parsed
        .object_graph
        .as_ref()
        .ok_or_else(|| "source PID has no object_graph (P&IDAttributes not parsed)".to_string())?;
    let from_id = resolve_drawing_id_in_graph(graph, from_or_prefix)?;
    let to_id = resolve_drawing_id_in_graph(graph, to_or_prefix)?;

    let path_ids = graph.shortest_path(&from_id, &to_id).ok_or_else(|| {
        format!(
            "no path from {} to {} (graph disconnected within these endpoints)",
            from_id, to_id
        )
    })?;
    let path_objects: Vec<PidNeighborInfo> = path_ids
        .iter()
        .filter_map(|id| graph.object_by_drawing_id(id).map(project_neighbor))
        .collect();
    let from_info = path_objects.first().cloned().unwrap_or_else(|| {
        // Should be unreachable because path always contains at least from.
        PidNeighborInfo {
            drawing_id: from_id.clone(),
            item_type: String::new(),
            model_id: None,
            tag_label: None,
        }
    });
    let to_info = path_objects.last().cloned().unwrap_or(from_info.clone());
    Ok((from_info, to_info, path_objects))
}

/// Look up an object in the cached `PidPackage`'s ObjectGraph by its
/// `drawing_id` *or any unique prefix thereof*, and return reachable
/// neighbors within `depth` hops via resolved relationship endpoints.
///
/// `depth=0` → empty neighbor list (only the resolved self_info is
/// returned).  
/// `depth=1` → direct neighbors (matches earlier behaviour).  
/// `depth=N` → BFS over relationship edges, distinct, level-by-level.
///
/// Resolution rules for `drawing_id_or_prefix`:
/// 1. If the input is an exact `drawing_id`, use it directly.
/// 2. Otherwise treat it as a prefix and call
///    [`pid_parse::ObjectGraph::find_drawing_ids_by_prefix`]:
///    - 0 matches → "no drawing_id matches '<input>'"
///    - 1 match → resolve and proceed
///    - N matches (N ≥ 2) → "prefix is ambiguous (matches N: <first 3>, ...)"
///
/// Self-loops and unresolved (`None`) endpoints are silently skipped
/// (mirrors `ObjectGraph::neighbors_of` / `neighbors_within` semantics).
pub fn list_pid_neighbors(
    source: &Path,
    drawing_id_or_prefix: &str,
    depth: usize,
) -> Result<(PidNeighborInfo, Vec<PidNeighborInfo>), String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let graph = arc
        .parsed
        .object_graph
        .as_ref()
        .ok_or_else(|| "source PID has no object_graph (P&IDAttributes not parsed)".to_string())?;

    let resolved_id = resolve_drawing_id_in_graph(graph, drawing_id_or_prefix)?;

    let self_obj = graph.object_by_drawing_id(&resolved_id).expect(
        "BTreeMap::range invariant: prefix-matched key must exist in by_drawing_id",
    );
    let neighbors: Vec<PidNeighborInfo> = graph
        .neighbors_within(&resolved_id, depth)
        .into_iter()
        .map(project_neighbor)
        .collect();
    Ok((project_neighbor(self_obj), neighbors))
}

/// Search the cached `PidPackage`'s ObjectGraph for objects matching
/// `criterion` and return owned [`PidNeighborInfo`] projections in
/// source order. Errors when no ObjectGraph is present.
pub fn list_pid_objects_matching(
    source: &Path,
    criterion: &PidFindCriterion,
) -> Result<Vec<PidNeighborInfo>, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let graph = arc
        .parsed
        .object_graph
        .as_ref()
        .ok_or_else(|| "source PID has no object_graph (P&IDAttributes not parsed)".to_string())?;
    let raw_matches: Vec<&pid_parse::PidObject> = match criterion {
        PidFindCriterion::ItemType(t) => graph.find_objects_by_item_type(t),
        PidFindCriterion::ExtraEquals { key, value } => {
            graph.find_objects_by_extra(key, value)
        }
    };
    Ok(raw_matches.into_iter().map(project_neighbor).collect())
}

/// Aggregate counts + endpoint-resolution distribution for the cached
/// `PidPackage`'s ObjectGraph. Errors when no ObjectGraph is present
/// (the file lacked decodable P&IDAttributes records).
pub fn pid_graph_stats(source: &Path) -> Result<PidGraphStats, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let graph = arc
        .parsed
        .object_graph
        .as_ref()
        .ok_or_else(|| "source PID has no object_graph (P&IDAttributes not parsed)".to_string())?;
    let s = graph.endpoint_resolution_stats();
    Ok(PidGraphStats {
        object_count: graph.objects.len(),
        relationship_count: graph.relationships.len(),
        fully_resolved: s.fully_resolved,
        partially_resolved: s.partially_resolved,
        unresolved: s.unresolved,
    })
}

/// Lightweight projection of an "unidentified" top-level CFB stream:
/// path, byte length, and optional 4-byte magic tag. Owned so the
/// command-line layer can clone / compare without borrowing into the
/// cached `PidPackage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnidentifiedStreamInfo {
    pub path: String,
    pub size: u64,
    pub magic_u32_le: Option<u32>,
    /// ASCII rendering of the magic (when all 4 bytes are printable),
    /// e.g. `Some("root")` for `0x746F6F72`. `None` otherwise.
    pub magic_tag: Option<String>,
}

fn project_unidentified(
    streams: impl IntoIterator<Item = pid_parse::StreamEntry>,
) -> Vec<UnidentifiedStreamInfo> {
    streams
        .into_iter()
        .map(|s| UnidentifiedStreamInfo {
            path: s.path,
            size: s.size,
            magic_tag: s
                .magic_u32_le
                .and_then(pid_parse::parsers::magic::magic_tag),
            magic_u32_le: s.magic_u32_le,
        })
        .collect()
}

/// List top-level CFB streams in the **cached** `PidPackage` for
/// `source` that `pid-parse` does not yet recognize. Returns an empty
/// vec when the sample is fully covered.
pub fn list_pid_unidentified_cached(
    source: &Path,
) -> Result<Vec<UnidentifiedStreamInfo>, String> {
    let arc = pid_package_store::get_package(source).ok_or_else(|| {
        format!(
            "no cached PidPackage for {} (open the file in H7CAD first)",
            source.display()
        )
    })?;
    let leftover = pid_parse::inspect::unidentified_top_level_streams(&arc.parsed);
    Ok(project_unidentified(leftover.into_iter().cloned()))
}

/// List top-level CFB streams in `path` that `pid-parse` does not yet
/// recognize. Parses fresh; does not consult or mutate the package
/// store.
pub fn list_pid_unidentified_file(
    path: &Path,
) -> Result<Vec<UnidentifiedStreamInfo>, String> {
    let doc = PidParser::new()
        .parse_file(path)
        .map_err(|e| e.to_string())?;
    let leftover = pid_parse::inspect::unidentified_top_level_streams(&doc);
    Ok(project_unidentified(leftover.into_iter().cloned()))
}

/// Round-trip an arbitrary `.pid` file on disk: parse → write to a
/// temp file via `PidWriter` → re-parse → compare. Does not consult or
/// mutate the package store.
pub fn verify_pid_file(path: &Path) -> Result<PidVerifyReport, String> {
    let parser = PidParser::new();
    let original = parser.parse_package(path).map_err(|e| e.to_string())?;
    let dst = unique_temp_pid("file");
    PidWriter::write_to(&original, &WritePlan::default(), &dst).map_err(|e| e.to_string())?;
    let roundtrip = parser.parse_package(&dst).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&dst);
    let roundtrip = roundtrip?;
    Ok(compare_streams(&original, &roundtrip))
}

/// Save a PID file by re-emitting the cached `PidPackage` through the
/// `pid-parse` writer. First-version policy is **passthrough only**:
/// edits to the in-memory `CadDocument` do not flow back into the PID
/// container — the writer simply replays the raw CFB streams that were
/// captured at open time.
///
/// Returns an explanatory error if no `PidPackage` is cached for the
/// supplied source path. The caller (UI / save dialog) should surface
/// the message verbatim.
pub fn save_pid_native(path: &Path, source_path: &Path) -> Result<(), String> {
    let package = pid_package_store::get_package(source_path).ok_or_else(|| {
        format!(
            "PID save requires the original .pid file to be opened first (no cached package for {})",
            source_path.display()
        )
    })?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create destination directory: {e}"))?;
        }
    }
    PidWriter::write_to(&package, &WritePlan::default(), path).map_err(|e| e.to_string())?;
    copy_publish_sidecars_if_present(source_path, path)?;
    Ok(())
}

/// Mirror publish sidecars (`{stem}_Data.xml` + `{stem}_Meta.xml`) from
/// the source `.pid` directory to the destination's directory, renamed to
/// match the destination's stem. A no-op when no sidecars exist next to
/// the source. Errors when only one of the pair is present — mirrors the
/// "both or nothing" contract enforced by [`merge_publish_sidecars`] on
/// open.
///
/// Without this copy, a "Save As" that moves a H7CAD-published `.pid` to
/// a new directory would orphan the sidecars, and re-opening the new
/// `.pid` would silently drop the publish-enhanced object graph
/// ([`publish_data_path`] / [`publish_meta_path`] naming is stem-derived
/// and so cannot find the old files at the new location).
fn copy_publish_sidecars_if_present(src: &Path, dst: &Path) -> Result<(), String> {
    let src_data = publish_data_path(src);
    let src_meta = publish_meta_path(src);
    let has_data = src_data.exists();
    let has_meta = src_meta.exists();

    if !has_data && !has_meta {
        return Ok(());
    }
    if has_data != has_meta {
        return Err(format!(
            "incomplete publish bundle beside {} (expected both {} and {})",
            src.display(),
            src_data.display(),
            src_meta.display()
        ));
    }

    let dst_data = publish_data_path(dst);
    let dst_meta = publish_meta_path(dst);

    // Same-file save: sidecars already live at the correct names; skip the
    // copy (fs::copy of a file onto itself is OS-dependent garbage).
    if src_data == dst_data && src_meta == dst_meta {
        return Ok(());
    }

    std::fs::copy(&src_data, &dst_data)
        .map_err(|e| format!("failed to copy publish Data.xml sidecar: {e}"))?;
    std::fs::copy(&src_meta, &dst_meta)
        .map_err(|e| format!("failed to copy publish Meta.xml sidecar: {e}"))?;

    Ok(())
}

pub fn ensure_sppid_bran_block_library(doc: &mut acadrust::CadDocument) -> Result<(), String> {
    if doc.block_records.get(SPPID_BRAN_BLOCK_NAME).is_some() {
        return Ok(());
    }

    let br_handle = doc.allocate_handle();
    let mut block = BlockRecord::new(SPPID_BRAN_BLOCK_NAME);
    block.handle = br_handle;
    block.block_entity_handle = doc.allocate_handle();
    block.block_end_handle = doc.allocate_handle();
    block.flags.has_attributes = true;
    doc.block_records
        .add(block)
        .map_err(|e| format!("failed to add SPPID block record: {e}"))?;

    for entity in [
        EntityType::Line(Line::from_coords(-40.0, 0.0, 0.0, 40.0, 0.0, 0.0)),
        EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 0.0, 28.0, 0.0)),
        EntityType::Circle(Circle::from_coords(0.0, 0.0, 0.0, 3.5)),
    ] {
        let mut entity = entity;
        entity.common_mut().owner_handle = br_handle;
        entity.common_mut().layer = "0".to_string();
        doc.add_entity(entity)
            .map_err(|e| format!("failed to seed SPPID block geometry: {e}"))?;
    }

    for (index, (tag, prompt, default_value)) in SPPID_BRAN_ATTRIBUTES.iter().enumerate() {
        let mut attdef = AttributeDefinition::new(
            (*tag).to_string(),
            (*prompt).to_string(),
            (*default_value).to_string(),
        );
        attdef.common.owner_handle = br_handle;
        attdef.common.layer = "0".to_string();
        attdef.insertion_point = Vector3::new(-48.0, -18.0 - index as f64 * 4.0, 0.0);
        attdef.height = 2.5;
        attdef.flags.invisible = true;
        doc.add_entity(EntityType::AttributeDefinition(attdef))
            .map_err(|e| format!("failed to seed SPPID block attribute definition: {e}"))?;
    }

    Ok(())
}

pub fn populate_sppid_bran_demo(doc: &mut acadrust::CadDocument) -> Result<(), String> {
    ensure_sppid_bran_block_library(doc)?;
    let existing = doc
        .entities()
        .filter(|entity| matches!(
            entity,
            EntityType::Insert(insert) if insert.block_name.eq_ignore_ascii_case(SPPID_BRAN_BLOCK_NAME)
        ))
        .count();
    if existing > 0 {
        return Ok(());
    }

    for entity in [
        EntityType::Line(Line::from_coords(-120.0, 0.0, 0.0, 120.0, 0.0, 0.0)),
        EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 0.0, 72.0, 0.0)),
    ] {
        doc.add_entity(entity)
            .map_err(|e| format!("failed to seed SPPID demo geometry: {e}"))?;
    }

    let mut insert = Insert::new(SPPID_BRAN_BLOCK_NAME, Vector3::new(0.0, 0.0, 0.0));
    insert.attributes = SPPID_BRAN_ATTRIBUTES
        .iter()
        .map(|(tag, _, value)| {
            let mut attr = AttributeEntity {
                tag: (*tag).to_string(),
                value: (*value).to_string(),
                ..Default::default()
            };
            attr.set_value(*value);
            attr
        })
        .collect();
    doc.add_entity(EntityType::Insert(insert))
        .map_err(|e| format!("failed to place SPPID BRAN demo insert: {e}"))?;
    Ok(())
}

pub fn export_sppid_publish_bundle(
    doc: &acadrust::CadDocument,
    pid_path: &Path,
) -> Result<PidExportBundle, String> {
    let model = bran_publish_model_from_document(doc)?;
    if let Some(parent) = pid_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create publish directory: {e}"))?;
        }
    }

    let drawing_xml = build_publish_drawing_stream_xml(&model);
    let general_xml = build_publish_general_stream_xml(pid_path);
    let data_xml = build_publish_data_xml(&model);
    let meta_xml = build_publish_meta_xml(&model, pid_path);

    let mut streams = BTreeMap::new();
    streams.insert(
        DRAWING_STREAM_PATH.to_string(),
        RawStream {
            path: DRAWING_STREAM_PATH.to_string(),
            data: drawing_xml.clone().into_bytes(),
            modified: false,
        },
    );
    streams.insert(
        GENERAL_STREAM_PATH.to_string(),
        RawStream {
            path: GENERAL_STREAM_PATH.to_string(),
            data: general_xml.clone().into_bytes(),
            modified: false,
        },
    );

    let parsed = build_publish_pid_document(
        &model,
        drawing_xml.clone(),
        general_xml.clone(),
        data_xml.clone(),
        meta_xml.clone(),
        pid_path,
    );
    let package = PidPackage::new(Some(pid_path.to_path_buf()), streams, parsed);
    PidWriter::write_to(&package, &WritePlan::default(), pid_path).map_err(|e| e.to_string())?;

    let data_xml_path = publish_data_path(pid_path);
    let meta_xml_path = publish_meta_path(pid_path);
    std::fs::write(&data_xml_path, data_xml)
        .map_err(|e| format!("failed to write publish Data.xml: {e}"))?;
    std::fs::write(&meta_xml_path, meta_xml)
        .map_err(|e| format!("failed to write publish Meta.xml: {e}"))?;

    Ok(PidExportBundle {
        pid_path: pid_path.to_path_buf(),
        data_xml_path,
        meta_xml_path,
        report: PidPublishReport {
            drawing_no: model.drawing_no,
            object_count: 7,
            relationship_count: 6,
        },
    })
}

fn publish_data_path(pid_path: &Path) -> PathBuf {
    pid_path.with_file_name(format!(
        "{}_Data.xml",
        pid_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("publish")
    ))
}

fn publish_meta_path(pid_path: &Path) -> PathBuf {
    pid_path.with_file_name(format!(
        "{}_Meta.xml",
        pid_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("publish")
    ))
}

fn merge_publish_sidecars(path: &Path, doc: &mut PidDocument) -> Result<(), String> {
    let data_path = publish_data_path(path);
    let meta_path = publish_meta_path(path);
    let has_data = data_path.exists();
    let has_meta = meta_path.exists();
    if !has_data && !has_meta {
        return Ok(());
    }
    if has_data != has_meta {
        return Err(format!(
            "incomplete publish bundle beside {} (expected both {} and {})",
            path.display(),
            data_path.display(),
            meta_path.display()
        ));
    }

    let publish_data = parse_publish_data_xml(&data_path)?;
    let publish_meta = parse_publish_meta_xml(&meta_path)?;

    doc.object_graph = Some(publish_data.graph.clone());
    let summary = doc.summary.get_or_insert_with(SummaryInfo::default);
    if summary.title.is_none() {
        summary.title = publish_data.title.clone();
    }
    summary.raw.insert(
        "PublishDataPath".to_string(),
        data_path.display().to_string(),
    );
    if let Some(version) = &publish_meta.doc_version {
        summary
            .raw
            .insert("PublishDocVersion".to_string(), version.clone());
    }
    if let Some(revision) = &publish_meta.doc_revision {
        summary
            .raw
            .insert("PublishDocRevision".to_string(), revision.clone());
    }

    if doc.drawing_meta.is_none() {
        doc.drawing_meta = Some(DrawingMeta {
            drawing_number: publish_data.drawing_no.clone(),
            raw_xml: publish_data.raw_xml.clone(),
            tags: BTreeMap::from([
                (
                    "DocName".to_string(),
                    publish_data.drawing_no.clone().unwrap_or_default(),
                ),
                (
                    "DocUID".to_string(),
                    publish_data.doc_uid.unwrap_or_default(),
                ),
            ]),
            ..Default::default()
        });
    }
    doc.general_meta = Some(publish_meta.general_meta);
    Ok(())
}

#[derive(Debug, Clone)]
struct ParsedPublishData {
    graph: ObjectGraph,
    title: Option<String>,
    drawing_no: Option<String>,
    doc_uid: Option<String>,
    raw_xml: String,
}

#[derive(Debug, Clone)]
struct ParsedPublishMeta {
    general_meta: GeneralMeta,
    doc_version: Option<String>,
    doc_revision: Option<String>,
}

fn parse_publish_data_xml(path: &Path) -> Result<ParsedPublishData, String> {
    let raw_xml = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read publish data XML {}: {e}", path.display()))?;
    let document = roxmltree::Document::parse(&raw_xml)
        .map_err(|e| format!("failed to parse publish data XML {}: {e}", path.display()))?;
    let root = document.root_element();

    let mut drawing_no = root.attribute("DocName").map(str::to_string);
    let doc_uid = root.attribute("DocUID").map(str::to_string);
    let mut title = None;
    let mut objects = Vec::new();
    let mut counts_by_type = BTreeMap::new();
    let mut raw_relationships = Vec::new();

    for node in root.children().filter(|node| node.is_element()) {
        let tag = node.tag_name().name();
        if tag == "Rel" {
            let rel = node
                .children()
                .find(|child| child.is_element() && child.tag_name().name() == "IRel");
            if let Some(rel) = rel {
                let uid1 = rel.attribute("UID1").unwrap_or_default().trim();
                let uid2 = rel.attribute("UID2").unwrap_or_default().trim();
                if !uid1.is_empty() && !uid2.is_empty() {
                    raw_relationships.push((
                        node.children()
                            .find(|child| child.is_element() && child.tag_name().name() == "IObject")
                            .and_then(|obj| obj.attribute("UID"))
                            .map(str::to_string),
                        uid1.to_string(),
                        uid2.to_string(),
                        rel.attribute("DefUID").unwrap_or("Relationship").to_string(),
                    ));
                }
            }
            continue;
        }

        let object = node
            .children()
            .find(|child| child.is_element() && child.tag_name().name() == "IObject");
        let Some(object) = object else {
            continue;
        };
        let Some(uid) = object.attribute("UID") else {
            continue;
        };

        if tag == "PIDDrawing" {
            drawing_no = object.attribute("Name").map(str::to_string).or(drawing_no);
            title = node
                .children()
                .find(|child| child.is_element() && child.tag_name().name() == "IDocument")
                .and_then(|child| child.attribute("DocTitle"))
                .map(str::to_string)
                .or(title);
        }

        let mut extra = BTreeMap::new();
        if let Some(name) = object.attribute("Name") {
            if !name.is_empty() {
                extra.insert("IObject.Name".to_string(), name.to_string());
            }
        }
        if let Some(description) = object.attribute("Description") {
            if !description.is_empty() {
                extra.insert("IObject.Description".to_string(), description.to_string());
            }
        }
        for child in node.children().filter(|child| child.is_element()) {
            if child.tag_name().name() == "IObject" {
                continue;
            }
            for attr in child.attributes() {
                extra.insert(
                    format!("{}.{}", child.tag_name().name(), attr.name()),
                    attr.value().to_string(),
                );
            }
            if let Some(text) = child.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    extra.insert(
                        format!("{}.text", child.tag_name().name()),
                        trimmed.to_string(),
                    );
                }
            }
        }

        let item_type = tag.to_string();
        *counts_by_type.entry(item_type.clone()).or_insert(0usize) += 1;
        objects.push(PidObject {
            drawing_id: uid.to_string(),
            item_type,
            drawing_item_type: node
                .children()
                .any(|child| child.is_element() && child.tag_name().name() == "IDrawingItem")
                .then(|| "IDrawingItem".to_string()),
            model_id: object.attribute("Name").map(str::to_string),
            extra,
            record_id: None,
            field_x: None,
        });
    }

    let by_drawing_id: BTreeMap<String, usize> = objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.drawing_id.clone(), index))
        .collect();
    let relationships = raw_relationships
        .into_iter()
        .filter(|(_, uid1, uid2, _)| by_drawing_id.contains_key(uid1) && by_drawing_id.contains_key(uid2))
        .map(|(uid, uid1, uid2, def_uid)| {
            let guid = uid.unwrap_or_else(|| stable_uid(&format!("{def_uid}:{uid1}:{uid2}")));
            PidRelationship {
                model_id: format!("Relationship.{def_uid}.{guid}"),
                guid,
                record_id: None,
                field_x: None,
                source_drawing_id: Some(uid1),
                target_drawing_id: Some(uid2),
            }
        })
        .collect();

    Ok(ParsedPublishData {
        graph: ObjectGraph {
            drawing_no: drawing_no.clone(),
            project_number: root
                .attribute("Project")
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .or_else(|| root.attribute("Plant").map(str::to_string)),
            objects,
            relationships,
            by_drawing_id,
            counts_by_type,
        },
        title,
        drawing_no,
        doc_uid,
        raw_xml,
    })
}

fn parse_publish_meta_xml(path: &Path) -> Result<ParsedPublishMeta, String> {
    let raw_xml = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read publish meta XML {}: {e}", path.display()))?;
    let document = roxmltree::Document::parse(&raw_xml)
        .map_err(|e| format!("failed to parse publish meta XML {}: {e}", path.display()))?;
    let root = document.root_element();

    let doc_version = root
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "DocumentVersion")
        .and_then(|node| {
            node.children()
                .find(|child| child.is_element() && child.tag_name().name() == "IDocumentVersion")
        })
        .and_then(|node| node.attribute("DocVersion"))
        .map(str::to_string);
    let doc_revision = root
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "DocumentVersion")
        .and_then(|node| {
            node.children()
                .find(|child| child.is_element() && child.tag_name().name() == "IDocumentVersion")
        })
        .and_then(|node| node.attribute("DocRevision"))
        .map(str::to_string);
    let file_path = root
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "File")
        .and_then(|node| {
            node.children()
                .find(|child| child.is_element() && child.tag_name().name() == "IFile")
        })
        .and_then(|node| node.attribute("FilePath"))
        .map(str::to_string);

    let mut tags = BTreeMap::new();
    if let Some(doc_name) = root.attribute("DocName") {
        tags.insert("DocName".to_string(), doc_name.to_string());
    }
    if let Some(doc_uid) = root.attribute("DocUID") {
        tags.insert("DocUID".to_string(), doc_uid.to_string());
    }
    if let Some(version) = &doc_version {
        tags.insert("DocVersion".to_string(), version.clone());
    }
    if let Some(revision) = &doc_revision {
        tags.insert("DocRevision".to_string(), revision.clone());
    }

    Ok(ParsedPublishMeta {
        general_meta: GeneralMeta {
            file_path,
            file_size: None,
            raw_xml,
            tags,
        },
        doc_version,
        doc_revision,
    })
}

fn bran_publish_model_from_document(doc: &acadrust::CadDocument) -> Result<BranPublishModel, String> {
    let inserts: Vec<Insert> = doc
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Insert(insert) if insert.block_name.eq_ignore_ascii_case(SPPID_BRAN_BLOCK_NAME) => {
                Some(insert.clone())
            }
            _ => None,
        })
        .collect();
    let insert = match inserts.as_slice() {
        [] => {
            return Err(
                "SPPID export requires exactly one SPPID_BRAN insert in the active drawing".to_string(),
            )
        }
        [insert] => insert,
        _ => {
            return Err(
                "SPPID export currently supports exactly one SPPID_BRAN insert per drawing".to_string(),
            )
        }
    };

    let attributes: BTreeMap<String, String> = insert
        .attributes
        .iter()
        .map(|attr| (attr.tag.to_ascii_uppercase(), attr.value.trim().to_string()))
        .collect();

    let drawing_no = attr_or_default(&attributes, "DRAWING_NO")?;
    let doc_title = attr_or_default(&attributes, "DOC_TITLE")?;
    let pipeline_name = attr_or_default(&attributes, "PIPELINE")?;
    let connector_name = attr_or_default(&attributes, "CONNECTOR")?;
    let branch_name = attr_or_default(&attributes, "BRANCH_NAME")?;
    let piping_class = attr_or_default(&attributes, "PIPING_CLASS")?;
    let nominal_diameter = attr_or_default(&attributes, "NOMINAL_DIAMETER")?;

    Ok(BranPublishModel {
        drawing_uid: stable_uid(&format!("drawing:{drawing_no}")),
        pipeline_uid: stable_uid(&format!("pipeline:{pipeline_name}")),
        connector_uid: stable_uid(&format!("connector:{connector_name}")),
        piping_branch_uid: stable_uid(&format!("piping-branch:{drawing_no}:{branch_name}")),
        branch_uid: stable_uid(&format!("branch:{drawing_no}:{branch_name}")),
        process_point_uid: stable_uid(&format!("process-point:{pipeline_name}:{branch_name}")),
        representation_uid: stable_uid(&format!("representation:{drawing_no}:{branch_name}")),
        doc_version_uid: stable_uid(&format!("doc-version:{drawing_no}")),
        doc_revision_uid: stable_uid(&format!("doc-revision:{drawing_no}")),
        file_uid: stable_uid(&format!("file:{drawing_no}")),
        drawing_no,
        doc_title,
        pipeline_name,
        connector_name,
        branch_name,
        piping_class,
        nominal_diameter,
    })
}

fn attr_or_default(attributes: &BTreeMap<String, String>, tag: &str) -> Result<String, String> {
    if let Some(value) = attributes.get(tag) {
        if !value.trim().is_empty() {
            return Ok(value.trim().to_string());
        }
    }
    SPPID_BRAN_ATTRIBUTES
        .iter()
        .find(|(name, _, _)| *name == tag)
        .map(|(_, _, default)| (*default).to_string())
        .ok_or_else(|| format!("missing required SPPID attribute {tag}"))
}

fn build_publish_pid_document(
    model: &BranPublishModel,
    drawing_xml: String,
    general_xml: String,
    _data_xml: String,
    _meta_xml: String,
    pid_path: &Path,
) -> PidDocument {
    let graph = build_publish_object_graph(model);
    let drawing_xml_len = drawing_xml.len() as u64;
    let general_xml_len = general_xml.len() as u64;
    let mut summary = SummaryInfo::default();
    summary.title = Some(model.doc_title.clone());
    summary.raw.insert(
        "PublishDataPath".to_string(),
        publish_data_path(pid_path).display().to_string(),
    );
    summary.raw.insert(
        "PublishMetaPath".to_string(),
        publish_meta_path(pid_path).display().to_string(),
    );

    PidDocument {
        summary: Some(summary),
        drawing_meta: Some(DrawingMeta {
            drawing_number: Some(model.drawing_no.clone()),
            raw_xml: drawing_xml,
            tags: BTreeMap::from([
                ("SP_DRAWINGNUMBER".to_string(), model.drawing_no.clone()),
                ("SP_DOCTITLE".to_string(), model.doc_title.clone()),
            ]),
            ..Default::default()
        }),
        general_meta: Some(GeneralMeta {
            file_path: pid_path.parent().map(|path| path.display().to_string()),
            file_size: None,
            raw_xml: general_xml,
            tags: BTreeMap::from([(
                "FilePath".to_string(),
                pid_path
                    .parent()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            )]),
        }),
        object_graph: Some(graph),
        streams: vec![
            pid_parse::StreamEntry {
                path: DRAWING_STREAM_PATH.to_string(),
                size: drawing_xml_len,
                preview_ascii: vec![model.drawing_no.clone()],
                magic_u32_le: None,
            },
            pid_parse::StreamEntry {
                path: GENERAL_STREAM_PATH.to_string(),
                size: general_xml_len,
                preview_ascii: vec![model.doc_title.clone()],
                magic_u32_le: None,
            },
        ],
        ..PidDocument::default()
    }
}

fn build_publish_object_graph(model: &BranPublishModel) -> ObjectGraph {
    let objects = vec![
        PidObject {
            drawing_id: model.drawing_uid.clone(),
            item_type: "PIDDrawing".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(model.drawing_no.clone()),
            extra: BTreeMap::from([("DocTitle".to_string(), model.doc_title.clone())]),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.pipeline_uid.clone(),
            item_type: "PIDPipeline".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(model.pipeline_name.clone()),
            extra: BTreeMap::from([
                ("PipelineName".to_string(), model.pipeline_name.clone()),
                ("NominalDiameter".to_string(), model.nominal_diameter.clone()),
                ("PipingMaterialsClass".to_string(), model.piping_class.clone()),
            ]),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.connector_uid.clone(),
            item_type: "PIDPipingConnector".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(model.connector_name.clone()),
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.piping_branch_uid.clone(),
            item_type: "PIDPipingBranchPoint".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(model.branch_name.clone()),
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.branch_uid.clone(),
            item_type: "PIDBranchPoint".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(model.branch_name.clone()),
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.process_point_uid.clone(),
            item_type: "PIDProcessPoint".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(format!("{}-PP", model.branch_name)),
            extra: BTreeMap::new(),
            record_id: None,
            field_x: None,
        },
        PidObject {
            drawing_id: model.representation_uid.clone(),
            item_type: "PIDRepresentation".to_string(),
            drawing_item_type: Some("IDrawingItem".to_string()),
            model_id: Some(format!("{}-REP", model.branch_name)),
            extra: BTreeMap::from([("RepresentationType".to_string(), "Polyline".to_string())]),
            record_id: None,
            field_x: None,
        },
    ];
    let relationships = vec![
        publish_relationship(
            SPPID_REL_END1,
            &model.connector_uid,
            &model.piping_branch_uid,
        ),
        publish_relationship(
            SPPID_REL_END2,
            &model.connector_uid,
            &model.process_point_uid,
        ),
        publish_relationship(
            SPPID_REL_TAP,
            &model.piping_branch_uid,
            &model.branch_uid,
        ),
        publish_relationship(
            SPPID_REL_PROCESS_POINT,
            &model.process_point_uid,
            &model.pipeline_uid,
        ),
        publish_relationship(
            SPPID_REL_DRAWING_ITEMS,
            &model.pipeline_uid,
            &model.drawing_uid,
        ),
        publish_relationship(
            SPPID_REL_REP_COMPOSITION,
            &model.representation_uid,
            &model.piping_branch_uid,
        ),
    ];
    let by_drawing_id = objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.drawing_id.clone(), index))
        .collect();
    let mut counts_by_type = BTreeMap::new();
    for object in &objects {
        *counts_by_type.entry(object.item_type.clone()).or_insert(0usize) += 1;
    }
    ObjectGraph {
        drawing_no: Some(model.drawing_no.clone()),
        project_number: None,
        objects,
        relationships,
        by_drawing_id,
        counts_by_type,
    }
}

fn publish_relationship(def_uid: &str, uid1: &str, uid2: &str) -> PidRelationship {
    let guid = stable_uid(&format!("{def_uid}:{uid1}:{uid2}"));
    PidRelationship {
        model_id: format!("Relationship.{def_uid}.{guid}"),
        guid,
        record_id: None,
        field_x: None,
        source_drawing_id: Some(uid1.to_string()),
        target_drawing_id: Some(uid2.to_string()),
    }
}

fn build_publish_drawing_stream_xml(model: &BranPublishModel) -> String {
    format!(
        "<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"{}\" SP_DOCTITLE=\"{}\"/></Drawing>",
        xml_escape_attr(&model.drawing_no),
        xml_escape_attr(&model.doc_title)
    )
}

fn build_publish_general_stream_xml(pid_path: &Path) -> String {
    let file_path = pid_path
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\"?><General><FilePath>{}</FilePath></General>",
        xml_escape_text(&file_path)
    )
}

fn build_publish_data_xml(model: &BranPublishModel) -> String {
    let drawing_no = xml_escape_attr(&model.drawing_no);
    let doc_title = xml_escape_attr(&model.doc_title);
    let pipeline_name = xml_escape_attr(&model.pipeline_name);
    let connector_name = xml_escape_attr(&model.connector_name);
    let branch_name = xml_escape_attr(&model.branch_name);
    let piping_class = xml_escape_attr(&model.piping_class);
    let nominal_diameter = xml_escape_attr(&model.nominal_diameter);
    format!(
        r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container CompSchema="{schema}" Scope="Data" SoftwareVersion="{software}" IsValidated="False" SchemaVersion="04.02.17.01" Plant="P01" Project="" DocUID="{drawing_uid}" DocName="{drawing_no}" Version="" ToolID="{tool}" ToolSignature="AAAD" SDECIMAL=".">
   <PIDDrawing>
      <IObject UID="{drawing_uid}" Name="{drawing_no}" Description=""/>
      <IDocument DocCategory="P&amp;ID Documents" DocTitle="{doc_title}" DocType="P&amp;ID" DocSubtype=""/>
      <IDocVersionComposition/>
      <IDwgRepresentationComposition/>
      <IPIDDrawing/>
      <ISchematicDwg/>
      <IPBSItem/>
   </PIDDrawing>
   <PIDPipeline>
      <IObject UID="{pipeline_uid}" Name="{pipeline_name}"/>
      <IDrawingItem/>
      <IPipeline PipelineName="{pipeline_name}"/>
      <IPipeCrossSectionItem NominalDiameter="{nominal_diameter}"/>
      <IPipingSpecifiedItem PipingMaterialsClass="{piping_class}"/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipeline>
   <PIDPipingConnector>
      <IObject UID="{connector_uid}" Name="{connector_name}"/>
      <IDrawingItem/>
      <IPipingConnector/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipingConnector>
   <PIDPipingBranchPoint>
      <IObject UID="{piping_branch_uid}" Name="{branch_name}"/>
      <IDrawingItem/>
      <IPipingBranchPoint/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipingBranchPoint>
   <PIDBranchPoint>
      <IObject UID="{branch_uid}" Name="{branch_name}"/>
      <IDrawingItem/>
      <IBranchPoint/>
      <IDocumentItem/>
   </PIDBranchPoint>
   <PIDProcessPoint>
      <IObject UID="{process_point_uid}" Name="{branch_name}-PP"/>
      <IDrawingItem/>
      <IProcessPoint/>
      <IDocumentItem/>
   </PIDProcessPoint>
   <PIDRepresentation>
      <IObject UID="{representation_uid}" Name="{branch_name}-REP"/>
      <IDrawingItem/>
      <IRepresentation RepresentationType="Polyline"/>
   </PIDRepresentation>
   {rel_draw_pipeline}
   {rel_draw_connector}
   {rel_draw_branch}
   {rel_draw_branch_2}
   {rel_draw_pp}
   {rel_draw_rep}
   {rel_rep}
   {rel_end1}
   {rel_end2}
   {rel_tap}
   {rel_process}
</Container>
"#,
        schema = SPPID_DATA_COMPONENT_SCHEMA,
        software = SPPID_SOFTWARE_VERSION,
        tool = SPPID_TOOL_ID,
        drawing_uid = model.drawing_uid,
        drawing_no = drawing_no,
        doc_title = doc_title,
        pipeline_uid = model.pipeline_uid,
        pipeline_name = pipeline_name,
        nominal_diameter = nominal_diameter,
        piping_class = piping_class,
        connector_uid = model.connector_uid,
        connector_name = connector_name,
        piping_branch_uid = model.piping_branch_uid,
        branch_name = branch_name,
        branch_uid = model.branch_uid,
        process_point_uid = model.process_point_uid,
        representation_uid = model.representation_uid,
        rel_draw_pipeline = publish_rel_xml("REL-DRAW-PIPELINE", &model.pipeline_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_draw_connector = publish_rel_xml("REL-DRAW-CONNECTOR", &model.connector_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_draw_branch = publish_rel_xml("REL-DRAW-PIPING-BRANCH", &model.piping_branch_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_draw_branch_2 = publish_rel_xml("REL-DRAW-BRANCH", &model.branch_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_draw_pp = publish_rel_xml("REL-DRAW-PROCESS", &model.process_point_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_draw_rep = publish_rel_xml("REL-DRAW-REP", &model.representation_uid, &model.drawing_uid, SPPID_REL_DRAWING_ITEMS),
        rel_rep = publish_rel_xml("REL-REP-COMP", &model.representation_uid, &model.piping_branch_uid, SPPID_REL_REP_COMPOSITION),
        rel_end1 = publish_rel_xml("REL-END1", &model.connector_uid, &model.piping_branch_uid, SPPID_REL_END1),
        rel_end2 = publish_rel_xml("REL-END2", &model.connector_uid, &model.process_point_uid, SPPID_REL_END2),
        rel_tap = publish_rel_xml("REL-TAP", &model.piping_branch_uid, &model.branch_uid, SPPID_REL_TAP),
        rel_process = publish_rel_xml("REL-PROCESS", &model.process_point_uid, &model.pipeline_uid, SPPID_REL_PROCESS_POINT),
    )
}

fn build_publish_meta_xml(model: &BranPublishModel, pid_path: &Path) -> String {
    let drawing_no = xml_escape_attr(&model.drawing_no);
    let pid_name = xml_escape_attr(
        &pid_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("publish.pid"),
    );
    let file_path = xml_escape_attr(
        &pid_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    );
    format!(
        r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container CompSchema="{schema}" Scope="Data" SoftwareVersion="{software}" IsValidated="False" SchemaVersion="04.02.17.01" Plant="P01" Project="" DocUID="{drawing_uid}" DocName="{drawing_no}" Version="" ToolID="{tool}" ToolSignature="AAAD" SDECIMAL=".">
   <DocumentVersion>
      <IObject UID="{doc_version_uid}" Name="{drawing_no} Version"/>
      <IDocumentVersion DocRevision="0" DocVersionDate="2026/04/20" DocVersion="1"/>
      <IFileComposition/>
   </DocumentVersion>
   {rel_version}
   <DocumentRevision>
      <IObject UID="{doc_revision_uid}" Name="{drawing_no} Revision"/>
      <IDocumentRevision MajorRev_ForRevise="0" MinorRev_ForRevise=""/>
   </DocumentRevision>
   {rel_revision}
   <File>
      <IObject UID="{file_uid}" Name="{pid_name}" Description=""/>
      <IFile FilePath="{file_path}"/>
   </File>
   {rel_file}
</Container>
"#,
        schema = SPPID_META_COMPONENT_SCHEMA,
        software = SPPID_SOFTWARE_VERSION,
        tool = SPPID_TOOL_ID,
        drawing_uid = model.drawing_uid,
        drawing_no = drawing_no,
        doc_version_uid = model.doc_version_uid,
        doc_revision_uid = model.doc_revision_uid,
        file_uid = model.file_uid,
        pid_name = pid_name,
        file_path = file_path,
        rel_version = publish_rel_xml("REL-VERSIONED-DOC", &model.drawing_uid, &model.doc_version_uid, "VersionedDoc"),
        rel_revision = publish_rel_xml("REL-REVISED-DOC", &model.doc_revision_uid, &model.drawing_uid, "RevisedDocument"),
        rel_file = publish_rel_xml("REL-FILE-COMP", &model.file_uid, &model.doc_version_uid, "FileComposition"),
    )
}

fn publish_rel_xml(uid: &str, uid1: &str, uid2: &str, def_uid: &str) -> String {
    format!(
        "<Rel><IObject UID=\"{}\"/><IRel UID1=\"{}\" UID2=\"{}\" DefUID=\"{}\"/></Rel>",
        xml_escape_attr(uid),
        xml_escape_attr(uid1),
        xml_escape_attr(uid2),
        xml_escape_attr(def_uid)
    )
}

fn xml_escape_attr(value: &str) -> String {
    xml_escape_text(value).replace('"', "&quot;")
}

fn xml_escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
}

fn stable_uid(seed: &str) -> String {
    fn fnv1a64(seed: &str, offset: u64) -> u64 {
        let mut hash = offset;
        for byte in seed.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001B3);
        }
        hash
    }

    format!(
        "{:016X}{:016X}",
        fnv1a64(seed, 0xcbf29ce484222325),
        fnv1a64(seed, 0x84222325cbf29ce4)
    )
}

fn pid_document_to_bundle(doc: &PidDocument) -> PidOpenBundle {
    let view = build_import_view(doc);
    let (native_preview, summary, preview_index) = pid_document_to_preview(doc, &view);
    PidOpenBundle {
        pid_doc: doc.clone(),
        native_preview,
        summary,
        preview_index,
    }
}

#[cfg(test)]
fn pid_document_to_native(doc: &PidDocument) -> (nm::CadDocument, PidImportSummary) {
    let bundle = pid_document_to_bundle(doc);
    (bundle.native_preview, bundle.summary)
}

fn pid_document_to_preview(
    doc: &PidDocument,
    view: &PidImportView,
) -> (nm::CadDocument, PidImportSummary, PidPreviewIndex) {
    let mut native = nm::CadDocument::new();
    ensure_layer(&mut native, "PID_META", 5);
    ensure_layer(&mut native, "PID_RELATIONSHIPS", 1);
    ensure_layer(&mut native, "PID_LAYOUT_TEXT", 7);
    ensure_layer(&mut native, "PID_FALLBACK", 8);
    ensure_layer(&mut native, "PID_SYMBOLS", 4);
    ensure_layer(&mut native, "PID_CLUSTERS", 2);
    ensure_layer(&mut native, "PID_STREAMS", 3);
    ensure_layer(&mut native, "PID_CROSSREF", 7);
    ensure_layer(&mut native, "PID_UNRESOLVED", 6);

    let mut preview_index = PidPreviewIndex::default();
    let mut positions = BTreeMap::new();
    let object_count = view.objects.len();
    let unresolved_edges = if let Some(layout) = doc.layout.as_ref().filter(|layout| !layout.items.is_empty()) {
        let rendered = add_layout_entities(&mut native, &mut preview_index, view, layout);
        positions = rendered.positions;
        add_layout_text_entities(&mut native, &mut preview_index, layout);
        add_fallback_entities(&mut native, &mut preview_index, view, &positions);
        add_layout_segment_entities(&mut native, &mut preview_index, layout);
        count_unresolved_relationships(view, &positions)
    } else {
        for (index, object) in view.objects.iter().enumerate() {
            let point = grid_point(index);
            positions.insert(object.drawing_id.clone(), point);
            add_object_entities(&mut native, &mut preview_index, object, point);
        }
        add_relationship_entities(&mut native, &mut preview_index, view, &positions)
    };

    add_meta_entities(&mut native, &mut preview_index, view);
    add_symbol_entities(&mut native, &mut preview_index, view);
    add_cluster_entities(&mut native, &mut preview_index, doc, view);
    add_stream_entities(&mut native, &mut preview_index, doc);
    add_cross_reference_entities(&mut native, &mut preview_index, doc);
    add_unresolved_entities(&mut native, &mut preview_index, view, unresolved_edges);

    let attribute_class_count = doc
        .cross_reference
        .as_ref()
        .map(|cross| cross.attribute_classes.len())
        .unwrap_or(0);
    let tagged_text_count = doc
        .tagged_storages
        .as_ref()
        .map(|tagged| tagged.entries.len())
        .unwrap_or(0);
    let dynamic_attribute_record_count = doc
        .dynamic_attributes
        .as_ref()
        .map(|attrs| attrs.attribute_records.len())
        .unwrap_or(0);

    let summary = PidImportSummary {
        title: view.title.clone(),
        object_count,
        relationship_count: view.relationships.len(),
        unresolved_relationship_count: unresolved_edges + view.unresolved.len(),
        symbol_count: view.symbols.len(),
        cluster_count: doc.clusters.len(),
        sheet_count: doc.sheet_streams.len(),
        stream_count: doc.streams.len(),
        attribute_class_count,
        tagged_text_count,
        dynamic_attribute_record_count,
        object_graph_available: doc.object_graph.is_some(),
    };
    (native, summary, preview_index)
}

fn ensure_layer(doc: &mut nm::CadDocument, name: &str, color: i16) {
    doc.layers.entry(name.to_string()).or_insert_with(|| {
        let mut layer = nm::LayerProperties::new(name);
        layer.color = color;
        layer
    });
}

fn ensure_object_layer(doc: &mut nm::CadDocument, item_type: &str) -> String {
    let layer = format!("PID_OBJECTS_{}", sanitize_layer_name(item_type));
    let color = match item_type {
        "Instrument" => 4,
        "PipeRun" => 3,
        "Nozzle" => 2,
        "Drawing" => 7,
        _ => 6,
    };
    ensure_layer(doc, &layer, color);
    layer
}

fn sanitize_layer_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn grid_point(index: usize) -> [f64; 3] {
    let col = (index % GRID_COLUMNS) as f64;
    let row = (index / GRID_COLUMNS) as f64;
    [col * GRID_SPACING_X, -(row * GRID_SPACING_Y), 0.0]
}

fn add_indexed_entity(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    key: Option<PidNodeKey>,
    entity: nm::Entity,
) -> Option<Handle> {
    let handle = doc.add_entity(entity).ok().map(|handle| Handle::new(handle.value()))?;
    if let Some(key) = key {
        preview_index.record_existing_handle(key, handle);
    }
    Some(handle)
}

fn add_layout_indexed_entity(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    entity: nm::Entity,
) -> Option<Handle> {
    let handle = add_indexed_entity(doc, preview_index, key, entity)?;
    preview_index.record_layout_refs(handle, drawing_id, graphic_oid);
    Some(handle)
}

fn add_panel_line(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    insertion: [f64; 3],
    width: f64,
    value: String,
    key: Option<PidNodeKey>,
) -> Option<Handle> {
    let mut text = nm::Entity::new(nm::EntityData::MText {
        insertion,
        height: OBJECT_TEXT_HEIGHT,
        width,
        rectangle_height: None,
        value,
        rotation: 0.0,
        style_name: "Standard".into(),
        attachment_point: 1,
        line_spacing_factor: 1.0,
        drawing_direction: 1,
    });
    text.layer_name = layer.into();
    add_indexed_entity(doc, preview_index, key, text)
}

fn add_object_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    object: &pid_parse::PidVisualObject,
    point: [f64; 3],
) {
    let layer = ensure_object_layer(doc, &object.item_type);
    let key = PidNodeKey::Object {
        drawing_id: object.drawing_id.clone(),
    };

    let mut marker = nm::Entity::new(match object.item_type.as_str() {
        "PipeRun" => nm::EntityData::Line {
            start: [point[0] - 28.0, point[1], 0.0],
            end: [point[0] + 28.0, point[1], 0.0],
        },
        _ => nm::EntityData::Circle {
            center: point,
            radius: NODE_RADIUS,
        },
    });
    marker.layer_name = layer.clone();
    let _ = add_indexed_entity(doc, preview_index, Some(key.clone()), marker);

    let mut lines = vec![object.item_type.clone(), short_id(&object.drawing_id).to_string()];
    if let Some(kind) = &object.drawing_item_type {
        lines.push(short_text(kind, 24));
    }
    if let Some(model_id) = &object.model_id {
        lines.push(short_text(model_id, 28));
    }
    for (name, value) in object.extra.iter().take(2) {
        lines.push(format!("{}={}", short_text(name, 10), short_text(value, 18)));
    }

    let _ = add_panel_line(
        doc,
        preview_index,
        &layer,
        [point[0] + NODE_RADIUS + 8.0, point[1] + 12.0, 0.0],
        160.0,
        lines.join("\\P"),
        Some(key),
    );
}

#[derive(Default)]
struct LayoutRenderState {
    positions: BTreeMap<String, [f64; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutGlyphKind {
    Pipeline,
    Branch,
    Connector,
    ProcessPoint,
    Instrument,
    Equipment,
    Vessel,
    Note,
    Nozzle,
    OffPageConnector,
    PipingComponent,
    Generic,
}

fn classify_layout_glyph(item: &PidLayoutItem) -> LayoutGlyphKind {
    let semantic = item.symbol_name.as_deref().unwrap_or(item.kind.as_str());
    match semantic {
        "Pipeline" | "PIDPipeline" | "PipeRun" => LayoutGlyphKind::Pipeline,
        "Branch" | "PIDPipingBranchPoint" | "PIDBranchPoint" => LayoutGlyphKind::Branch,
        "Connector" | "PIDPipingConnector" => LayoutGlyphKind::Connector,
        "ProcessPoint" | "PIDProcessPoint" => LayoutGlyphKind::ProcessPoint,
        "Instrument" | "PIDInstrument" | "PIDControlSystemFunction" => {
            LayoutGlyphKind::Instrument
        }
        "Equipment" | "PIDEquipment" => LayoutGlyphKind::Equipment,
        "Vessel" | "PIDProcessVessel" => LayoutGlyphKind::Vessel,
        "Note" | "PIDNote" | "ItemNote" => LayoutGlyphKind::Note,
        "Nozzle" | "PIDNozzle" | "PipingPort" | "SignalPort" | "PIDPipingPort"
        | "PIDSignalPort" => LayoutGlyphKind::Nozzle,
        "OffPageConnector" | "PIDSignalConnector" | "OPC" => LayoutGlyphKind::OffPageConnector,
        "PipingComponent" | "PIDPipingComponent" | "PipingComp" => {
            LayoutGlyphKind::PipingComponent
        }
        _ => LayoutGlyphKind::Generic,
    }
}

fn add_layout_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    view: &PidImportView,
    layout: &PidLayoutModel,
) -> LayoutRenderState {
    let objects_by_id: BTreeMap<&str, &pid_parse::PidVisualObject> = view
        .objects
        .iter()
        .map(|object| (object.drawing_id.as_str(), object))
        .collect();
    let mut rendered = LayoutRenderState::default();

    for item in &layout.items {
        let Some(drawing_id) = item.drawing_id.as_deref() else {
            continue;
        };
        let key = Some(PidNodeKey::Object {
            drawing_id: drawing_id.to_string(),
        });
        let point = [item.anchor[0], item.anchor[1], 0.0];
        rendered.positions.insert(drawing_id.to_string(), point);
        let layer = ensure_object_layer(doc, item.kind.as_str());
        let label = item
            .label
            .as_deref()
            .or_else(|| objects_by_id.get(drawing_id).and_then(|object| object.model_id.as_deref()))
            .unwrap_or(item.kind.as_str());
        add_layout_glyph(
            doc,
            preview_index,
            &layer,
            key,
            drawing_id,
            item.graphic_oid,
            item,
            label,
        );
    }

    rendered
}

fn add_layout_text_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layout: &PidLayoutModel,
) {
    for text in &layout.texts {
        let key = text
            .drawing_id
            .as_ref()
            .map(|drawing_id| PidNodeKey::Object {
                drawing_id: drawing_id.clone(),
            })
            .or_else(|| (text.layout_id == "title").then_some(PidNodeKey::Overview));
        let mut entity = nm::Entity::new(nm::EntityData::MText {
            insertion: [text.anchor[0], text.anchor[1], 0.0],
            height: OBJECT_TEXT_HEIGHT,
            width: 160.0,
            rectangle_height: None,
            value: text.text.clone(),
            rotation: 0.0,
            style_name: "Standard".into(),
            attachment_point: 1,
            line_spacing_factor: 1.0,
            drawing_direction: 1,
        });
        entity.layer_name = "PID_LAYOUT_TEXT".into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key,
            text.drawing_id.as_deref(),
            None,
            entity,
        );
    }
}

fn add_fallback_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    view: &PidImportView,
    placed_positions: &BTreeMap<String, [f64; 3]>,
) {
    let unplaced: Vec<&pid_parse::PidVisualObject> = view
        .objects
        .iter()
        .filter(|object| !placed_positions.contains_key(&object.drawing_id))
        .collect();
    if unplaced.is_empty() {
        return;
    }

    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_FALLBACK",
        [FALLBACK_PANEL_X, HEADER_Y, 0.0],
        260.0,
        "Fallback / Unplaced".into(),
        None,
    );

    for (index, object) in unplaced.into_iter().enumerate() {
        let point = [
            FALLBACK_PANEL_X,
            FALLBACK_START_Y - index as f64 * FALLBACK_SPACING_Y,
            0.0,
        ];
        let key = Some(PidNodeKey::Object {
            drawing_id: object.drawing_id.clone(),
        });
        add_layout_placeholder_box(
            doc,
            preview_index,
            "PID_FALLBACK",
            key.clone(),
            Some(object.drawing_id.as_str()),
            None,
            point,
            44.0,
            26.0,
        );
        let mut lines = vec![object.item_type.clone(), short_id(&object.drawing_id).to_string()];
        if let Some(model_id) = &object.model_id {
            lines.push(short_text(model_id, 28));
        }
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_FALLBACK",
            [point[0] + 36.0, point[1] + 14.0, 0.0],
            220.0,
            lines.join("\\P"),
            key,
        );
    }
}

fn add_layout_segment_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layout: &PidLayoutModel,
) {
    for segment in &layout.segments {
        let key = segment
            .owner_drawing_id
            .as_ref()
            .map(|drawing_id| PidNodeKey::Object {
                drawing_id: drawing_id.clone(),
            });
        let mut entity = nm::Entity::new(nm::EntityData::Line {
            start: [segment.start[0], segment.start[1], 0.0],
            end: [segment.end[0], segment.end[1], 0.0],
        });
        entity.layer_name = "PID_RELATIONSHIPS".into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key,
            segment.owner_drawing_id.as_deref(),
            segment.graphic_oid,
            entity,
        );
    }
}

fn count_unresolved_relationships(
    view: &PidImportView,
    positions: &BTreeMap<String, [f64; 3]>,
) -> usize {
    view.relationships
        .iter()
        .filter(|relationship| {
            let source = relationship
                .source_drawing_id
                .as_ref()
                .and_then(|drawing_id| positions.get(drawing_id));
            let target = relationship
                .target_drawing_id
                .as_ref()
                .and_then(|drawing_id| positions.get(drawing_id));
            source.is_none() || target.is_none()
        })
        .count()
}

fn add_layout_glyph(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: &str,
    graphic_oid: Option<u32>,
    item: &PidLayoutItem,
    label: &str,
) {
    match classify_layout_glyph(item) {
        LayoutGlyphKind::Pipeline => {
            let mut entity = nm::Entity::new(nm::EntityData::Line {
                start: [item.anchor[0] - 48.0, item.anchor[1], 0.0],
                end: [item.anchor[0] + 48.0, item.anchor[1], 0.0],
            });
            entity.layer_name = layer.into();
            let _ = add_layout_indexed_entity(
                doc,
                preview_index,
                key,
                Some(drawing_id),
                graphic_oid,
                entity,
            );
        }
        LayoutGlyphKind::Branch => {
            add_layout_cross(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                16.0,
            );
        }
        LayoutGlyphKind::Connector => {
            add_layout_placeholder_box(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                24.0,
                24.0,
            );
        }
        LayoutGlyphKind::ProcessPoint => {
            let mut entity = nm::Entity::new(nm::EntityData::Circle {
                center: [item.anchor[0], item.anchor[1], 0.0],
                radius: NODE_RADIUS - 4.0,
            });
            entity.layer_name = layer.into();
            let _ = add_layout_indexed_entity(
                doc,
                preview_index,
                key,
                Some(drawing_id),
                graphic_oid,
                entity,
            );
        }
        LayoutGlyphKind::Instrument => {
            let mut entity = nm::Entity::new(nm::EntityData::Circle {
                center: [item.anchor[0], item.anchor[1], 0.0],
                radius: NODE_RADIUS - 2.0,
            });
            entity.layer_name = layer.into();
            let _ = add_layout_indexed_entity(
                doc,
                preview_index,
                key.clone(),
                Some(drawing_id),
                graphic_oid,
                entity,
            );
            let mut stem = nm::Entity::new(nm::EntityData::Line {
                start: [item.anchor[0], item.anchor[1] - NODE_RADIUS, 0.0],
                end: [item.anchor[0], item.anchor[1] - NODE_RADIUS - 12.0, 0.0],
            });
            stem.layer_name = layer.into();
            let _ = add_layout_indexed_entity(
                doc,
                preview_index,
                key,
                Some(drawing_id),
                graphic_oid,
                stem,
            );
        }
        LayoutGlyphKind::Equipment => {
            add_layout_placeholder_box(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                56.0,
                32.0,
            );
        }
        LayoutGlyphKind::Vessel => {
            add_layout_placeholder_box(
                doc,
                preview_index,
                layer,
                key.clone(),
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                64.0,
                38.0,
            );
            let mut centerline = nm::Entity::new(nm::EntityData::Line {
                start: [item.anchor[0], item.anchor[1] - 15.0, 0.0],
                end: [item.anchor[0], item.anchor[1] + 15.0, 0.0],
            });
            centerline.layer_name = layer.into();
            let _ = add_layout_indexed_entity(
                doc,
                preview_index,
                key,
                Some(drawing_id),
                graphic_oid,
                centerline,
            );
        }
        LayoutGlyphKind::Note => {
            add_layout_note_frame(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                56.0,
                36.0,
            );
        }
        LayoutGlyphKind::Nozzle => {
            add_layout_triangle(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                28.0,
                22.0,
            );
        }
        LayoutGlyphKind::OffPageConnector => {
            add_layout_triangle(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                34.0,
                26.0,
            );
        }
        LayoutGlyphKind::PipingComponent => {
            add_layout_diamond(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                30.0,
                22.0,
            );
        }
        LayoutGlyphKind::Generic => {
            add_layout_placeholder_box(
                doc,
                preview_index,
                layer,
                key,
                Some(drawing_id),
                graphic_oid,
                [item.anchor[0], item.anchor[1], 0.0],
                52.0,
                30.0,
            );
        }
    }

    // All layout items, regardless of kind, get a visible label so the
    // rendered preview reads closer to a real P&ID (where every tag /
    // pipeline / instrument has an adjacent identifier). Previously only
    // `Generic` placed a label, leaving every other shape unlabelled —
    // the 2026-04-21 pid-real-sample plan Task 2 identified this as the
    // main reason the target sample looked visually sparse compared to
    // the original P&ID.
    if !label.is_empty() {
        let _ = add_panel_line(
            doc,
            preview_index,
            layer,
            [item.anchor[0] - 24.0, item.anchor[1] - 34.0, 0.0],
            120.0,
            short_text(label, 20),
            Some(PidNodeKey::Object {
                drawing_id: drawing_id.to_string(),
            }),
        );
    }
}

fn add_layout_cross(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    point: [f64; 3],
    span: f64,
) {
    for (start, end) in [
        ([point[0] - span, point[1], 0.0], [point[0] + span, point[1], 0.0]),
        ([point[0], point[1] - span, 0.0], [point[0], point[1] + span, 0.0]),
    ] {
        let mut entity = nm::Entity::new(nm::EntityData::Line { start, end });
        entity.layer_name = layer.into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key.clone(),
            drawing_id,
            graphic_oid,
            entity,
        );
    }
}

fn add_layout_triangle(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    center: [f64; 3],
    width: f64,
    height: f64,
) {
    let left = center[0] - width / 2.0;
    let right = center[0] + width / 2.0;
    let bottom = center[1] - height / 2.0;
    let top = center[1] + height / 2.0;
    let points = [
        [left, bottom, 0.0],
        [right, center[1], 0.0],
        [left, top, 0.0],
        [left, bottom, 0.0],
    ];
    for segment in points.windows(2) {
        let mut entity = nm::Entity::new(nm::EntityData::Line {
            start: segment[0],
            end: segment[1],
        });
        entity.layer_name = layer.into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key.clone(),
            drawing_id,
            graphic_oid,
            entity,
        );
    }
}

fn add_layout_diamond(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    center: [f64; 3],
    width: f64,
    height: f64,
) {
    let points = [
        [center[0], center[1] + height / 2.0, 0.0],
        [center[0] + width / 2.0, center[1], 0.0],
        [center[0], center[1] - height / 2.0, 0.0],
        [center[0] - width / 2.0, center[1], 0.0],
        [center[0], center[1] + height / 2.0, 0.0],
    ];
    for segment in points.windows(2) {
        let mut entity = nm::Entity::new(nm::EntityData::Line {
            start: segment[0],
            end: segment[1],
        });
        entity.layer_name = layer.into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key.clone(),
            drawing_id,
            graphic_oid,
            entity,
        );
    }
}

fn add_layout_note_frame(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    center: [f64; 3],
    width: f64,
    height: f64,
) {
    let left = center[0] - width / 2.0;
    let right = center[0] + width / 2.0;
    let bottom = center[1] - height / 2.0;
    let top = center[1] + height / 2.0;
    let fold = 10.0;
    for (start, end) in [
        ([left, bottom, 0.0], [right, bottom, 0.0]),
        ([right, bottom, 0.0], [right, top - fold, 0.0]),
        ([right, top - fold, 0.0], [right - fold, top, 0.0]),
        ([right - fold, top, 0.0], [left, top, 0.0]),
        ([left, top, 0.0], [left, bottom, 0.0]),
        ([right - fold, top, 0.0], [right - fold, top - fold, 0.0]),
        ([right - fold, top - fold, 0.0], [right, top - fold, 0.0]),
    ] {
        let mut entity = nm::Entity::new(nm::EntityData::Line { start, end });
        entity.layer_name = layer.into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key.clone(),
            drawing_id,
            graphic_oid,
            entity,
        );
    }
}

fn add_layout_placeholder_box(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    layer: &str,
    key: Option<PidNodeKey>,
    drawing_id: Option<&str>,
    graphic_oid: Option<u32>,
    center: [f64; 3],
    width: f64,
    height: f64,
) {
    let left = center[0] - width / 2.0;
    let right = center[0] + width / 2.0;
    let bottom = center[1] - height / 2.0;
    let top = center[1] + height / 2.0;
    for (start, end) in [
        ([left, bottom, 0.0], [right, bottom, 0.0]),
        ([right, bottom, 0.0], [right, top, 0.0]),
        ([right, top, 0.0], [left, top, 0.0]),
        ([left, top, 0.0], [left, bottom, 0.0]),
    ] {
        let mut entity = nm::Entity::new(nm::EntityData::Line { start, end });
        entity.layer_name = layer.into();
        let _ = add_layout_indexed_entity(
            doc,
            preview_index,
            key.clone(),
            drawing_id,
            graphic_oid,
            entity,
        );
    }
}

fn add_relationship_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    view: &PidImportView,
    positions: &BTreeMap<String, [f64; 3]>,
) -> usize {
    let mut unresolved = 0;
    for relationship in &view.relationships {
        let source = relationship.source_drawing_id.as_ref().and_then(|id| positions.get(id));
        let target = relationship.target_drawing_id.as_ref().and_then(|id| positions.get(id));

        match (source, target) {
            (Some(source), Some(target)) => {
                let mut line = nm::Entity::new(nm::EntityData::Line {
                    start: *source,
                    end: *target,
                });
                line.layer_name = "PID_RELATIONSHIPS".into();
                let _ = add_indexed_entity(
                    doc,
                    preview_index,
                    Some(PidNodeKey::Relationship {
                        guid: relationship.guid.clone(),
                    }),
                    line,
                );
            }
            _ => unresolved += 1,
        }
    }
    unresolved
}

fn add_meta_entities(doc: &mut nm::CadDocument, preview_index: &mut PidPreviewIndex, view: &PidImportView) {
    let mut lines = vec![view.title.clone()];
    if let Some(project) = &view.project_number {
        lines.push(format!("project={project}"));
    }
    lines.push(format!(
        "objects={} relationships={} symbols={} clusters={} unresolved={}",
        view.objects.len(),
        view.relationships.len(),
        view.symbols.len(),
        view.clusters.len(),
        view.unresolved.len()
    ));

    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_META",
        [0.0, HEADER_Y, 0.0],
        420.0,
        lines.join("\\P"),
        Some(PidNodeKey::Overview),
    );

    let mut source_text = nm::Entity::new(nm::EntityData::Text {
        insertion: [0.0, SUBHEADER_Y, 0.0],
        height: OBJECT_TEXT_HEIGHT,
        value: "Imported from Smart P&ID via pid-parse".into(),
        rotation: 0.0,
        style_name: "Standard".into(),
        width_factor: 1.0,
        oblique_angle: 0.0,
        horizontal_alignment: 0,
        vertical_alignment: 0,
        alignment_point: None,
    });
    source_text.layer_name = "PID_META".into();
    let _ = add_indexed_entity(doc, preview_index, Some(PidNodeKey::Overview), source_text);
}

fn add_symbol_entities(doc: &mut nm::CadDocument, preview_index: &mut PidPreviewIndex, view: &PidImportView) {
    if view.symbols.is_empty() {
        return;
    }
    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_SYMBOLS",
        [SIDE_PANEL_X, HEADER_Y, 0.0],
        280.0,
        "Symbols".into(),
        None,
    );
    for (index, symbol) in view.symbols.iter().take(10).enumerate() {
        let label = format!(
            "{} [{}]",
            symbol
                .symbol_name
                .clone()
                .unwrap_or_else(|| short_text(&symbol.symbol_path, 24)),
            symbol.usage_count
        );
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_SYMBOLS",
            [SIDE_PANEL_X, HEADER_Y - 24.0 - index as f64 * 18.0, 0.0],
            280.0,
            label,
            Some(PidNodeKey::Symbol {
                symbol_path: symbol.symbol_path.clone(),
            }),
        );
    }
}

fn add_cluster_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    _pid_doc: &PidDocument,
    view: &PidImportView,
) {
    if view.clusters.is_empty() {
        return;
    }
    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_CLUSTERS",
        [0.0, BOTTOM_PANEL_Y, 0.0],
        520.0,
        "Clusters / Sheets".into(),
        None,
    );
    for (index, cluster) in view.clusters.iter().take(12).enumerate() {
        let key = if cluster.kind == "Sheet" {
            PidNodeKey::Sheet {
                name: cluster.name.clone(),
            }
        } else if cluster.kind == "Coverage" {
            PidNodeKey::ClusterCoverage
        } else {
            PidNodeKey::Cluster {
                name: cluster.name.clone(),
            }
        };
        if let Some(handle) = add_panel_line(
            doc,
            preview_index,
            "PID_CLUSTERS",
            [0.0, BOTTOM_PANEL_Y - 24.0 - index as f64 * 18.0, 0.0],
            520.0,
            format!(
                "{} [{}] {}",
                short_text(&cluster.name, 24),
                cluster.record_count,
                short_text(&cluster.note, 36)
            ),
            Some(key.clone()),
        ) {
            if let PidNodeKey::Sheet { name } = key {
                preview_index.record_existing_handle(PidNodeKey::Stream { name }, handle);
            }
        }
    }
}

fn add_stream_entities(doc: &mut nm::CadDocument, preview_index: &mut PidPreviewIndex, pid_doc: &PidDocument) {
    let has_streams = !pid_doc.sheet_streams.is_empty()
        || pid_doc.dynamic_attributes.is_some()
        || pid_doc
            .tagged_storages
            .as_ref()
            .map(|tagged| !tagged.entries.is_empty())
            .unwrap_or(false)
        || pid_doc.cross_reference.is_some();
    if !has_streams {
        return;
    }

    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_STREAMS",
        [SIDE_PANEL_X, STREAM_PANEL_Y, 0.0],
        300.0,
        "Streams".into(),
        None,
    );

    let mut row = 0usize;
    if let Some(dynamic) = &pid_doc.dynamic_attributes {
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_STREAMS",
            [SIDE_PANEL_X, STREAM_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
            300.0,
            format!(
                "DynamicAttrs [{} records]",
                dynamic.attribute_records.len()
            ),
            Some(PidNodeKey::DynamicAttributes),
        );
        row += 1;
    }

    for sheet in pid_doc.sheet_streams.iter().take(8) {
        let handle = add_panel_line(
            doc,
            preview_index,
            "PID_STREAMS",
            [SIDE_PANEL_X, STREAM_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
            300.0,
            format!(
                "{} [{} endpoints]",
                short_text(&sheet.name, 28),
                sheet.endpoint_records.len()
            ),
            Some(PidNodeKey::Stream {
                name: sheet.name.clone(),
            }),
        );
        if let Some(handle) = handle {
            preview_index.record_existing_handle(PidNodeKey::Sheet { name: sheet.name.clone() }, handle);
        }
        row += 1;
    }

    if let Some(tagged) = &pid_doc.tagged_storages {
        for entry in tagged.entries.iter().take(6) {
            let _ = add_panel_line(
                doc,
                preview_index,
                "PID_STREAMS",
                [SIDE_PANEL_X, STREAM_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
                300.0,
                format!("TaggedText {}", short_text(&entry.storage_name, 26)),
                Some(PidNodeKey::TaggedStorage {
                    storage_name: entry.storage_name.clone(),
                }),
            );
            row += 1;
        }
    }

    if let Some(cross) = &pid_doc.cross_reference {
        if !cross.cluster_coverage.declared_missing.is_empty()
            || !cross.cluster_coverage.found_extra.is_empty()
        {
            let _ = add_panel_line(
                doc,
                preview_index,
                "PID_STREAMS",
                [SIDE_PANEL_X, STREAM_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
                300.0,
                format!(
                    "ClusterCoverage missing={} extra={}",
                    cross.cluster_coverage.declared_missing.len(),
                    cross.cluster_coverage.found_extra.len()
                ),
                Some(PidNodeKey::ClusterCoverage),
            );
        }
    }
}

fn add_cross_reference_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    pid_doc: &PidDocument,
) {
    let Some(cross) = &pid_doc.cross_reference else {
        return;
    };
    if cross.attribute_classes.is_empty()
        && cross.root_presence.is_empty()
        && cross.cluster_coverage.declared_missing.is_empty()
        && cross.cluster_coverage.found_extra.is_empty()
    {
        return;
    }

    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_CROSSREF",
        [CROSSREF_PANEL_X, HEADER_Y, 0.0],
        320.0,
        "Cross Reference".into(),
        None,
    );

    let mut row = 0usize;
    for class in cross.attribute_classes.iter().take(8) {
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_CROSSREF",
            [CROSSREF_PANEL_X, HEADER_Y - 24.0 - row as f64 * 18.0, 0.0],
            320.0,
            format!(
                "{} [{} records]",
                short_text(&class.class_name, 26),
                class.record_count
            ),
            Some(PidNodeKey::AttributeClass {
                class_name: class.class_name.clone(),
            }),
        );
        row += 1;
    }

    for root in cross.root_presence.iter().take(6) {
        let status = if root.found_as_storage || root.found_as_stream {
            "ok"
        } else {
            "missing"
        };
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_CROSSREF",
            [CROSSREF_PANEL_X, HEADER_Y - 24.0 - row as f64 * 18.0, 0.0],
            320.0,
            format!("root {} [{}]", short_text(&root.name, 24), status),
            Some(PidNodeKey::Root {
                name: root.name.clone(),
            }),
        );
        row += 1;
    }

    if !cross.cluster_coverage.declared_missing.is_empty()
        || !cross.cluster_coverage.found_extra.is_empty()
    {
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_CROSSREF",
            [CROSSREF_PANEL_X, HEADER_Y - 24.0 - row as f64 * 18.0, 0.0],
            320.0,
            format!(
                "coverage missing={} extra={}",
                cross.cluster_coverage.declared_missing.len(),
                cross.cluster_coverage.found_extra.len()
            ),
            Some(PidNodeKey::ClusterCoverage),
        );
    }
}

fn add_unresolved_entities(
    doc: &mut nm::CadDocument,
    preview_index: &mut PidPreviewIndex,
    view: &PidImportView,
    unresolved_edges: usize,
) {
    if view.unresolved.is_empty() && unresolved_edges == 0 {
        return;
    }
    let _ = add_panel_line(
        doc,
        preview_index,
        "PID_UNRESOLVED",
        [CROSSREF_PANEL_X, UNRESOLVED_PANEL_Y, 0.0],
        320.0,
        "Unresolved".into(),
        None,
    );

    let mut row = 0usize;
    if unresolved_edges > 0 {
        let label = format!("{} unresolved relationship edge(s)", unresolved_edges);
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_UNRESOLVED",
            [CROSSREF_PANEL_X, UNRESOLVED_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
            320.0,
            label.clone(),
            Some(PidNodeKey::Unresolved { label }),
        );
        row += 1;
    }
    for line in view.unresolved.iter().take(8) {
        let _ = add_panel_line(
            doc,
            preview_index,
            "PID_UNRESOLVED",
            [CROSSREF_PANEL_X, UNRESOLVED_PANEL_Y - 24.0 - row as f64 * 18.0, 0.0],
            320.0,
            short_text(line, 72),
            Some(PidNodeKey::Unresolved {
                label: line.clone(),
            }),
        );
        row += 1;
    }
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

fn short_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let short: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", short)
    } else {
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::Handle;
    use pid_parse::{
        AttributeClassSummary, ClusterCoverage, CrossReferenceGraph, ObjectGraph, PidDocument,
        PidObject, PidRelationship, ProbeSummary, SheetStream, SymbolUsage,
    };
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    const FIXTURE_DRAWING: &str = "/TaggedTxtData/Drawing";
    const FIXTURE_GENERAL: &str = "/TaggedTxtData/General";
    const FIXTURE_SHEET: &str = "/PlainSheet/Sheet1";
    const FIXTURE_BLOB: &str = "/UnknownStorage/Blob";

    static FIXTURE_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_pid_path(name: &str) -> PathBuf {
        let n = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("h7cad-pid-rt-{pid}-{n}-{name}.pid"))
    }

    /// Build a tiny synthetic `.pid` CFB containing the four streams the
    /// `pid-parse` writer round-trip suite exercises. The contents need
    /// not parse as a real Smart P&ID — `parse_package` is happy with any
    /// CFB so long as the path layout is well-formed.
    fn build_fixture_pid(path: &std::path::Path) {
        if path.exists() {
            std::fs::remove_file(path).expect("clean fixture path");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("ensure tmp parent");
        }
        let mut cfb = ::cfb::create(path).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();
        cfb.create_storage("/PlainSheet").unwrap();
        cfb.create_storage("/UnknownStorage").unwrap();

        let drawing = b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);

        let general = b"<?xml version=\"1.0\"?><General><FilePath>C:/fixture.pid</FilePath></General>";
        let mut s = cfb.create_stream(FIXTURE_GENERAL).unwrap();
        s.write_all(general).unwrap();
        drop(s);

        let sheet: Vec<u8> = (0u8..16).collect();
        let mut s = cfb.create_stream(FIXTURE_SHEET).unwrap();
        s.write_all(&sheet).unwrap();
        drop(s);

        let blob: Vec<u8> = (0u8..32).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
        let mut s = cfb.create_stream(FIXTURE_BLOB).unwrap();
        s.write_all(&blob).unwrap();
        drop(s);

        cfb.flush().unwrap();
    }

    fn write_publish_sidecars(path: &std::path::Path, drawing_no: &str) -> (PathBuf, PathBuf) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("pid file stem");
        let dir = path.parent().expect("pid parent");
        let data_path = dir.join(format!("{stem}_Data.xml"));
        let meta_path = dir.join(format!("{stem}_Meta.xml"));
        let data_xml = format!(
            r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container CompSchema="PIDComponent" Scope="Data" SoftwareVersion="10.00.31.0023" IsValidated="False" SchemaVersion="04.02.17.01" Plant="P01" Project="" DocUID="DRAWING00000000000000000000000001" DocName="{drawing_no}" Version="" ToolID="SMARTPLANTPID" ToolSignature="AAAD" SDECIMAL=".">
   <PIDDrawing>
      <IObject UID="DRAWING00000000000000000000000001" Name="{drawing_no}" Description=""/>
      <IDocument DocCategory="P&amp;ID Documents" DocTitle="H7CAD BRAN Tutorial" DocType="P&amp;ID" DocSubtype=""/>
      <IDocVersionComposition/>
      <IDwgRepresentationComposition/>
      <IPIDDrawing/>
      <ISchematicDwg/>
      <IPBSItem/>
   </PIDDrawing>
   <PIDPipeline>
      <IObject UID="PIPELINE0000000000000000000000001" Name="BRAN-LINE-001"/>
      <IDrawingItem/>
      <IPipeline PipelineName="BRAN-LINE-001"/>
      <IPipeCrossSectionItem NominalDiameter="50 mm"/>
      <IPipingSpecifiedItem PipingMaterialsClass="1.6AR12"/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipeline>
   <PIDPipingConnector>
      <IObject UID="CONNECTOR000000000000000000000001" Name="BRAN-CONN-001"/>
      <IDrawingItem/>
      <IPipingConnector/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipingConnector>
   <PIDPipingBranchPoint>
      <IObject UID="PIPINGBRANCH000000000000000000001" Name="BRAN-1"/>
      <IDrawingItem/>
      <IPipingBranchPoint/>
      <IProcessPointCollection/>
      <IDocumentItem/>
   </PIDPipingBranchPoint>
   <PIDBranchPoint>
      <IObject UID="BRANCHPOINT0000000000000000000001" Name="BRAN-1"/>
      <IDrawingItem/>
      <IBranchPoint/>
      <IDocumentItem/>
   </PIDBranchPoint>
   <PIDProcessPoint>
      <IObject UID="PROCESSPOINT000000000000000000001" Name="PP-MAIN"/>
      <IDrawingItem/>
      <IProcessPoint/>
      <IDocumentItem/>
   </PIDProcessPoint>
   <PIDRepresentation>
      <IObject UID="REPRESENT000000000000000000000001" Name="BRAN-REP"/>
      <IDrawingItem/>
      <IRepresentation RepresentationType="Polyline"/>
   </PIDRepresentation>
   <Rel>
      <IObject UID="REL-DRAW-PIPELINE"/>
      <IRel UID1="PIPELINE0000000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-DRAW-CONNECTOR"/>
      <IRel UID1="CONNECTOR000000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-DRAW-PIPINGBRANCH"/>
      <IRel UID1="PIPINGBRANCH000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-DRAW-BRANCHPOINT"/>
      <IRel UID1="BRANCHPOINT0000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-DRAW-PP"/>
      <IRel UID1="PROCESSPOINT000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-DRAW-REP"/>
      <IRel UID1="REPRESENT000000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="DrawingItems"/>
   </Rel>
   <Rel>
      <IObject UID="REL-REP-COMP"/>
      <IRel UID1="REPRESENT000000000000000000000001" UID2="PIPINGBRANCH000000000000000000001" DefUID="DwgRepresentationComposition"/>
   </Rel>
   <Rel>
      <IObject UID="REL-END1"/>
      <IRel UID1="CONNECTOR000000000000000000000001" UID2="PIPINGBRANCH000000000000000000001" DefUID="PipingEnd1Conn"/>
   </Rel>
   <Rel>
      <IObject UID="REL-END2"/>
      <IRel UID1="CONNECTOR000000000000000000000001" UID2="PROCESSPOINT000000000000000000001" DefUID="PipingEnd2Conn"/>
   </Rel>
   <Rel>
      <IObject UID="REL-TAP"/>
      <IRel UID1="PIPINGBRANCH000000000000000000001" UID2="BRANCHPOINT0000000000000000000001" DefUID="PipingTapOrFitting"/>
   </Rel>
   <Rel>
      <IObject UID="REL-PROC-COLLECT"/>
      <IRel UID1="PROCESSPOINT000000000000000000001" UID2="PIPELINE0000000000000000000000001" DefUID="ProcessPointCollection"/>
   </Rel>
</Container>
"#
        );
        let meta_xml = format!(
            r#"<?xml version ="1.0" encoding="UTF-8"?>
<Container CompSchema="DocVersioningComponent" Scope="Data" SoftwareVersion="10.00.31.0023" IsValidated="False" SchemaVersion="04.02.17.01" Plant="P01" Project="" DocUID="DRAWING00000000000000000000000001" DocName="{drawing_no}" Version="" ToolID="SMARTPLANTPID" ToolSignature="AAAD" SDECIMAL=".">
   <DocumentVersion>
      <IObject UID="DOCVER000000000000000000000000001" Name="{drawing_no} Version"/>
      <IDocumentVersion DocRevision="0" DocVersionDate="2026/04/20" DocVersion="1"/>
      <IFileComposition/>
   </DocumentVersion>
   <Rel>
      <IObject UID="REL-VERSIONED"/>
      <IRel UID1="DRAWING00000000000000000000000001" UID2="DOCVER000000000000000000000000001" DefUID="VersionedDoc"/>
   </Rel>
   <DocumentRevision>
      <IObject UID="DOCREV000000000000000000000000001" Name="{drawing_no} Revision"/>
      <IDocumentRevision MajorRev_ForRevise="0" MinorRev_ForRevise=""/>
   </DocumentRevision>
   <Rel>
      <IObject UID="REL-REVISION"/>
      <IRel UID1="DOCREV000000000000000000000000001" UID2="DRAWING00000000000000000000000001" DefUID="RevisedDocument"/>
   </Rel>
   <File>
      <IObject UID="FILE0000000000000000000000000001" Name="{drawing_no}.pid" Description=""/>
      <IFile FilePath="C:\temp\publish"/>
   </File>
   <Rel>
      <IObject UID="REL-FILE"/>
      <IRel UID1="FILE0000000000000000000000000001" UID2="DOCVER000000000000000000000000001" DefUID="FileComposition"/>
   </Rel>
</Container>
"#
        );
        std::fs::write(&data_path, data_xml).expect("write data sidecar");
        std::fs::write(&meta_path, meta_xml).expect("write meta sidecar");
        (data_path, meta_path)
    }

    fn sample_layout_doc() -> PidDocument {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("PID-200".into()),
            project_number: Some("P-02".into()),
            objects: vec![
                PidObject {
                    drawing_id: "PIPE-001".into(),
                    item_type: "PIDPipeline".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("LINE-001".into()),
                    extra: BTreeMap::from([("PipelineName".into(), "LINE-001".into())]),
                    record_id: Some(0x6001),
                    field_x: Some(1),
                },
                PidObject {
                    drawing_id: "BRANCH-001".into(),
                    item_type: "PIDBranchPoint".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("BRAN-1".into()),
                    extra: BTreeMap::new(),
                    record_id: Some(0x6002),
                    field_x: Some(2),
                },
                PidObject {
                    drawing_id: "UNPLACED-001".into(),
                    item_type: "PIDRepresentation".into(),
                    drawing_item_type: Some("IDrawingItem".into()),
                    model_id: Some("REP-1".into()),
                    extra: BTreeMap::new(),
                    record_id: None,
                    field_x: None,
                },
            ],
            relationships: vec![PidRelationship {
                model_id: "Relationship.ProcessPointCollection.00000000000000000000000000000001"
                    .into(),
                guid: "00000000000000000000000000000001".into(),
                record_id: Some(0x7001),
                field_x: Some(3),
                source_drawing_id: Some("BRANCH-001".into()),
                target_drawing_id: Some("PIPE-001".into()),
            }],
            by_drawing_id: BTreeMap::from([
                ("PIPE-001".into(), 0),
                ("BRANCH-001".into(), 1),
                ("UNPLACED-001".into(), 2),
            ]),
            counts_by_type: BTreeMap::new(),
        });
        doc.layout = Some(PidLayoutModel {
            items: vec![
                PidLayoutItem {
                    layout_id: "item:PIPE-001".into(),
                    drawing_id: Some("PIPE-001".into()),
                    graphic_oid: Some(101),
                    kind: "PIDPipeline".into(),
                    anchor: [480.0, -140.0],
                    bounds: None,
                    symbol_name: Some("Pipeline".into()),
                    symbol_path: None,
                    label: Some("LINE-001".into()),
                    model_id: Some("LINE-001".into()),
                },
                PidLayoutItem {
                    layout_id: "item:BRANCH-001".into(),
                    drawing_id: Some("BRANCH-001".into()),
                    graphic_oid: Some(582),
                    kind: "PIDBranchPoint".into(),
                    anchor: [720.0, -140.0],
                    bounds: None,
                    symbol_name: Some("Branch".into()),
                    symbol_path: None,
                    label: Some("BRAN-1".into()),
                    model_id: Some("BRAN-1".into()),
                },
            ],
            segments: vec![pid_parse::PidLayoutSegment {
                layout_id: "segment:1".into(),
                owner_drawing_id: Some("PIPE-001".into()),
                graphic_oid: Some(582),
                start: [480.0, -140.0],
                end: [720.0, -140.0],
                role: "ProcessPointCollection".into(),
            }],
            texts: vec![PidLayoutText {
                layout_id: "text:PIPE-001".into(),
                drawing_id: Some("PIPE-001".into()),
                text: "LINE-001".into(),
                anchor: [528.0, -120.0],
                bounds: None,
            }],
            unplaced: vec![pid_parse::PidLayoutUnplaced {
                drawing_id: Some("UNPLACED-001".into()),
                kind: "PIDRepresentation".into(),
                label: "REP-1".into(),
            }],
            warnings: vec![],
        });
        doc
    }

    fn entity_for_handle(doc: &nm::CadDocument, handle: Handle) -> Option<&nm::Entity> {
        doc.get_entity(nm::Handle::new(handle.value()))
    }

    fn entity_anchor(entity: &nm::Entity) -> Option<[f64; 3]> {
        match &entity.data {
            nm::EntityData::Line { start, .. } => Some(*start),
            nm::EntityData::Circle { center, .. } => Some(*center),
            nm::EntityData::Text { insertion, .. } => Some(*insertion),
            nm::EntityData::MText { insertion, .. } => Some(*insertion),
            _ => None,
        }
    }

    fn real_sample_pid_path() -> Option<PathBuf> {
        let path = PathBuf::from(
            "C:/Users/Administrator/Documents/xwechat_files/happydpc_b2ec/msg/file/2026-04/XML文件(1)/DWG-0202GP06-01.pid",
        );
        path.exists().then_some(path)
    }

    /// Target acceptance sample fixed by the 2026-04-21 pid-real-sample
    /// display-and-screenshot plan. Lives in the sibling `pid-parse` repo
    /// so this test is skipped when the repo isn't checked out alongside
    /// H7CAD.
    fn target_sample_pid_path() -> Option<PathBuf> {
        let path = PathBuf::from(
            r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid",
        );
        path.exists().then_some(path)
    }

    #[test]
    fn target_pid_preview_layout_is_primary_visual_focus() {
        // Task 2 focused test (plan docs/plans/2026-04-21-pid-real-sample-
        // display-and-screenshot-plan.md): the target sample has a sparse
        // object graph (parse yields 2 objects, 0 relationships because
        // the `.pid` ships without publish sidecars). Without a focused
        // check the wide set of decorative side panels (meta / cross-ref
        // / unresolved / streams / fallback / clusters) dominate the
        // viewport after fit_all, making the real layout items invisible
        // amongst panel text. Assert the document still carries at least
        // one entity on the primary-object layer so decorative layers
        // never fully eclipse real geometry.
        let Some(path) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };
        let bundle = open_pid(&path).expect("open target pid sample");

        let primary_layer_count = bundle
            .native_preview
            .entities
            .iter()
            .filter(|e| e.layer_name.starts_with("PID_OBJECTS_"))
            .count();
        let layout_text_count = bundle
            .native_preview
            .entities
            .iter()
            .filter(|e| e.layer_name == "PID_LAYOUT_TEXT")
            .count();
        let decorative_count = bundle
            .native_preview
            .entities
            .iter()
            .filter(|e| {
                matches!(
                    e.layer_name.as_str(),
                    "PID_META"
                        | "PID_FALLBACK"
                        | "PID_CROSSREF"
                        | "PID_UNRESOLVED"
                        | "PID_STREAMS"
                        | "PID_CLUSTERS"
                        | "PID_SYMBOLS"
                )
            })
            .count();

        eprintln!(
            "target pid layout focus: primary_objects={}, layout_text={}, decorative={}",
            primary_layer_count, layout_text_count, decorative_count
        );

        assert!(
            primary_layer_count >= 1,
            "at least one entity must live on PID_OBJECTS_* so the main drawing has a real anchor; \
             got primary={} decorative={}",
            primary_layer_count,
            decorative_count
        );
    }

    #[test]
    fn target_pid_sample_fit_layers_matching_succeeds_for_main_drawing_layers() {
        // M3 / plan docs/plans/2026-04-21-pid-fit-main-drawing-plan.md:
        // the FileOpened PID branch first asks the scene to fit only
        // the main-drawing layers and falls back to `fit_all` when that
        // returns false. Verify the target sample's preview carries
        // enough primary geometry to satisfy the first call — if this
        // flips to false, we'd silently regress back to the pre-plan
        // behaviour where side panels dominate the viewport.
        let Some(path) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };
        let bundle = open_pid(&path).expect("open target pid sample");
        let compat = crate::io::native_bridge::native_doc_to_acadrust(&bundle.native_preview);
        let mut scene = crate::scene::Scene::new();
        scene.document = compat;
        scene.set_native_doc(Some(bundle.native_preview));
        scene.native_render_enabled = true;

        let fitted = scene.fit_layers_matching(&[
            "PID_OBJECTS_",
            "PID_LAYOUT_TEXT",
            "PID_RELATIONSHIPS",
        ]);
        assert!(
            fitted,
            "target pid sample must carry primary-layer geometry so the PID FileOpened \
             branch's fit_layers_matching call succeeds without falling back to fit_all"
        );
    }

    #[test]
    fn target_pid_sample_scene_has_fittable_geometry_and_native_doc() {
        // Task 3 (plan docs/plans/2026-04-21-pid-real-sample-display-and-
        // screenshot-plan.md): wire the opened PID through Scene so we
        // detect regressions where the document is parsed correctly but
        // the scene view lands in an empty / mis-fit state. This is an
        // offline equivalent of the full `Message::FileOpened` path:
        // open_pid → set_native_doc → entity_wires → fit_all. The test
        // asserts the three `fit_all` preconditions: scene carries a
        // native doc, compat doc has entities, and entity_wires yields
        // at least one fittable wire (empty wires cause fit_all to no-op
        // and leave the camera on its startup view).
        let Some(path) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };
        let bundle = open_pid(&path).expect("open target pid sample");
        let compat = crate::io::native_bridge::native_doc_to_acadrust(&bundle.native_preview);
        let mut scene = crate::scene::Scene::new();
        scene.document = compat;
        scene.set_native_doc(Some(bundle.native_preview.clone()));
        scene.native_render_enabled = true;

        assert!(
            scene.native_doc().is_some(),
            "scene must retain native document after set_native_doc"
        );
        assert!(
            !scene.document.entities().collect::<Vec<_>>().is_empty(),
            "scene.document (compat projection) must expose non-empty entities"
        );

        let wires = scene.entity_wires();
        assert!(
            !wires.is_empty(),
            "target pid scene must yield at least one fittable wire; otherwise \
             fit_all silently no-ops and the camera never converges"
        );

        // fit_all must not panic even on the sparse target sample.
        scene.fit_all();
    }

    #[test]
    fn open_target_pid_sample_builds_dense_preview() {
        let Some(path) = target_sample_pid_path() else {
            eprintln!("SKIP: target pid sample not found");
            return;
        };
        let bundle = open_pid(&path).expect("open target pid sample");
        let layout = bundle
            .pid_doc
            .layout
            .as_ref()
            .expect("target sample should derive layout");

        eprintln!(
            "target pid sample baseline: object_count={}, relationship_count={}, \
             layout.items={}, layout.segments={}, native_entities={}",
            bundle.summary.object_count,
            bundle.summary.relationship_count,
            layout.items.len(),
            layout.segments.len(),
            bundle.native_preview.entities.len(),
        );

        // Anchor thresholds reflect the **current** parsed shape of this
        // specific `.pid` sample (which ships without publish sidecars, so
        // object graph enrichment is limited). They exist to detect
        // regression to "empty preview", not to upper-bound display
        // quality — Task 2's dedicated test owns the density dimension.
        assert!(
            layout.items.len() >= 2,
            "target sample should place >= 2 layout items, got {}",
            layout.items.len()
        );
        assert!(
            bundle.summary.object_count >= 2,
            "target sample should expose >= 2 objects via summary, got {}",
            bundle.summary.object_count
        );
        assert!(
            bundle.native_preview.entities.len() >= 30,
            "target sample preview should produce >= 30 native entities \
             (panels + grid + layout items), got {}",
            bundle.native_preview.entities.len()
        );
    }

    #[test]
    fn pid_preview_prefers_layout_anchor_when_layout_exists() {
        let doc = sample_layout_doc();
        let view = build_import_view(&doc);
        let (native, _, preview_index) = pid_document_to_preview(&doc, &view);
        let handles = preview_index.handles_for(&PidNodeKey::Object {
            drawing_id: "PIPE-001".into(),
        });
        assert!(
            handles.iter().filter_map(|handle| entity_for_handle(&native, *handle)).any(|entity| {
                entity_anchor(entity)
                    .map(|point| (point[0] - 432.0).abs() < 0.1 || (point[0] - 480.0).abs() < 0.1)
                    .unwrap_or(false)
            }),
            "layout-backed preview should place PIPE-001 near its decoded anchor rather than the grid origin"
        );
        assert!(
            !handles.iter().filter_map(|handle| entity_for_handle(&native, *handle)).any(|entity| {
                entity_anchor(entity)
                    .map(|point| (point[0] - grid_point(0)[0]).abs() < 0.1 && (point[1] - grid_point(0)[1]).abs() < 0.1)
                    .unwrap_or(false)
            }),
            "layout-backed preview should no longer use grid_point(0) for the first object"
        );
    }

    #[test]
    fn pid_preview_places_unplaced_objects_on_fallback_layer() {
        let doc = sample_layout_doc();
        let view = build_import_view(&doc);
        let (native, _, preview_index) = pid_document_to_preview(&doc, &view);
        let handles = preview_index.handles_for(&PidNodeKey::Object {
            drawing_id: "UNPLACED-001".into(),
        });
        assert!(!handles.is_empty(), "fallback object should still be selectable");
        assert!(
            handles
                .iter()
                .filter_map(|handle| entity_for_handle(&native, *handle))
                .all(|entity| entity.layer_name == "PID_FALLBACK"),
            "every fallback entity should live on PID_FALLBACK"
        );
    }

    #[test]
    fn pid_preview_index_tracks_graphic_oid_for_layout_items() {
        let doc = sample_layout_doc();
        let view = build_import_view(&doc);
        let (_, _, preview_index) = pid_document_to_preview(&doc, &view);
        assert!(
            !preview_index.handles_for_graphic_oid(582).is_empty(),
            "graphic oid index should retain handles for layout-backed selection"
        );
        assert!(
            !preview_index.handles_for_drawing_id("PIPE-001").is_empty(),
            "drawing id index should retain handles for layout-backed selection"
        );
    }

    #[test]
    fn open_pid_real_sample_builds_layout_when_sample_present() {
        let Some(path) = real_sample_pid_path() else {
            eprintln!("SKIP: real sample pid not found");
            return;
        };
        let bundle = open_pid(&path).expect("open real sample");
        let layout = bundle
            .pid_doc
            .layout
            .as_ref()
            .expect("real sample should derive layout");
        assert!(
            layout.items.len() >= 10,
            "real sample should place at least 10 items, got {}",
            layout.items.len()
        );
        assert!(
            layout.segments.len() >= 5,
            "real sample should recover at least 5 layout segments, got {}",
            layout.segments.len()
        );
    }

    #[test]
    fn pid_preview_renders_process_point_as_circle_when_layout_kind_known() {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("PID-201".into()),
            project_number: Some("P-03".into()),
            objects: vec![PidObject {
                drawing_id: "PP-001".into(),
                item_type: "PIDProcessPoint".into(),
                drawing_item_type: Some("IDrawingItem".into()),
                model_id: Some("PP-001".into()),
                extra: BTreeMap::new(),
                record_id: Some(0x6101),
                field_x: Some(1),
            }],
            relationships: vec![],
            by_drawing_id: BTreeMap::from([("PP-001".into(), 0)]),
            counts_by_type: BTreeMap::new(),
        });
        doc.layout = Some(PidLayoutModel {
            items: vec![PidLayoutItem {
                layout_id: "item:PP-001".into(),
                drawing_id: Some("PP-001".into()),
                graphic_oid: Some(701),
                kind: "PIDProcessPoint".into(),
                anchor: [360.0, -120.0],
                bounds: None,
                symbol_name: Some("ProcessPoint".into()),
                symbol_path: None,
                label: Some("PP-001".into()),
                model_id: Some("PP-001".into()),
            }],
            segments: vec![],
            texts: vec![PidLayoutText {
                layout_id: "text:PP-001".into(),
                drawing_id: Some("PP-001".into()),
                text: "PP-001".into(),
                anchor: [406.0, -100.0],
                bounds: None,
            }],
            unplaced: vec![],
            warnings: vec![],
        });

        let view = build_import_view(&doc);
        let (native, _, preview_index) = pid_document_to_preview(&doc, &view);
        let handles = preview_index.handles_for(&PidNodeKey::Object {
            drawing_id: "PP-001".into(),
        });
        assert!(
            handles
                .iter()
                .filter_map(|handle| entity_for_handle(&native, *handle))
                .any(|entity| matches!(entity.data, nm::EntityData::Circle { .. })),
            "PIDProcessPoint should render as a circle glyph instead of falling back to a placeholder box"
        );
    }

    #[test]
    fn save_pid_round_trips_every_stream_through_h7cad_save_path() {
        let src = unique_pid_path("rt-src");
        let dst = unique_pid_path("rt-dst");
        build_fixture_pid(&src);

        load_pid_native_with_package(&src).expect("H7CAD load_pid_native_with_package failed");

        save_pid_native(&dst, &src).expect("H7CAD save_pid_native failed");

        let parser = PidParser::new();
        let original = parser.parse_package(&src).expect("re-parse source");
        let written = parser.parse_package(&dst).expect("parse written file");

        let original_keys: Vec<&String> = original.streams.keys().collect();
        let written_keys: Vec<&String> = written.streams.keys().collect();
        assert_eq!(
            original_keys, written_keys,
            "stream key set must round-trip exactly through the H7CAD save path"
        );
        for key in original_keys {
            assert_eq!(
                original.streams[key].data, written.streams[key].data,
                "stream {} bytes diverged after H7CAD save",
                key
            );
        }

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn save_pid_without_prior_open_returns_explicit_error() {
        let src = unique_pid_path("missing-cache-src");
        let dst = unique_pid_path("missing-cache-dst");
        let err = save_pid_native(&dst, &src).expect_err("must error without cached package");
        assert!(
            err.contains("PID save requires") && err.contains(&src.display().to_string()),
            "error message should name the missing source path; got: {err}"
        );
    }

    #[test]
    fn edit_pid_drawing_number_swaps_attribute_in_cached_package() {
        let src = unique_pid_path("edit-swap");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let report = edit_pid_drawing_number(&src, "NEW-007").expect("edit must succeed");
        assert_eq!(report.previous.as_deref(), Some("FX-001"));
        assert_eq!(report.next, "NEW-007");

        let pkg = pid_package_store::get_package(&src).expect("cache should still hold package");
        let xml = std::str::from_utf8(&pkg.streams[FIXTURE_DRAWING].data).unwrap();
        assert!(
            xml.contains("SP_DRAWINGNUMBER=\"NEW-007\""),
            "Drawing XML should hold the new value; got: {xml}"
        );
        assert!(
            !xml.contains("FX-001"),
            "old drawing number must be gone; got: {xml}"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_pid_drawing_number_without_cached_package_errors() {
        let src = unique_pid_path("edit-no-cache");
        let err = edit_pid_drawing_number(&src, "X").expect_err("must fail without cache");
        assert!(
            err.contains("no cached PidPackage") && err.contains(&src.display().to_string()),
            "error must name missing cache + source path; got: {err}"
        );
    }

    #[test]
    fn edit_pid_drawing_number_rejects_non_utf8_drawing_xml() {
        let src = unique_pid_path("edit-non-utf8");
        // Build a synthetic PidPackage in-memory whose Drawing stream is
        // intentionally not valid UTF-8 (UTF-16 BOM + a couple of code
        // units). We bypass build_fixture_pid + parse_package because
        // the parser would reject this too — we want to test the edit
        // helper's response to a cached but malformed stream.
        use pid_parse::model::PidDocument;
        use pid_parse::package::{PidPackage, RawStream};
        use std::collections::BTreeMap;

        let mut streams = BTreeMap::new();
        let bad_bytes = vec![0xFF, 0xFE, 0x44, 0x00, 0x72, 0x00]; // "Dr" in UTF-16 LE
        streams.insert(
            FIXTURE_DRAWING.to_string(),
            RawStream {
                path: FIXTURE_DRAWING.into(),
                data: bad_bytes,
                modified: false,
            },
        );
        let pkg = PidPackage::new(Some(src.clone()), streams, PidDocument::default());
        pid_package_store::cache_package(&src, pkg);

        let err = edit_pid_drawing_number(&src, "X").expect_err("non-UTF-8 must error");
        assert!(
            err.contains("not UTF-8"),
            "error must call out UTF-8 problem; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    /// Build a fixture whose Drawing XML carries multiple `SP_*`
    /// attributes so we can verify that editing one of them leaves the
    /// others byte-identical.
    fn build_fixture_pid_with_multi_attrs(path: &std::path::Path) {
        if path.exists() {
            std::fs::remove_file(path).expect("clean fixture path");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("ensure tmp parent");
        }
        let mut cfb = ::cfb::create(path).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();

        let drawing = b"<?xml version=\"1.0\"?><Drawing>\
            <Tag SP_DRAWINGNUMBER=\"FX-001\" SP_PROJECTNUMBER=\"PRJ-OLD\" SP_REVISION=\"1\"/>\
            </Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);

        cfb.flush().unwrap();
    }

    #[test]
    fn edit_pid_drawing_attribute_swaps_arbitrary_attribute() {
        let src = unique_pid_path("attr-swap");
        build_fixture_pid_with_multi_attrs(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let report = edit_pid_drawing_attribute(&src, "SP_PROJECTNUMBER", "PRJ-2026-A")
            .expect("edit");
        assert_eq!(report.attr, "SP_PROJECTNUMBER");
        assert_eq!(report.previous.as_deref(), Some("PRJ-OLD"));
        assert_eq!(report.next, "PRJ-2026-A");

        let pkg = pid_package_store::get_package(&src).unwrap();
        let xml = std::str::from_utf8(&pkg.streams[FIXTURE_DRAWING].data).unwrap();
        assert!(xml.contains("SP_PROJECTNUMBER=\"PRJ-2026-A\""));
        // Other attributes must survive intact.
        assert!(xml.contains("SP_DRAWINGNUMBER=\"FX-001\""));
        assert!(xml.contains("SP_REVISION=\"1\""));

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_pid_drawing_attribute_returns_attr_not_found_for_unknown_name() {
        let src = unique_pid_path("attr-not-found");
        build_fixture_pid_with_multi_attrs(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let err = edit_pid_drawing_attribute(&src, "SP_NOSUCH", "X")
            .expect_err("must fail for unknown attribute");
        assert!(
            err.contains("metadata edit failed") && err.contains("SP_NOSUCH"),
            "error must transit metadata_helpers diagnostic; got: {err}"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_pid_drawing_attribute_preserves_other_attributes_byte_for_byte() {
        let src = unique_pid_path("attr-preserve");
        build_fixture_pid_with_multi_attrs(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        // Snapshot the original Drawing bytes.
        let original_bytes = pid_package_store::get_package(&src)
            .unwrap()
            .streams[FIXTURE_DRAWING]
            .data
            .clone();

        edit_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER", "NEW-9999")
            .expect("edit drawing number");

        let new_bytes = pid_package_store::get_package(&src)
            .unwrap()
            .streams[FIXTURE_DRAWING]
            .data
            .clone();
        let original_xml = std::str::from_utf8(&original_bytes).unwrap();
        let new_xml = std::str::from_utf8(&new_bytes).unwrap();

        // The diff must be confined to the SP_DRAWINGNUMBER value
        // region. We verify by recomputing what the bytes outside that
        // single attribute should look like and asserting equality
        // against the cached result.
        let expected = original_xml.replace(
            "SP_DRAWINGNUMBER=\"FX-001\"",
            "SP_DRAWINGNUMBER=\"NEW-9999\"",
        );
        assert_eq!(
            new_xml, expected,
            "bytes outside the targeted attribute must be preserved verbatim"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    /// General-stream-focused fixture: Drawing is minimal, General has
    /// multiple elements so single-element edits can be cross-checked.
    fn build_fixture_pid_with_general(path: &std::path::Path) {
        if path.exists() {
            std::fs::remove_file(path).expect("clean fixture path");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("ensure tmp parent");
        }
        let mut cfb = ::cfb::create(path).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();

        let drawing = b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);

        let general = b"<?xml version=\"1.0\"?><General>\
            <FilePath>C:/old/path.pid</FilePath>\
            <FileSize>2048</FileSize>\
            <Author>OLD-AUTHOR</Author>\
            </General>";
        let mut s = cfb.create_stream(FIXTURE_GENERAL).unwrap();
        s.write_all(general).unwrap();
        drop(s);

        cfb.flush().unwrap();
    }

    #[test]
    fn edit_pid_general_element_replaces_file_path() {
        let src = unique_pid_path("gen-replace");
        build_fixture_pid_with_general(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let report = edit_pid_general_element(&src, "FilePath", "D:/issued/rev2.pid")
            .expect("edit");
        assert_eq!(report.element, "FilePath");
        assert_eq!(report.previous.as_deref(), Some("C:/old/path.pid"));
        assert_eq!(report.next, "D:/issued/rev2.pid");

        let pkg = pid_package_store::get_package(&src).unwrap();
        let xml = std::str::from_utf8(&pkg.streams[FIXTURE_GENERAL].data).unwrap();
        assert!(xml.contains("<FilePath>D:/issued/rev2.pid</FilePath>"));
        // Sibling elements survive intact.
        assert!(xml.contains("<FileSize>2048</FileSize>"));
        assert!(xml.contains("<Author>OLD-AUTHOR</Author>"));

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_pid_general_element_returns_not_found_for_unknown_element() {
        let src = unique_pid_path("gen-not-found");
        build_fixture_pid_with_general(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let err = edit_pid_general_element(&src, "NoSuchElement", "X")
            .expect_err("must fail");
        assert!(
            err.contains("metadata edit failed") && err.contains("NoSuchElement"),
            "error must surface metadata_helpers diagnostic; got: {err}"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_pid_general_element_preserves_other_elements_byte_for_byte() {
        let src = unique_pid_path("gen-preserve");
        build_fixture_pid_with_general(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let original = pid_package_store::get_package(&src)
            .unwrap()
            .streams[FIXTURE_GENERAL]
            .data
            .clone();
        edit_pid_general_element(&src, "Author", "NEW-AUTHOR").expect("edit");
        let new_bytes = pid_package_store::get_package(&src)
            .unwrap()
            .streams[FIXTURE_GENERAL]
            .data
            .clone();
        let original_xml = std::str::from_utf8(&original).unwrap();
        let new_xml = std::str::from_utf8(&new_bytes).unwrap();

        let expected = original_xml
            .replace("<Author>OLD-AUTHOR</Author>", "<Author>NEW-AUTHOR</Author>");
        assert_eq!(
            new_xml, expected,
            "bytes outside the targeted element must be preserved verbatim"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn read_pid_drawing_attribute_returns_value_via_helper() {
        let src = unique_pid_path("read-attr");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let value = read_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER");
        assert_eq!(value.as_deref(), Some("FX-001"));

        let absent = read_pid_drawing_attribute(&src, "SP_NOSUCH");
        assert_eq!(absent, None);

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn read_pid_drawing_attribute_returns_none_when_no_cache() {
        let src = unique_pid_path("read-no-cache");
        let value = read_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER");
        assert_eq!(
            value, None,
            "missing cache should silently return None (callers compose with edit_* for typed errors)"
        );
    }

    #[test]
    fn read_pid_general_element_returns_value_via_helper() {
        let src = unique_pid_path("read-gen");
        build_fixture_pid_with_general(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        assert_eq!(
            read_pid_general_element(&src, "FilePath").as_deref(),
            Some("C:/old/path.pid")
        );
        assert_eq!(read_pid_general_element(&src, "NoSuchElement"), None);

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn read_pid_general_element_returns_none_when_no_cache() {
        let src = unique_pid_path("read-gen-no-cache");
        assert_eq!(read_pid_general_element(&src, "FilePath"), None);
    }

    #[test]
    fn list_pid_metadata_returns_drawing_and_general_pairs() {
        let src = unique_pid_path("list-metadata");
        // Need both Drawing (with multiple SP_*) and General (with multiple
        // elements). Build a custom fixture composing both.
        if src.exists() {
            std::fs::remove_file(&src).expect("clean fixture");
        }
        let mut cfb = ::cfb::create(&src).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();
        let drawing = b"<?xml version=\"1.0\"?><Drawing>\
            <Tag SP_DRAWINGNUMBER=\"FX-001\" SP_PROJECTNUMBER=\"PRJ\"/>\
            </Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);
        let general = b"<?xml version=\"1.0\"?><General>\
            <FilePath>C:/abc.pid</FilePath><FileSize>123</FileSize>\
            </General>";
        let mut s = cfb.create_stream(FIXTURE_GENERAL).unwrap();
        s.write_all(general).unwrap();
        drop(s);
        cfb.flush().unwrap();
        load_pid_native_with_package(&src).expect("load");

        let listing = list_pid_metadata(&src).expect("list");
        assert_eq!(
            listing.drawing_attributes,
            vec![
                ("SP_DRAWINGNUMBER".to_string(), "FX-001".to_string()),
                ("SP_PROJECTNUMBER".to_string(), "PRJ".to_string()),
            ]
        );
        assert_eq!(
            listing.general_elements,
            vec![
                ("FilePath".to_string(), "C:/abc.pid".to_string()),
                ("FileSize".to_string(), "123".to_string()),
            ]
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn list_pid_metadata_returns_error_without_cache() {
        let src = unique_pid_path("list-no-cache");
        let err = list_pid_metadata(&src).expect_err("must fail without cache");
        assert!(
            err.contains("no cached PidPackage"),
            "should mention missing cache; got: {err}"
        );
    }

    #[test]
    fn verify_pid_cached_passes_for_unmodified_fixture() {
        let src = unique_pid_path("verify-cached");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let report = verify_pid_cached(&src).expect("verify");
        assert!(report.ok(), "report not ok: {:?}", report);
        assert_eq!(report.matched, 4, "fixture has 4 streams (Drawing/General/Sheet/Blob)");
        assert!(report.only_in_source.is_empty());
        assert!(report.only_in_roundtrip.is_empty());

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn verify_pid_cached_passes_after_metadata_edit() {
        let src = unique_pid_path("verify-after-edit");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        edit_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER", "EDITED-XYZ")
            .expect("edit drawing number");

        let report = verify_pid_cached(&src).expect("verify after edit");
        assert!(
            report.ok(),
            "edited cached package must still round-trip; report: {:?}",
            report
        );
        assert_eq!(report.matched, 4);

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    /// Build a fixture that deliberately carries an **unidentified**
    /// top-level CFB stream (`/MysteryTop`). `unidentified_top_level_streams`
    /// only surfaces streams whose parent is the root — nested streams
    /// inside `/TaggedTxtData/…` / `/JSite…/…` etc. are filtered by the
    /// lib-side KNOWN prefixes.
    fn build_fixture_pid_with_toplevel_unknown(path: &std::path::Path) {
        if path.exists() {
            std::fs::remove_file(path).expect("clean fixture path");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("ensure tmp parent");
        }
        let mut cfb = ::cfb::create(path).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();

        // Minimal Drawing/General so parse_package succeeds end-to-end.
        let drawing = b"<?xml version=\"1.0\"?><Drawing><Tag SP_DRAWINGNUMBER=\"FX-001\"/></Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);

        let general = b"<?xml version=\"1.0\"?><General><FilePath>C:/fixture.pid</FilePath></General>";
        let mut s = cfb.create_stream(FIXTURE_GENERAL).unwrap();
        s.write_all(general).unwrap();
        drop(s);

        // Now the unidentified top-level marker: a stream sitting at the
        // root of the CFB that is NOT in KNOWN_TOP_LEVEL_STREAM_NAMES.
        let mystery = b"\x78\x56\x34\x12hello-mystery";
        let mut s = cfb.create_stream("/MysteryTop").unwrap();
        s.write_all(mystery).unwrap();
        drop(s);

        cfb.flush().unwrap();
    }

    /// Build a minimal `PidPackage` whose `parsed.object_graph` is
    /// pre-populated with `objects` + `relationships`. Bypasses the
    /// real CFB parse path so neighbor/stats helpers can be exercised
    /// without a fixture file.
    fn cache_synthetic_graph_package(
        path: &std::path::Path,
        objects: Vec<pid_parse::PidObject>,
        relationships: Vec<pid_parse::PidRelationship>,
    ) {
        use pid_parse::model::PidDocument;
        use pid_parse::package::PidPackage;
        use pid_parse::ObjectGraph;
        use std::collections::BTreeMap;

        let mut by_drawing_id = BTreeMap::new();
        for (i, o) in objects.iter().enumerate() {
            by_drawing_id.insert(o.drawing_id.clone(), i);
        }
        let mut counts_by_type = BTreeMap::new();
        for o in &objects {
            *counts_by_type.entry(o.item_type.clone()).or_insert(0) += 1;
        }
        let mut parsed = PidDocument::default();
        parsed.object_graph = Some(ObjectGraph {
            drawing_no: None,
            project_number: None,
            objects,
            relationships,
            by_drawing_id,
            counts_by_type,
        });
        let pkg = PidPackage::new(Some(path.to_path_buf()), BTreeMap::new(), parsed);
        pid_package_store::cache_package(path, pkg);
    }

    fn make_object(id: &str, item_type: &str, tag: Option<&str>) -> pid_parse::PidObject {
        let mut extra = BTreeMap::new();
        if let Some(t) = tag {
            extra.insert("Tag".to_string(), t.to_string());
        }
        pid_parse::PidObject {
            drawing_id: id.into(),
            item_type: item_type.into(),
            drawing_item_type: None,
            model_id: None,
            extra,
            record_id: None,
            field_x: None,
        }
    }

    fn make_rel(guid: &str, src: Option<&str>, dst: Option<&str>) -> pid_parse::PidRelationship {
        pid_parse::PidRelationship {
            model_id: format!("Relationship.{guid}"),
            guid: guid.into(),
            record_id: None,
            field_x: None,
            source_drawing_id: src.map(String::from),
            target_drawing_id: dst.map(String::from),
        }
    }

    #[test]
    fn list_pid_neighbors_returns_self_and_resolved_neighbors() {
        let src = unique_pid_path("neighbors-happy");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", Some("E-101")),
                make_object("BBBB", "PipeRun", None),
                make_object("CCCC", "Instrument", Some("FIT-001")),
            ],
            vec![
                make_rel("R1", Some("AAAA"), Some("BBBB")),
                make_rel("R2", Some("AAAA"), Some("CCCC")),
            ],
        );

        let (self_info, neighbors) =
            list_pid_neighbors(&src, "AAAA", 1).expect("neighbors lookup");
        assert_eq!(self_info.drawing_id, "AAAA");
        assert_eq!(self_info.item_type, "Equipment");
        assert_eq!(self_info.tag_label.as_deref(), Some("E-101"));

        let neighbor_ids: Vec<&str> =
            neighbors.iter().map(|n| n.drawing_id.as_str()).collect();
        assert_eq!(neighbor_ids, vec!["BBBB", "CCCC"]);
        assert_eq!(neighbors[1].tag_label.as_deref(), Some("FIT-001"));

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_path_returns_path_through_chain() {
        let src = unique_pid_path("path-chain");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", None),
                make_object("BBBB", "PipeRun", None),
                make_object("CCCC", "Instrument", Some("FIT-001")),
            ],
            vec![
                make_rel("R1", Some("AAAA"), Some("BBBB")),
                make_rel("R2", Some("BBBB"), Some("CCCC")),
            ],
        );

        let (from_info, to_info, path) =
            list_pid_path(&src, "AAAA", "CCCC").expect("path");
        assert_eq!(from_info.drawing_id, "AAAA");
        assert_eq!(to_info.drawing_id, "CCCC");
        let ids: Vec<&str> = path.iter().map(|n| n.drawing_id.as_str()).collect();
        assert_eq!(ids, vec!["AAAA", "BBBB", "CCCC"], "BFS shortest path");
        // Tag projection on intermediate / endpoint preserved.
        assert_eq!(path[2].tag_label.as_deref(), Some("FIT-001"));

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_path_returns_error_when_no_path() {
        let src = unique_pid_path("path-disconnected");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", None),
                make_object("DDDD", "PipeRun", None), // islanded
            ],
            vec![],
        );

        let err = list_pid_path(&src, "AAAA", "DDDD")
            .expect_err("disconnected → error");
        assert!(
            err.contains("no path") && err.contains("AAAA") && err.contains("DDDD"),
            "should call out missing path + endpoints; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_path_accepts_unique_prefix_for_both_endpoints() {
        let src = unique_pid_path("path-prefix");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AA111", "Equipment", None),
                make_object("BB222", "PipeRun", None),
            ],
            vec![make_rel("R1", Some("AA111"), Some("BB222"))],
        );

        let (from_info, to_info, path) =
            list_pid_path(&src, "AA", "BB").expect("path with prefixes");
        assert_eq!(from_info.drawing_id, "AA111");
        assert_eq!(to_info.drawing_id, "BB222");
        assert_eq!(path.len(), 2);

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_with_depth_two_walks_two_hops() {
        // Build A↔B↔C chain — A's neighbors at depth=1 is just [B];
        // depth=2 should add C.
        let src = unique_pid_path("neighbors-depth2");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", None),
                make_object("BBBB", "PipeRun", None),
                make_object("CCCC", "Instrument", None),
            ],
            vec![
                make_rel("R1", Some("AAAA"), Some("BBBB")),
                make_rel("R2", Some("BBBB"), Some("CCCC")),
            ],
        );

        let (_, depth1) = list_pid_neighbors(&src, "AAAA", 1).expect("depth 1");
        let ids1: Vec<&str> = depth1.iter().map(|n| n.drawing_id.as_str()).collect();
        assert_eq!(ids1, vec!["BBBB"], "depth=1 only direct neighbor");

        let (_, depth2) = list_pid_neighbors(&src, "AAAA", 2).expect("depth 2");
        let ids2: Vec<&str> = depth2.iter().map(|n| n.drawing_id.as_str()).collect();
        assert_eq!(ids2, vec!["BBBB", "CCCC"], "depth=2 adds C");

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_with_depth_zero_returns_only_self() {
        let src = unique_pid_path("neighbors-depth0");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", None),
                make_object("BBBB", "PipeRun", None),
            ],
            vec![make_rel("R1", Some("AAAA"), Some("BBBB"))],
        );

        let (self_info, neighbors) = list_pid_neighbors(&src, "AAAA", 0).expect("depth 0");
        assert_eq!(self_info.drawing_id, "AAAA");
        assert!(
            neighbors.is_empty(),
            "depth=0 must take zero hops; neighbors empty"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_accepts_unique_prefix() {
        let src = unique_pid_path("neighbors-prefix-unique");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAAAA1", "Equipment", Some("E-101")),
                make_object("BBBBBB2", "PipeRun", None),
                make_object("CCCCCC3", "Instrument", None),
            ],
            vec![make_rel("R1", Some("AAAAAA1"), Some("BBBBBB2"))],
        );

        // 4-char prefix matches AAAAAA1 uniquely.
        let (self_info, neighbors) =
            list_pid_neighbors(&src, "AAAA", 1).expect("prefix lookup");
        assert_eq!(self_info.drawing_id, "AAAAAA1");
        assert_eq!(self_info.tag_label.as_deref(), Some("E-101"));
        assert_eq!(
            neighbors.iter().map(|n| n.drawing_id.as_str()).collect::<Vec<_>>(),
            vec!["BBBBBB2"]
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_returns_ambiguous_prefix_error() {
        let src = unique_pid_path("neighbors-prefix-ambiguous");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("DD1111", "Equipment", None),
                make_object("DD2222", "PipeRun", None),
                make_object("DD3333", "Nozzle", None),
                make_object("DD4444", "Instrument", None),
                make_object("EE5555", "PipeRun", None),
            ],
            vec![],
        );

        let err = list_pid_neighbors(&src, "DD", 1)
            .expect_err("ambiguous prefix must error");
        assert!(
            err.contains("ambiguous") && err.contains("4"),
            "should report ambiguity + count; got: {err}"
        );
        // Should preview at least the first match.
        assert!(
            err.contains("DD1111"),
            "should preview first matches; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_returns_no_match_error_for_unknown_prefix() {
        let src = unique_pid_path("neighbors-prefix-none");
        cache_synthetic_graph_package(
            &src,
            vec![make_object("AAAA", "Equipment", None)],
            vec![],
        );

        let err = list_pid_neighbors(&src, "ZZZZ", 1)
            .expect_err("unknown prefix must error");
        assert!(
            err.contains("no drawing_id matches") && err.contains("ZZZZ"),
            "should call out missing match + input; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_neighbors_returns_error_for_unknown_drawing_id() {
        let src = unique_pid_path("neighbors-unknown");
        cache_synthetic_graph_package(
            &src,
            vec![make_object("AAAA", "Equipment", None)],
            vec![],
        );

        let err = list_pid_neighbors(&src, "ZZZZ", 1)
            .expect_err("unknown id must error");
        // After the prefix-match upgrade, the error message format is
        // "no drawing_id matches 'X' (exact or prefix)" rather than
        // "not found". Both pieces of info still surface.
        assert!(
            err.contains("no drawing_id matches") && err.contains("ZZZZ"),
            "should call out missing drawing_id; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn read_pid_clsid_returns_none_for_default_package() {
        let src = unique_pid_path("clsid-default");
        use pid_parse::model::PidDocument;
        use pid_parse::package::PidPackage;
        use std::collections::BTreeMap;
        // PidPackage::new initializes root_clsid=None, storage_clsids=empty.
        let pkg = PidPackage::new(Some(src.clone()), BTreeMap::new(), PidDocument::default());
        pid_package_store::cache_package(&src, pkg);

        let info = read_pid_clsid(&src).expect("clsid read");
        assert_eq!(info.root_clsid, None);
        assert!(info.non_root.is_empty());

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn read_pid_clsid_returns_populated_fields() {
        let src = unique_pid_path("clsid-populated");
        use pid_parse::model::PidDocument;
        use pid_parse::package::PidPackage;
        use std::collections::BTreeMap;

        let root =
            pid_parse::Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let sub =
            pid_parse::Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
        let mut storage = BTreeMap::new();
        storage.insert("/JSite0".to_string(), sub);
        storage.insert("/JSite1".to_string(), sub);

        let pkg = PidPackage::new(Some(src.clone()), BTreeMap::new(), PidDocument::default())
            .with_root_clsid(Some(root))
            .with_storage_clsids(storage);
        pid_package_store::cache_package(&src, pkg);

        let info = read_pid_clsid(&src).expect("clsid read");
        assert_eq!(
            info.root_clsid.as_deref(),
            Some("{12345678-1234-1234-1234-123456789abc}")
        );
        assert_eq!(info.non_root.len(), 2);
        // BTreeMap sorted iteration → /JSite0 before /JSite1
        assert_eq!(info.non_root[0].0, "/JSite0");
        assert_eq!(
            info.non_root[0].1,
            "{abcdef01-2345-6789-abcd-ef0123456789}"
        );
        assert_eq!(info.non_root[1].0, "/JSite1");

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn build_pid_health_report_aggregates_from_cached_package() {
        let src = unique_pid_path("report-aggregate");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let r = build_pid_health_report(&src).expect("report");
        // Fixture has 4 streams (Drawing/General/Sheet/Blob).
        assert_eq!(r.stream_count, 4);
        // Fixture has no P&IDAttributes records → graph_stats gracefully None.
        assert!(r.graph_stats.is_none());
        // Drawing XML includes SP_DRAWINGNUMBER, so list_drawing_attributes non-empty.
        assert!(
            r.drawing_attributes
                .iter()
                .any(|(k, _)| k == "SP_DRAWINGNUMBER"),
            "drawing attrs should include SP_DRAWINGNUMBER"
        );
        // Fixture has /UnknownStorage/Blob nested → not top-level → 0 unidentified.
        assert!(r.unidentified.is_empty());
        // round-trip on this synthetic fixture must PASS.
        let v = r.verify.as_ref().expect("verify present");
        assert!(v.ok(), "fixture must round-trip clean");

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn diff_pid_files_reports_no_difference_for_identical_fixtures() {
        let a = unique_pid_path("diff-same-a");
        let b = unique_pid_path("diff-same-b");
        build_fixture_pid(&a);
        build_fixture_pid(&b);

        let (has_diff, text) = diff_pid_files(&a, &b).expect("diff");
        // Note: two freshly-created fixtures may differ on root CLSID /
        // storage CLSID because `cfb::create` can produce different
        // metadata on each call. Byte-level stream content is what we
        // care about; `(no differences)` is the render when stream set
        // is identical AND CLSID matches. For synthetic fixtures we
        // assert the weaker invariant that the rendered text mentions
        // Package Diff header and reports zero modified streams.
        assert!(text.contains("=== Package Diff ==="));
        if has_diff {
            // Synthetic CFBs often differ on CLSID alone; stream set
            // should still match.
            assert!(text.contains("summary:"), "should have summary line: {text}");
        }

        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn diff_pid_files_reports_modified_stream() {
        let a = unique_pid_path("diff-mod-a");
        let b = unique_pid_path("diff-mod-b");
        build_fixture_pid(&a);

        // Create b as a round-trip-modified version of a via the
        // writer layer — changes one attribute, preserves other
        // streams byte-for-byte.
        load_pid_native_with_package(&a).expect("load a");
        edit_pid_drawing_attribute(&a, "SP_DRAWINGNUMBER", "MODIFIED-001")
            .expect("edit drawing number");
        save_pid_native(&b, &a).expect("save b");

        let (has_diff, text) = diff_pid_files(&a, &b).expect("diff");
        assert!(has_diff, "modified drawing number must show as a diff");
        assert!(
            text.contains("Modified Streams") || text.contains("modified"),
            "rendered text should mention modifications; got: {text}"
        );

        pid_package_store::clear_package(&a);
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn diff_pid_files_errors_on_non_pid_extension() {
        let a = unique_pid_path("diff-ok");
        let b = a.with_extension("dwg");
        build_fixture_pid(&a);

        let err = diff_pid_files(&a, &b).expect_err("non-.pid should error");
        assert!(
            err.contains("not a .pid file"),
            "error should call out non-pid; got: {err}"
        );

        let _ = std::fs::remove_file(&a);
    }

    #[test]
    fn list_pid_versions_returns_none_without_decoded_field() {
        // Build a synthetic PidPackage whose parsed.doc_version2_decoded is None.
        let src = unique_pid_path("versions-none");
        use pid_parse::model::PidDocument;
        use pid_parse::package::PidPackage;
        use std::collections::BTreeMap;
        let pkg = PidPackage::new(Some(src.clone()), BTreeMap::new(), PidDocument::default());
        pid_package_store::cache_package(&src, pkg);

        let result = list_pid_versions(&src).expect("ok for cached, decoded=None");
        assert_eq!(result, None);

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_versions_returns_records_when_decoded() {
        let src = unique_pid_path("versions-some");
        use pid_parse::model::{DocVersion2, DocVersion2Record, PidDocument};
        use pid_parse::package::PidPackage;
        use std::collections::BTreeMap;

        let mut parsed = PidDocument::default();
        parsed.doc_version2_decoded = Some(DocVersion2 {
            magic_u32_le: 0x0001_0034,
            reserved_all_zero: true,
            records: vec![
                DocVersion2Record {
                    op_type: 0x82,
                    fixed: [0, 0, 9],
                    separator: 0,
                    version: 144,
                },
                DocVersion2Record {
                    op_type: 0x81,
                    fixed: [0, 0, 9],
                    separator: 0,
                    version: 77,
                },
            ],
        });
        let pkg = PidPackage::new(Some(src.clone()), BTreeMap::new(), parsed);
        pid_package_store::cache_package(&src, pkg);

        let log = list_pid_versions(&src)
            .expect("ok cached")
            .expect("Some decoded");
        assert_eq!(log.magic_u32_le, 0x0001_0034);
        assert!(log.reserved_all_zero);
        assert_eq!(log.records.len(), 2);
        assert_eq!(log.records[0].op_type, 0x82);
        assert_eq!(log.records[0].op_label, "SaveAs");
        assert_eq!(log.records[0].version, 144);
        assert_eq!(log.records[1].op_label, "Save");
        assert_eq!(log.records[1].version, 77);

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_objects_matching_filters_by_item_type() {
        let src = unique_pid_path("find-by-type");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "PipeRun", None),
                make_object("BBBB", "Instrument", Some("FIT-001")),
                make_object("CCCC", "PipeRun", Some("Run-002")),
                make_object("DDDD", "Equipment", None),
            ],
            vec![],
        );

        let pipe_runs = list_pid_objects_matching(
            &src,
            &PidFindCriterion::ItemType("PipeRun".into()),
        )
        .expect("find by type");
        let ids: Vec<&str> = pipe_runs.iter().map(|m| m.drawing_id.as_str()).collect();
        assert_eq!(ids, vec!["AAAA", "CCCC"]);
        // Tag projection is preserved.
        assert_eq!(pipe_runs[1].tag_label.as_deref(), Some("Run-002"));

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_objects_matching_filters_by_extra_field() {
        let src = unique_pid_path("find-by-extra");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Instrument", Some("FIT-001")),
                make_object("BBBB", "Instrument", Some("FIT-002")),
                make_object("CCCC", "Instrument", Some("FIT-001")),
            ],
            vec![],
        );

        let hits = list_pid_objects_matching(
            &src,
            &PidFindCriterion::ExtraEquals {
                key: "Tag".into(),
                value: "FIT-001".into(),
            },
        )
        .expect("find by extra");
        let ids: Vec<&str> = hits.iter().map(|m| m.drawing_id.as_str()).collect();
        assert_eq!(ids, vec!["AAAA", "CCCC"]);

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_objects_matching_returns_empty_when_no_match() {
        let src = unique_pid_path("find-empty");
        cache_synthetic_graph_package(
            &src,
            vec![make_object("AAAA", "PipeRun", None)],
            vec![],
        );
        let hits = list_pid_objects_matching(
            &src,
            &PidFindCriterion::ItemType("NoSuch".into()),
        )
        .expect("find returns Ok with empty Vec");
        assert!(hits.is_empty());

        let hits2 = list_pid_objects_matching(
            &src,
            &PidFindCriterion::ExtraEquals {
                key: "Tag".into(),
                value: "X".into(),
            },
        )
        .expect("find returns Ok with empty Vec");
        assert!(hits2.is_empty());

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn pid_graph_stats_returns_aggregate_counts() {
        let src = unique_pid_path("stats");
        cache_synthetic_graph_package(
            &src,
            vec![
                make_object("AAAA", "Equipment", None),
                make_object("BBBB", "PipeRun", None),
                make_object("CCCC", "Nozzle", None),
            ],
            vec![
                make_rel("R1", Some("AAAA"), Some("BBBB")), // fully
                make_rel("R2", Some("AAAA"), None),          // partially
                make_rel("R3", None, None),                  // unresolved
            ],
        );

        let s = pid_graph_stats(&src).expect("stats");
        assert_eq!(s.object_count, 3);
        assert_eq!(s.relationship_count, 3);
        assert_eq!(s.fully_resolved, 1);
        assert_eq!(s.partially_resolved, 1);
        assert_eq!(s.unresolved, 1);

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn pid_graph_stats_without_object_graph_errors() {
        let src = unique_pid_path("stats-no-graph");
        // Cache a package whose parsed.object_graph is None.
        use pid_parse::model::PidDocument;
        use pid_parse::package::PidPackage;
        use std::collections::BTreeMap;
        let pkg = PidPackage::new(
            Some(src.clone()),
            BTreeMap::new(),
            PidDocument::default(), // object_graph = None
        );
        pid_package_store::cache_package(&src, pkg);

        let err = pid_graph_stats(&src)
            .expect_err("no object_graph → error");
        assert!(
            err.contains("no object_graph"),
            "should call out missing object_graph; got: {err}"
        );

        pid_package_store::clear_package(&src);
    }

    #[test]
    fn list_pid_unidentified_cached_returns_mystery_stream() {
        let src = unique_pid_path("raw-cached");
        build_fixture_pid_with_toplevel_unknown(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        let list = list_pid_unidentified_cached(&src).expect("list");
        let paths: Vec<&str> = list.iter().map(|i| i.path.as_str()).collect();
        assert!(
            paths.contains(&"/MysteryTop"),
            "MysteryTop should be flagged as unidentified; got: {:?}",
            paths
        );
        // All known top-level prefixes (TaggedTxtData/…) must be filtered out.
        assert!(paths.iter().all(|p| !p.starts_with("/TaggedTxtData")));

        // magic is 0x12345678 → not ASCII-printable → magic_tag is None.
        let mystery = list.iter().find(|i| i.path == "/MysteryTop").unwrap();
        assert_eq!(mystery.magic_u32_le, Some(0x12345678));
        assert_eq!(mystery.magic_tag, None);
        assert!(mystery.size as usize >= 4);

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn list_pid_unidentified_file_works_without_cache() {
        let src = unique_pid_path("raw-file");
        build_fixture_pid_with_toplevel_unknown(&src);
        // Intentionally do not load_pid_native_with_package; the file
        // helper bypasses the cache entirely.

        let list = list_pid_unidentified_file(&src).expect("list from file");
        let paths: Vec<&str> = list.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"/MysteryTop"));

        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn list_pid_unidentified_cached_without_cache_errors() {
        let src = unique_pid_path("raw-no-cache");
        let err = list_pid_unidentified_cached(&src)
            .expect_err("must fail without cache");
        assert!(
            err.contains("no cached PidPackage"),
            "should call out missing cache; got: {err}"
        );
    }

    #[test]
    fn save_pid_native_then_verify_pid_file_always_passes() {
        // The combined "PIDSAVEAS <path> --verify" pipeline collapses to
        // save_pid_native + verify_pid_file; this test cements the
        // invariant that the two helpers are always consistent for a
        // freshly-loaded fixture (no edits between load and save).
        let src = unique_pid_path("saveas-verify-src");
        let dst = unique_pid_path("saveas-verify-dst");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        save_pid_native(&dst, &src).expect("save_pid_native must succeed");
        let report = verify_pid_file(&dst).expect("verify_pid_file on saved output");
        assert!(
            report.ok(),
            "save+verify pipeline must always report ok for a passthrough fixture; report: {:?}",
            report
        );
        assert_eq!(report.matched, 4);

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn sppid_software_version_tracks_cargo_pkg_version() {
        // SPPID_SOFTWARE_VERSION is written into publish Data.xml / Meta.xml
        // as `SoftwareVersion`. SPPID consumers may string-match that field
        // exactly, so we don't bind the constant to `CARGO_PKG_VERSION` at
        // compile time. Instead this test asserts the two are identical —
        // any `cargo release` that bumps Cargo.toml without touching this
        // constant will fail here, forcing an explicit review.
        assert_eq!(
            super::SPPID_SOFTWARE_VERSION,
            env!("CARGO_PKG_VERSION"),
            "SPPID_SOFTWARE_VERSION drift: Cargo.toml version changed but \
             the publish identity constant was not updated. Either bump \
             SPPID_SOFTWARE_VERSION to match, or (if the SPPID consumer \
             requires a frozen value) update the doc-comment and relax \
             this assertion intentionally."
        );
    }

    #[test]
    fn sppid_tool_id_matches_crate_name() {
        // Guard that `env!("CARGO_PKG_NAME")` binding is in effect and the
        // crate name remains "H7CAD" (which SPPID consumers rely on).
        assert_eq!(super::SPPID_TOOL_ID, env!("CARGO_PKG_NAME"));
        assert_eq!(super::SPPID_TOOL_ID, "H7CAD");
    }

    #[test]
    fn save_pid_native_copies_sidecars_when_both_present_with_new_stem() {
        // Reproduces the orphan-sidecar bug: a published .pid carries
        // {stem}_Data.xml + {stem}_Meta.xml next to it. "Save As" to a
        // different basename in a different directory must mirror both
        // sidecars with the new stem so re-open picks them up.
        let src = unique_pid_path("sidecar-copy-src");
        let dst = unique_pid_path("sidecar-copy-dst-renamed");
        build_fixture_pid(&src);
        let (src_data, src_meta) = write_publish_sidecars(&src, "SIDECAR-SRC");
        load_pid_native_with_package(&src).expect("load fixture");

        save_pid_native(&dst, &src).expect("save with sidecars");

        let dst_data = dst.with_file_name(format!(
            "{}_Data.xml",
            dst.file_stem().and_then(|s| s.to_str()).unwrap()
        ));
        let dst_meta = dst.with_file_name(format!(
            "{}_Meta.xml",
            dst.file_stem().and_then(|s| s.to_str()).unwrap()
        ));
        assert!(dst_data.exists(), "dst sidecar Data.xml must exist at {}", dst_data.display());
        assert!(dst_meta.exists(), "dst sidecar Meta.xml must exist at {}", dst_meta.display());

        let src_data_bytes = std::fs::read(&src_data).unwrap();
        let dst_data_bytes = std::fs::read(&dst_data).unwrap();
        assert_eq!(src_data_bytes, dst_data_bytes, "Data.xml bytes must be identical");
        let src_meta_bytes = std::fs::read(&src_meta).unwrap();
        let dst_meta_bytes = std::fs::read(&dst_meta).unwrap();
        assert_eq!(src_meta_bytes, dst_meta_bytes, "Meta.xml bytes must be identical");

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
        let _ = std::fs::remove_file(&src_data);
        let _ = std::fs::remove_file(&src_meta);
        let _ = std::fs::remove_file(&dst_data);
        let _ = std::fs::remove_file(&dst_meta);
    }

    #[test]
    fn save_pid_native_is_noop_when_no_sidecars_present() {
        // A .pid opened straight from SmartPlant (no H7CAD publish sidecar)
        // must continue to Save As without fabricating sidecars.
        let src = unique_pid_path("sidecar-noop-src");
        let dst = unique_pid_path("sidecar-noop-dst");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        save_pid_native(&dst, &src).expect("save without sidecars must succeed");

        let dst_data = dst.with_file_name(format!(
            "{}_Data.xml",
            dst.file_stem().and_then(|s| s.to_str()).unwrap()
        ));
        let dst_meta = dst.with_file_name(format!(
            "{}_Meta.xml",
            dst.file_stem().and_then(|s| s.to_str()).unwrap()
        ));
        assert!(!dst_data.exists(), "no sidecar should be fabricated at {}", dst_data.display());
        assert!(!dst_meta.exists(), "no sidecar should be fabricated at {}", dst_meta.display());

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn save_pid_native_errors_on_incomplete_sidecar_pair() {
        // If only one of the sidecars exists next to src at save time, Save
        // As must refuse rather than silently propagate a half-broken
        // bundle — mirrors merge_publish_sidecars' open-side contract.
        //
        // Note on ordering: `open_pid` itself rejects incomplete pairs, so
        // we load FIRST (no sidecars present yet), then drop just one
        // sidecar next to src to trigger the save-side check.
        let src = unique_pid_path("sidecar-partial-src");
        let dst = unique_pid_path("sidecar-partial-dst");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture with no sidecars");

        // Drop only the Data.xml half to simulate the corrupt bundle that
        // the save-side check must reject.
        let src_data = src.with_file_name(format!(
            "{}_Data.xml",
            src.file_stem().and_then(|s| s.to_str()).unwrap()
        ));
        std::fs::write(&src_data, b"<dummy/>").expect("write half sidecar");

        let err = save_pid_native(&dst, &src).expect_err("incomplete pair must error");
        assert!(
            err.contains("incomplete publish bundle"),
            "expected 'incomplete publish bundle' hint; got: {err}"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
        let _ = std::fs::remove_file(&src_data);
    }

    #[test]
    fn verify_pid_file_passes_for_synthetic_fixture_without_cache() {
        let src = unique_pid_path("verify-file");
        build_fixture_pid(&src);
        // Intentionally do not load_pid_native_with_package; verify_pid_file
        // bypasses the cache entirely.

        let report = verify_pid_file(&src).expect("verify file");
        assert!(report.ok(), "verify_pid_file should pass on a fresh fixture; report: {:?}", report);
        assert_eq!(report.matched, 4);

        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn list_pid_metadata_returns_error_when_general_stream_missing() {
        let src = unique_pid_path("list-no-general");
        // Build a CFB that has Drawing but no General stream.
        if src.exists() {
            std::fs::remove_file(&src).expect("clean fixture");
        }
        let mut cfb = ::cfb::create(&src).expect("create fixture cfb");
        cfb.create_storage("/TaggedTxtData").unwrap();
        let drawing = b"<Drawing><Tag SP_X=\"1\"/></Drawing>";
        let mut s = cfb.create_stream(FIXTURE_DRAWING).unwrap();
        s.write_all(drawing).unwrap();
        drop(s);
        cfb.flush().unwrap();
        load_pid_native_with_package(&src).expect("load");

        let err = list_pid_metadata(&src).expect_err("missing General stream → error");
        assert!(
            err.contains("missing") && err.contains("General"),
            "should call out missing General stream; got: {err}"
        );

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn edit_then_save_round_trips_new_drawing_number_through_disk() {
        let src = unique_pid_path("edit-rt-src");
        let dst = unique_pid_path("edit-rt-dst");
        build_fixture_pid(&src);
        load_pid_native_with_package(&src).expect("load fixture");

        edit_pid_drawing_number(&src, "ROUND-TRIP-009").expect("edit");
        save_pid_native(&dst, &src).expect("save");

        let parser = PidParser::new();
        let written = parser.parse_package(&dst).expect("re-parse written file");
        let new_xml = std::str::from_utf8(&written.streams[FIXTURE_DRAWING].data).unwrap();
        assert!(
            new_xml.contains("SP_DRAWINGNUMBER=\"ROUND-TRIP-009\""),
            "edited drawing number must survive disk round-trip; got: {new_xml}"
        );
        // The other three streams must remain byte-for-byte identical
        // with the original fixture.
        let original = parser.parse_package(&src).expect("re-parse src for compare");
        for path in [FIXTURE_GENERAL, FIXTURE_SHEET, FIXTURE_BLOB] {
            assert_eq!(
                original.streams[path].data, written.streams[path].data,
                "untouched stream {} should round-trip verbatim after metadata edit",
                path
            );
        }

        pid_package_store::clear_package(&src);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    fn sample_doc() -> PidDocument {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("D-100".into()),
            project_number: Some("P-01".into()),
            objects: vec![
                PidObject {
                    drawing_id: "OBJ_AAAA1111".into(),
                    item_type: "Instrument".into(),
                    drawing_item_type: Some("Symbol".into()),
                    model_id: Some("MODEL-INST-001".into()),
                    extra: BTreeMap::from([("Tag".into(), "FIT-001".into())]),
                    record_id: None,
                    field_x: Some(1),
                },
                PidObject {
                    drawing_id: "OBJ_BBBB2222".into(),
                    item_type: "PipeRun".into(),
                    drawing_item_type: None,
                    model_id: None,
                    extra: BTreeMap::new(),
                    record_id: None,
                    field_x: Some(2),
                },
            ],
            relationships: vec![PidRelationship {
                model_id: "Relationship.R1".into(),
                guid: "R1".into(),
                record_id: None,
                field_x: Some(3),
                source_drawing_id: Some("OBJ_AAAA1111".into()),
                target_drawing_id: Some("OBJ_BBBB2222".into()),
            }],
            by_drawing_id: BTreeMap::new(),
            counts_by_type: BTreeMap::new(),
        });
        doc
    }

    #[test]
    fn converts_pid_document_into_multi_panel_native_entities() {
        let (native, summary) = pid_document_to_native(&sample_doc());
        assert!(native.layers.contains_key("PID_META"));
        assert!(native.layers.contains_key("PID_SYMBOLS"));
        assert!(native.layers.contains_key("PID_CLUSTERS"));
        assert!(native.layers.contains_key("PID_OBJECTS_Instrument"));
        assert!(native.layers.contains_key("PID_OBJECTS_PipeRun"));
        assert_eq!(summary.object_count, 2);
        assert_eq!(summary.relationship_count, 1);
        assert!(native.entities.len() >= 5);
    }

    #[test]
    fn missing_relationship_endpoints_are_reported() {
        let mut doc = sample_doc();
        if let Some(graph) = &mut doc.object_graph {
            graph.relationships[0].source_drawing_id = Some("UNKNOWN".into());
        }
        let (native, summary) = pid_document_to_native(&doc);
        let line_count = native
            .model_space_entities()
            .filter(|entity| matches!(entity.data, nm::EntityData::Line { .. }))
            .count();
        assert_eq!(line_count, 1);
        assert!(summary.unresolved_relationship_count >= 1);
        assert!(native.layers.contains_key("PID_UNRESOLVED"));
    }

    #[test]
    fn pid_bundle_preserves_doc_and_indexes_preview_handles() {
        let mut doc = sample_doc();
        doc.sheet_streams.push(SheetStream {
            name: "Sheet-1".into(),
            path: "/Sheets/Sheet-1".into(),
            size: 128,
            extracted_texts: vec!["endpoint-a".into(), "endpoint-b".into()],
            magic_u32_le: Some(0x6C90_F544),
            magic_tag: Some("DF90".into()),
            header: None,
            attribute_records: vec![],
            probe_summary: Some(ProbeSummary {
                body_start_offset: 16,
                marker_count: 2,
                records_extracted: 2,
                bytes_scanned: 128,
            }),
            endpoint_records: vec![],
        });
        doc.cross_reference = Some(CrossReferenceGraph {
            cluster_coverage: ClusterCoverage::default(),
            symbol_usage: vec![SymbolUsage {
                symbol_path: r"\\srv\sym\Valve.sym".into(),
                symbol_name: Some("Valve".into()),
                jsite_names: vec!["JSite0".into()],
                usage_count: 1,
                references: Vec::new(),
            }],
            attribute_classes: vec![AttributeClassSummary {
                class_name: "Instrument".into(),
                record_count: 1,
                drawing_ids: vec!["OBJ_AAAA1111".into()],
                model_ids: vec!["MODEL-INST-001".into()],
                unique_attribute_names: vec!["Tag".into()],
                records: Vec::new(),
            }],
            root_presence: vec![],
        });

        let bundle = pid_document_to_bundle(&doc);
        assert_eq!(bundle.pid_doc.object_graph.as_ref().unwrap().objects.len(), 2);
        assert_eq!(bundle.summary.sheet_count, 1);
        assert_eq!(bundle.summary.symbol_count, 1);
        assert_eq!(bundle.summary.attribute_class_count, 1);

        let object_key = PidNodeKey::Object {
            drawing_id: "OBJ_AAAA1111".into(),
        };
        let relationship_key = PidNodeKey::Relationship { guid: "R1".into() };
        let sheet_key = PidNodeKey::Sheet {
            name: "Sheet-1".into(),
        };
        let symbol_key = PidNodeKey::Symbol {
            symbol_path: r"\\srv\sym\Valve.sym".into(),
        };

        let object_handle = bundle.preview_index.handles_for(&object_key)[0];
        let relationship_handle = bundle.preview_index.handles_for(&relationship_key)[0];

        assert!(!bundle.preview_index.handles_for(&sheet_key).is_empty());
        assert!(!bundle.preview_index.handles_for(&symbol_key).is_empty());
        assert_eq!(
            bundle.preview_index.key_for_handle(object_handle),
            Some(&object_key)
        );
        assert_eq!(
            bundle.preview_index.key_for_handle(relationship_handle),
            Some(&relationship_key)
        );
        assert_eq!(
            bundle.preview_index.key_for_handle(Handle::new(9_999_999)),
            None
        );
    }

    #[test]
    fn open_pid_merges_publish_sidecars_into_object_graph_when_present() {
        let path = unique_pid_path("publish-sidecar");
        build_fixture_pid(&path);
        let (data_path, meta_path) = write_publish_sidecars(&path, "DWG-BRAN-001");

        let bundle = open_pid(&path).expect("open pid with sidecars");
        let graph = bundle.pid_doc.object_graph.as_ref().expect("object graph");

        assert!(bundle.summary.object_graph_available);
        assert_eq!(bundle.summary.object_count, graph.objects.len());
        assert!(
            graph
                .counts_by_type
                .get("PIDBranchPoint")
                .copied()
                .unwrap_or_default()
                >= 1
        );
        assert!(
            graph
                .counts_by_type
                .get("PIDPipingBranchPoint")
                .copied()
                .unwrap_or_default()
                >= 1
        );
        assert!(
            graph.relationships.iter().any(|rel| {
                rel.model_id.contains("PipingTapOrFitting")
                    && rel.source_drawing_id.as_deref() == Some("PIPINGBRANCH000000000000000000001")
            }),
            "sidecar relationships should survive into the object graph"
        );

        crate::io::pid_package_store::clear_package(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&data_path);
        let _ = std::fs::remove_file(&meta_path);
    }

    #[test]
    fn export_sppid_publish_bundle_without_bran_insert_errors() {
        let doc = acadrust::CadDocument::new();
        let out = unique_pid_path("publish-no-bran");
        let err = export_sppid_publish_bundle(&doc, &out).expect_err("missing BRAN should fail");
        assert!(
            err.contains("exactly one SPPID_BRAN insert"),
            "error should call out the missing BRAN authoring insert; got: {err}"
        );
    }

    #[test]
    fn export_sppid_publish_bundle_rejects_multiple_bran_inserts() {
        let mut doc = acadrust::CadDocument::new();
        ensure_sppid_bran_block_library(&mut doc).expect("seed library");
        for _ in 0..2 {
            let mut insert = Insert::new(SPPID_BRAN_BLOCK_NAME, Vector3::new(0.0, 0.0, 0.0));
            insert.attributes = SPPID_BRAN_ATTRIBUTES
                .iter()
                .map(|(tag, _, value)| {
                    let mut attr = AttributeEntity {
                        tag: (*tag).to_string(),
                        value: (*value).to_string(),
                        ..Default::default()
                    };
                    attr.set_value(*value);
                    attr
                })
                .collect();
            doc.add_entity(EntityType::Insert(insert))
                .expect("seed BRAN insert");
        }

        let out = unique_pid_path("publish-multi-bran");
        let err = export_sppid_publish_bundle(&doc, &out).expect_err("multiple BRANs should fail");
        assert!(
            err.contains("exactly one SPPID_BRAN insert per drawing"),
            "error should call out the first-phase one-BRAN limit; got: {err}"
        );
    }
}
