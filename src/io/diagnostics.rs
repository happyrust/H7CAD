//! UI-facing diagnostic notices surfaced from file-open operations.
//!
//! The underlying DWG/DXF/PID readers may encounter non-fatal issues
//! (unsupported sub-sections, recovered errors, "not implemented yet"
//! features, etc.). `acadrust` already collects these into a
//! [`NotificationCollection`] on `CadDocument::notifications`, but
//! until now the runtime threw that information away at the io-module
//! boundary. Milestone D' keeps those notices alive so the command
//! line can echo them to the user.
//!
//! The type here is intentionally **string-based** and independent of
//! the upstream `acadrust::Notification` enum. That keeps the io→app
//! boundary clean of vendor types, lets future DWG backends (our own
//! native-dwg, DXF-level advisories) emit the same shape of message
//! without round-tripping through acadrust, and makes serialisation
//! for future persistence trivial.
//!
//! Conversion helpers exist on a per-source basis so each backend owns
//! its mapping: [`from_acadrust_notifications`] for the acadrust DWG
//! path, and [`from_native_dxf_document`] for native DXF advisories.

use acadrust::notification::{Notification, NotificationCollection, NotificationType};
use std::collections::BTreeMap;

/// Severity classification for an [`OpenNotice`].
///
/// Mirrors acadrust's `NotificationType` but is owned by the io layer
/// and carries Chinese-facing labels for UI presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeSeverity {
    /// Feature recognised but not implemented yet in the reader.
    NotImplemented,
    /// Feature implemented but not supported in the current context
    /// (e.g. attempted inside an unsupported section).
    NotSupported,
    /// Non-fatal anomaly (missing handle, duplicate key, etc.).
    Warning,
    /// Recoverable error — the reader salvaged the record and
    /// continued parsing.
    Error,
}

impl NoticeSeverity {
    /// Short Chinese label used by the command line.
    pub fn zh_label(self) -> &'static str {
        match self {
            Self::NotImplemented => "未实现",
            Self::NotSupported => "不支持",
            Self::Warning => "警告",
            Self::Error => "已恢复错误",
        }
    }

    /// Stable English tag used for log-line prefixes / future
    /// persistence. Mirrors acadrust's `NotificationType` naming so
    /// round-tripping with upstream stays obvious. Not wired into the
    /// current UI path yet; kept for the planned diagnostic-panel /
    /// log-file work.
    #[allow(dead_code)]
    pub fn en_tag(self) -> &'static str {
        match self {
            Self::NotImplemented => "NotImplemented",
            Self::NotSupported => "NotSupported",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

impl From<NotificationType> for NoticeSeverity {
    fn from(value: NotificationType) -> Self {
        match value {
            NotificationType::NotImplemented => Self::NotImplemented,
            NotificationType::NotSupported => Self::NotSupported,
            NotificationType::Warning => Self::Warning,
            NotificationType::Error => Self::Error,
        }
    }
}

/// A single user-facing notice surfaced from a file-open operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenNotice {
    pub severity: NoticeSeverity,
    pub message: String,
}

impl OpenNotice {
    pub fn new(severity: NoticeSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
        }
    }

    /// Chinese-facing one-line form suitable for the command line:
    /// `"[警告] missing handle"`.
    pub fn format_zh(&self) -> String {
        format!("[{}] {}", self.severity.zh_label(), self.message)
    }
}

impl From<&Notification> for OpenNotice {
    fn from(value: &Notification) -> Self {
        Self::new(value.notification_type.into(), value.message.clone())
    }
}

/// Aggregate severity-bucket counts for a notice list, used by the
/// command-line summary line ("3 warnings, 1 recovered error, …").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoticeCounts {
    pub not_implemented: usize,
    pub not_supported: usize,
    pub warning: usize,
    pub error: usize,
}

impl NoticeCounts {
    pub fn from_notices(notices: &[OpenNotice]) -> Self {
        let mut c = Self::default();
        for n in notices {
            match n.severity {
                NoticeSeverity::NotImplemented => c.not_implemented += 1,
                NoticeSeverity::NotSupported => c.not_supported += 1,
                NoticeSeverity::Warning => c.warning += 1,
                NoticeSeverity::Error => c.error += 1,
            }
        }
        c
    }

    pub fn total(&self) -> usize {
        self.not_implemented + self.not_supported + self.warning + self.error
    }

