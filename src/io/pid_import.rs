use acadrust::Handle;
use h7cad_native_model as nm;
use pid_parse::writer::{PidWriter, WritePlan};
use pid_parse::{build_import_view, PidDocument, PidImportView, PidParser};
use std::collections::BTreeMap;
use std::path::Path;

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
}

impl PidPreviewIndex {
    pub fn handles_for(&self, key: &PidNodeKey) -> Vec<Handle> {
        self.by_key.get(key).cloned().unwrap_or_default()
    }

    pub fn key_for_handle(&self, handle: Handle) -> Option<&PidNodeKey> {
        self.by_handle.get(&handle.value())
    }

    fn record_existing_handle(&mut self, key: PidNodeKey, handle: Handle) {
        self.by_key.entry(key.clone()).or_default().push(handle);
        self.by_handle.entry(handle.value()).or_insert(key);
    }
}

#[derive(Debug, Clone)]
pub struct PidOpenBundle {
    pub pid_doc: PidDocument,
    pub native_preview: nm::CadDocument,
    pub summary: PidImportSummary,
    pub preview_index: PidPreviewIndex,
}

pub fn open_pid(path: &Path) -> Result<PidOpenBundle, String> {
    let parser = PidParser::new();
    let package = parser.parse_package(path).map_err(|e| e.to_string())?;
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
    Ok(())
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
    ensure_layer(&mut native, "PID_SYMBOLS", 4);
    ensure_layer(&mut native, "PID_CLUSTERS", 2);
    ensure_layer(&mut native, "PID_STREAMS", 3);
    ensure_layer(&mut native, "PID_CROSSREF", 7);
    ensure_layer(&mut native, "PID_UNRESOLVED", 6);

    let mut preview_index = PidPreviewIndex::default();
    let mut positions = BTreeMap::new();
    let mut object_count = 0usize;
    for (index, object) in view.objects.iter().enumerate() {
        let point = grid_point(index);
        positions.insert(object.drawing_id.clone(), point);
        add_object_entities(&mut native, &mut preview_index, object, point);
        object_count += 1;
    }

    let unresolved_edges = add_relationship_entities(&mut native, &mut preview_index, view, &positions);
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
            }],
            attribute_classes: vec![AttributeClassSummary {
                class_name: "Instrument".into(),
                record_count: 1,
                drawing_ids: vec!["OBJ_AAAA1111".into()],
                model_ids: vec!["MODEL-INST-001".into()],
                unique_attribute_names: vec!["Tag".into()],
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
}