    /// Compact Chinese-facing summary fragment suitable for suffixing
    /// to the "Opened ... — N entities" line.
    /// Returns [`None`] when there are zero notices so callers can
    /// skip the suffix entirely.
    pub fn summary_zh(&self) -> Option<String> {
        if self.total() == 0 {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        if self.warning > 0 {
            parts.push(format!("{} 条警告", self.warning));
        }
        if self.error > 0 {
            parts.push(format!("{} 条已恢复错误", self.error));
        }
        if self.not_implemented > 0 {
            parts.push(format!("{} 条未实现", self.not_implemented));
        }
        if self.not_supported > 0 {
            parts.push(format!("{} 条不支持", self.not_supported));
        }
        Some(parts.join(" / "))
    }
}

/// Convert an acadrust [`NotificationCollection`] (as found on
/// `CadDocument::notifications`) into the io-layer [`OpenNotice`]
/// vector.
pub fn from_acadrust_notifications(src: &NotificationCollection) -> Vec<OpenNotice> {
    src.iter().map(OpenNotice::from).collect()
}

/// Produce PID-specific advisories from a [`PidImportSummary`].
///
/// Surface only high-signal facts to the open-notice channel:
/// * unresolved relationships (broken object references)
/// * missing object graph despite present relationships (navigation impact)
/// * absence of Sheet streams (no source-backed geometry possible)
///
/// All other summary fields stay implicit; the PID workbench panel can
/// expose detailed counts where appropriate.
pub fn from_pid_import_summary(
    summary: &crate::io::pid_import::PidImportSummary,
) -> Vec<OpenNotice> {
    let mut notices = Vec::new();

    if summary.unresolved_relationship_count > 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!(
                "PID 文件含 {} 个未解析关系，部分对象引用可能无法定位",
                summary.unresolved_relationship_count
            ),
        ));
    }

    if !summary.object_graph_available && summary.relationship_count > 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            "PID 对象关系图未生成，导航/搜索功能可能受限".to_string(),
        ));
    }

    if summary.sheet_count == 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::NotImplemented,
            "PID 文件无 Sheet stream，当前无法显示原始几何".to_string(),
        ));
    }

    notices
}

/// Produce DXF-specific advisories from a successfully parsed native
/// document. These are not parse errors; they flag content preserved for
/// round-trip but not yet fully understood or editable by H7CAD.
pub fn from_native_dxf_document(doc: &h7cad_native_model::CadDocument) -> Vec<OpenNotice> {
    use h7cad_native_model::{Entity, EntityData, ObjectData};

    fn scan_entities(
        entities: &[Entity],
        unknown_entities: &mut BTreeMap<String, usize>,
        proxy_entities: &mut usize,
    ) {
        for entity in entities {
            match &entity.data {
                EntityData::Unknown { entity_type, .. } => {
                    *unknown_entities.entry(entity_type.clone()).or_default() += 1;
                }
                EntityData::ProxyEntity { .. } => *proxy_entities += 1,
                _ => {}
            }
        }
    }

    fn summarize_counts(counts: &BTreeMap<String, usize>) -> String {
        counts
            .iter()
            .map(|(name, count)| format!("{name} x{count}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    let mut unknown_entities = BTreeMap::new();
    let mut proxy_entities = 0usize;
    scan_entities(&doc.entities, &mut unknown_entities, &mut proxy_entities);
    for block in doc.block_records.values() {
        scan_entities(&block.entities, &mut unknown_entities, &mut proxy_entities);
    }

    let mut unknown_objects = BTreeMap::new();
    let mut proxy_objects = 0usize;
    for object in &doc.objects {
        match &object.data {
            ObjectData::Unknown { object_type, .. } => {
                *unknown_objects.entry(object_type.clone()).or_default() += 1;
            }
            ObjectData::ProxyObject { .. } => proxy_objects += 1,
            _ => {}
        }
    }

    let mut notices = Vec::new();
    if !unknown_entities.is_empty() {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!(
                "DXF contains unsupported entity records preserved as placeholders: {}",
                summarize_counts(&unknown_entities)
            ),
        ));
    }
    if proxy_entities > 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!(
                "DXF contains {proxy_entities} proxy entity records preserved for round-trip; display/edit support is limited"
            ),
        ));
    }
    if !unknown_objects.is_empty() {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!(
                "DXF contains unsupported object records preserved as placeholders: {}",
                summarize_counts(&unknown_objects)
            ),
        ));
    }
    if proxy_objects > 0 {
        notices.push(OpenNotice::new(
            NoticeSeverity::Warning,
            format!("DXF contains {proxy_objects} proxy object records preserved for round-trip"),
        ));
    }

    notices
}

#[cfg(test)]
mod tests {
    use super::*;
    use h7cad_native_model::{CadDocument, CadObject, Entity, EntityData, Handle, ObjectData};

    #[test]
    fn severity_from_acadrust_covers_every_variant() {
        assert_eq!(
            NoticeSeverity::from(NotificationType::NotImplemented),
            NoticeSeverity::NotImplemented
        );
        assert_eq!(
            NoticeSeverity::from(NotificationType::NotSupported),
            NoticeSeverity::NotSupported
        );
        assert_eq!(
            NoticeSeverity::from(NotificationType::Warning),
            NoticeSeverity::Warning
        );
        assert_eq!(
            NoticeSeverity::from(NotificationType::Error),
            NoticeSeverity::Error
        );
    }

    #[test]
    fn severity_labels_are_stable_in_chinese_and_english() {
        for sev in [
            NoticeSeverity::NotImplemented,
            NoticeSeverity::NotSupported,
            NoticeSeverity::Warning,
            NoticeSeverity::Error,
        ] {
            assert!(!sev.zh_label().is_empty(), "{sev:?} zh_label empty");
            assert!(!sev.en_tag().is_empty(), "{sev:?} en_tag empty");
        }
        assert_eq!(NoticeSeverity::Warning.zh_label(), "警告");
        assert_eq!(NoticeSeverity::Error.zh_label(), "已恢复错误");
        assert_eq!(NoticeSeverity::NotImplemented.en_tag(), "NotImplemented");
    }

    #[test]
    fn format_zh_produces_bracketed_label_prefix() {
        let notice = OpenNotice::new(NoticeSeverity::Warning, "handle 0xABC missing");
        assert_eq!(notice.format_zh(), "[警告] handle 0xABC missing");
    }

    #[test]
    fn from_acadrust_notifications_preserves_order_and_message_text() {
        let mut collection = NotificationCollection::new();
        collection.notify(NotificationType::Warning, "w1");
        collection.notify(NotificationType::Error, "e1");
        collection.notify(NotificationType::NotImplemented, "ni1");

        let notices = from_acadrust_notifications(&collection);
        assert_eq!(notices.len(), 3);
        assert_eq!(notices[0].severity, NoticeSeverity::Warning);
        assert_eq!(notices[0].message, "w1");
        assert_eq!(notices[1].severity, NoticeSeverity::Error);
        assert_eq!(notices[1].message, "e1");
        assert_eq!(notices[2].severity, NoticeSeverity::NotImplemented);
        assert_eq!(notices[2].message, "ni1");
    }

    #[test]
    fn from_acadrust_notifications_returns_empty_for_empty_collection() {
        let collection = NotificationCollection::new();
        assert!(from_acadrust_notifications(&collection).is_empty());
    }

    #[test]
    fn notice_counts_bucket_by_severity() {
        let notices = vec![
            OpenNotice::new(NoticeSeverity::Warning, "w1"),
            OpenNotice::new(NoticeSeverity::Warning, "w2"),
            OpenNotice::new(NoticeSeverity::Error, "e1"),
            OpenNotice::new(NoticeSeverity::NotImplemented, "ni1"),
        ];
        let counts = NoticeCounts::from_notices(&notices);
        assert_eq!(counts.warning, 2);
        assert_eq!(counts.error, 1);
        assert_eq!(counts.not_implemented, 1);
        assert_eq!(counts.not_supported, 0);
        assert_eq!(counts.total(), 4);
    }

    #[test]
    fn notice_counts_summary_zh_none_when_empty() {
        let counts = NoticeCounts::default();
        assert!(counts.summary_zh().is_none());
    }

    #[test]
    fn notice_counts_summary_zh_joins_nonzero_buckets() {
        let counts = NoticeCounts {
            warning: 3,
            error: 1,
            not_implemented: 2,
            not_supported: 0,
        };
        let summary = counts.summary_zh().expect("non-empty");
        assert!(summary.contains("3 条警告"));
        assert!(summary.contains("1 条已恢复错误"));
        assert!(summary.contains("2 条未实现"));
        assert!(!summary.contains("不支持"));
        assert!(summary.contains(" / "));
    }

    #[test]
    fn native_dxf_diagnostics_report_unknown_and_proxy_records() {
        let mut doc = CadDocument::new();
        doc.entities.push(Entity::new(EntityData::Unknown {
            entity_type: "CUSTOM_ENTITY".into(),
            raw_codes: Vec::new(),
        }));
        doc.entities.push(Entity::new(EntityData::ProxyEntity {
            class_id: 1,
            application_class_id: 2,
            raw_codes: vec![(90, "1".into())],
        }));
        doc.objects.push(CadObject {
            handle: Handle::new(0x80),
            owner_handle: Handle::NULL,
            data: ObjectData::Unknown {
                object_type: "CUSTOM_OBJECT".into(),
                raw_codes: Vec::new(),
            },
        });
        doc.objects.push(CadObject {
            handle: Handle::new(0x81),
            owner_handle: Handle::NULL,
            data: ObjectData::ProxyObject {
                class_id: 3,
                application_class_id: 4,
                raw_codes: vec![(90, "3".into())],
            },
        });

        let notices = from_native_dxf_document(&doc);

        assert_eq!(notices.len(), 4);
        assert!(notices
            .iter()
            .all(|n| n.severity == NoticeSeverity::Warning));
        assert!(notices
            .iter()
            .any(|n| n.message.contains("CUSTOM_ENTITY x1")));
        assert!(notices.iter().any(|n| n.message.contains("proxy entity")));
        assert!(notices
            .iter()
            .any(|n| n.message.contains("CUSTOM_OBJECT x1")));
        assert!(notices.iter().any(|n| n.message.contains("proxy object")));
    }

    #[test]
    fn native_dxf_diagnostics_empty_for_fully_supported_document() {
        let doc = CadDocument::new();
        assert!(from_native_dxf_document(&doc).is_empty());
    }

    use crate::io::pid_import::PidImportSummary;

    fn pid_summary_baseline() -> PidImportSummary {
        PidImportSummary {
            title: "T".into(),
            object_count: 5,
            relationship_count: 3,
            unresolved_relationship_count: 0,
            symbol_count: 1,
            cluster_count: 1,
            sheet_count: 1,
            stream_count: 10,
            attribute_class_count: 0,
            tagged_text_count: 0,
            dynamic_attribute_record_count: 0,
            rendered_geom_points: 0,
            rendered_geom_lines: 0,
            skipped_probe_only_geometry: 0,
            skipped_broad_coordinate_hints: 0,
            object_graph_available: true,
        }
    }

    #[test]
    fn pid_diagnostics_empty_for_healthy_summary() {
        assert!(from_pid_import_summary(&pid_summary_baseline()).is_empty());
    }

    #[test]
    fn pid_diagnostics_warn_when_relationships_unresolved() {
        let mut summary = pid_summary_baseline();
        summary.unresolved_relationship_count = 7;
        let notices = from_pid_import_summary(&summary);
        assert!(
            notices
                .iter()
                .any(|n| n.severity == NoticeSeverity::Warning && n.message.contains("7")),
            "expected unresolved warning, got {notices:?}"
        );
    }

    #[test]
    fn pid_diagnostics_warn_when_object_graph_missing_but_relationships_present() {
        let mut summary = pid_summary_baseline();
        summary.object_graph_available = false;
        let notices = from_pid_import_summary(&summary);
        assert!(
            notices
                .iter()
                .any(|n| n.severity == NoticeSeverity::Warning && n.message.contains("对象关系图")),
            "expected object-graph warning, got {notices:?}"
        );
    }

    #[test]
    fn pid_diagnostics_skip_object_graph_warning_when_no_relationships() {
        let mut summary = pid_summary_baseline();
        summary.object_graph_available = false;
        summary.relationship_count = 0;
        let notices = from_pid_import_summary(&summary);
        assert!(
            !notices.iter().any(|n| n.message.contains("对象关系图")),
            "object-graph warning should not fire without relationships, got {notices:?}"
        );
    }

    #[test]
    fn pid_diagnostics_flag_when_no_sheet_streams() {
        let mut summary = pid_summary_baseline();
        summary.sheet_count = 0;
        let notices = from_pid_import_summary(&summary);
        assert!(
            notices.iter().any(
                |n| n.severity == NoticeSeverity::NotImplemented && n.message.contains("Sheet")
            ),
            "expected no-Sheet advisory, got {notices:?}"
        );
    }
}
