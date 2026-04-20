use crate::scene::Scene;
use crate::io::pid_import::{PidImportSummary, PidNodeKey, PidPreviewIndex};
use crate::ui::{LayerPanel, PidBrowserListItem, PidBrowserSection, PropertiesPanel};
use crate::command::CadCommand;
use crate::snap::SnapResult;
use crate::scene::grip::GripEdit;
use crate::scene::GripDef;
use crate::modules::home::modify::refedit::RefEditSession;
use acadrust::{CadDocument, Handle};
use acadrust::tables::Ucs;
use h7cad_native_model as nm;
use crate::linetypes;
use pid_parse::{PidDocument, PidImportView};
use std::path::PathBuf;
use iced;

// ── Per-document tab state ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocumentTabMode {
    Cad,
    Pid,
}

pub(super) struct PidTabState {
    pub(super) document: PidDocument,
    pub(super) import_view: PidImportView,
    pub(super) summary: PidImportSummary,
    pub(super) preview_index: PidPreviewIndex,
    pub(super) active_section: PidBrowserSection,
    pub(super) search_text: String,
    pub(super) selected_key: Option<PidNodeKey>,
    pub(super) last_located_handle: Option<Handle>,
    pub(super) hide_meta: bool,
    pub(super) hide_unresolved: bool,
}

impl PidTabState {
    pub(super) fn new(
        document: PidDocument,
        import_view: PidImportView,
        summary: PidImportSummary,
        preview_index: PidPreviewIndex,
    ) -> Self {
        let active_section = if document.object_graph.is_some() {
            PidBrowserSection::Overview
        } else {
            PidBrowserSection::Streams
        };
        Self {
            document,
            import_view,
            summary,
            preview_index,
            active_section,
            search_text: String::new(),
            selected_key: None,
            last_located_handle: None,
            hide_meta: false,
            hide_unresolved: false,
        }
    }

    pub(super) fn browser_items(&self) -> Vec<PidBrowserListItem> {
        let mut items = match self.active_section {
            PidBrowserSection::Overview => vec![PidBrowserListItem {
                key: PidNodeKey::Overview,
                title: "Document Overview".into(),
                subtitle: Some(self.summary.title.clone()),
                badge: Some(format!(
                    "{} obj / {} rel / {} unresolved",
                    self.summary.object_count,
                    self.summary.relationship_count,
                    self.summary.unresolved_relationship_count
                )),
            }],
            PidBrowserSection::Objects => self
                .import_view
                .objects
                .iter()
                .map(|object| PidBrowserListItem {
                    key: PidNodeKey::Object {
                        drawing_id: object.drawing_id.clone(),
                    },
                    title: object.drawing_id.clone(),
                    subtitle: Some(object.item_type.clone()),
                    badge: object.model_id.clone(),
                })
                .collect(),
            PidBrowserSection::Relationships => self
                .import_view
                .relationships
                .iter()
                .map(|relationship| PidBrowserListItem {
                    key: PidNodeKey::Relationship {
                        guid: relationship.guid.clone(),
                    },
                    title: relationship.guid.clone(),
                    subtitle: Some(format!(
                        "{} -> {}",
                        relationship
                            .source_drawing_id
                            .clone()
                            .unwrap_or_else(|| "?".into()),
                        relationship
                            .target_drawing_id
                            .clone()
                            .unwrap_or_else(|| "?".into())
                    )),
                    badge: Some(relationship.model_id.clone()),
                })
                .collect(),
            PidBrowserSection::Sheets => self
                .import_view
                .clusters
                .iter()
                .map(|cluster| {
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
                    PidBrowserListItem {
                        key,
                        title: cluster.name.clone(),
                        subtitle: Some(cluster.kind.clone()),
                        badge: Some(format!("{} rec", cluster.record_count)),
                    }
                })
                .collect(),
            PidBrowserSection::Streams => {
                let mut items = Vec::new();
                if let Some(dynamic) = &self.document.dynamic_attributes {
                    items.push(PidBrowserListItem {
                        key: PidNodeKey::DynamicAttributes,
                        title: "Dynamic Attributes".into(),
                        subtitle: Some(dynamic.path.clone()),
                        badge: Some(format!("{} records", dynamic.attribute_records.len())),
                    });
                }
                for sheet in &self.document.sheet_streams {
                    items.push(PidBrowserListItem {
                        key: PidNodeKey::Stream {
                            name: sheet.name.clone(),
                        },
                        title: sheet.name.clone(),
                        subtitle: Some(sheet.path.clone()),
                        badge: Some(format!("{} endpoints", sheet.endpoint_records.len())),
                    });
                }
                if let Some(tagged) = &self.document.tagged_storages {
                    for entry in &tagged.entries {
                        items.push(PidBrowserListItem {
                            key: PidNodeKey::TaggedStorage {
                                storage_name: entry.storage_name.clone(),
                            },
                            title: entry.storage_name.clone(),
                            subtitle: Some(tagged.list_name.clone()),
                            badge: Some("TaggedText".into()),
                        });
                    }
                }
                if let Some(cross) = &self.document.cross_reference {
                    if !cross.cluster_coverage.declared_missing.is_empty()
                        || !cross.cluster_coverage.found_extra.is_empty()
                    {
                        items.push(PidBrowserListItem {
                            key: PidNodeKey::ClusterCoverage,
                            title: "Cluster Coverage".into(),
                            subtitle: Some("declared vs found".into()),
                            badge: Some(format!(
                                "{} missing / {} extra",
                                cross.cluster_coverage.declared_missing.len(),
                                cross.cluster_coverage.found_extra.len()
                            )),
                        });
                    }
                }
                items
            }
            PidBrowserSection::CrossRef => {
                let mut items = Vec::new();
                for symbol in &self.import_view.symbols {
                    items.push(PidBrowserListItem {
                        key: PidNodeKey::Symbol {
                            symbol_path: symbol.symbol_path.clone(),
                        },
                        title: symbol
                            .symbol_name
                            .clone()
                            .unwrap_or_else(|| symbol.symbol_path.clone()),
                        subtitle: Some(symbol.symbol_path.clone()),
                        badge: Some(format!("{} use", symbol.usage_count)),
                    });
                }
                if let Some(cross) = &self.document.cross_reference {
                    for class in &cross.attribute_classes {
                        items.push(PidBrowserListItem {
                            key: PidNodeKey::AttributeClass {
                                class_name: class.class_name.clone(),
                            },
                            title: class.class_name.clone(),
                            subtitle: Some("Attribute Class".into()),
                            badge: Some(format!("{} rec", class.record_count)),
                        });
                    }
                    for root in &cross.root_presence {
                        items.push(PidBrowserListItem {
                            key: PidNodeKey::Root {
                                name: root.name.clone(),
                            },
                            title: root.name.clone(),
                            subtitle: Some("Root Presence".into()),
                            badge: Some(if root.found_as_storage || root.found_as_stream {
                                "ok".into()
                            } else {
                                "missing".into()
                            }),
                        });
                    }
                }
                for line in self.import_view.unresolved.iter().take(8) {
                    items.push(PidBrowserListItem {
                        key: PidNodeKey::Unresolved {
                            label: line.clone(),
                        },
                        title: "Unresolved".into(),
                        subtitle: Some(line.clone()),
                        badge: None,
                    });
                }
                items
            }
        };

        if !self.search_text.trim().is_empty() {
            let query = self.search_text.to_lowercase();
            items.retain(|item| {
                item.title.to_lowercase().contains(&query)
                    || item
                        .subtitle
                        .as_ref()
                        .map(|value| value.to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || item
                        .badge
                        .as_ref()
                        .map(|value| value.to_lowercase().contains(&query))
                        .unwrap_or(false)
            });
        }

        items
    }

    pub(super) fn empty_hint(&self) -> &'static str {
        match self.active_section {
            PidBrowserSection::Objects | PidBrowserSection::Relationships
                if !self.summary.object_graph_available =>
            {
                "Object graph unavailable; inspect Streams, TaggedText, DynamicAttrs, and CrossRef instead."
            }
            PidBrowserSection::CrossRef => "No cross-reference evidence available for this file.",
            _ => "No entries in this section.",
        }
    }

    pub(super) fn selected_handles(&self) -> Vec<Handle> {
        self.selected_key
            .as_ref()
            .map(|key| self.preview_index.handles_for(key))
            .unwrap_or_default()
    }
}

pub(super) struct DocumentTab {
    pub(super) tab_mode: DocumentTabMode,
    pub(super) pid_state: Option<PidTabState>,
    pub(super) scene: Scene,
    pub(super) native_render_enabled: bool,
    pub(super) current_path: Option<PathBuf>,
    pub(super) dirty: bool,
    pub(super) tab_title: String,
    pub(super) properties: PropertiesPanel,
    pub(super) layers: LayerPanel,
    pub(super) active_cmd: Option<Box<dyn CadCommand>>,
    pub(super) last_cmd: Option<String>,
    pub(super) snap_result: Option<SnapResult>,
    pub(super) active_grip: Option<GripEdit>,
    pub(super) selected_grips: Vec<GripDef>,
    pub(super) selected_handle: Option<Handle>,
    pub(super) wireframe: bool,
    pub(super) visual_style: String,
    pub(super) last_cursor_world: glam::Vec3,
    pub(super) last_cursor_screen: iced::Point,
    pub(super) history: HistoryState,
    pub(super) active_layer: String,
    /// Currently active UCS. `None` means WCS (identity transform).
    pub(super) active_ucs: Option<Ucs>,
    /// Custom model-space background color.  `None` = default dark grey.
    pub(super) bg_color: Option<[f32; 4]>,
    /// Custom paper-space background color.  `None` = default off-white grey.
    pub(super) paper_bg_color: Option<[f32; 4]>,
    /// Active REFEDIT session, if any.
    pub(super) refedit_session: Option<RefEditSession>,
    /// Currently active MLeader style name.
    pub(super) active_mleader_style: String,
}

impl DocumentTab {
    pub(super) fn new_drawing(n: usize) -> Self {
        let mut scene = Scene::new();
        linetypes::populate_document(&mut scene.document);
        Self {
            tab_mode: DocumentTabMode::Cad,
            pid_state: None,
            scene,
            native_render_enabled: false,
            current_path: None,
            dirty: false,
            tab_title: format!("Drawing{}", n),
            properties: PropertiesPanel::empty(),
            layers: LayerPanel::default(),
            active_cmd: None,
            last_cmd: None,
            snap_result: None,
            active_grip: None,
            selected_grips: vec![],
            selected_handle: None,
            wireframe: false,
            visual_style: "Shaded".into(),
            last_cursor_world: glam::Vec3::ZERO,
            last_cursor_screen: iced::Point::ORIGIN,
            history: HistoryState::default(),
            active_layer: "0".to_string(),
            active_ucs: None,
            bg_color: None,
            paper_bg_color: None,
            refedit_session: None,
            active_mleader_style: "Standard".to_string(),
        }
    }

    pub(super) fn is_pid(&self) -> bool {
        self.tab_mode == DocumentTabMode::Pid
    }

    pub(super) fn tab_display_name(&self) -> String {
        match &self.current_path {
            Some(p) => p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            None => self.tab_title.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct HistorySnapshot {
    pub(super) document: CadDocument,
    pub(super) native_doc_clone: Option<nm::CadDocument>,
    pub(super) current_layout: String,
    pub(super) selected: Vec<Handle>,
    pub(super) dirty: bool,
    pub(super) label: String,
}

#[derive(Default)]
pub(super) struct HistoryState {
    pub(super) undo_stack: Vec<HistorySnapshot>,
    pub(super) redo_stack: Vec<HistorySnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nativerender_new_drawing_disables_flag_by_default() {
        let tab = DocumentTab::new_drawing(1);
        assert!(!tab.native_render_enabled);
    }

    #[test]
    fn history_snapshot_clone_preserves_native_document() {
        let mut native = h7cad_native_model::CadDocument::new();
        native
            .add_entity(h7cad_native_model::Entity::new(
                h7cad_native_model::EntityData::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [1.0, 0.0, 0.0],
                },
            ))
            .expect("native line should be added");

        let snapshot = HistorySnapshot {
            document: CadDocument::new(),
            native_doc_clone: Some(native.clone()),
            current_layout: "Model".into(),
            selected: vec![],
            dirty: false,
            label: "test".into(),
        };

        let cloned = snapshot.clone();
        assert_eq!(cloned.native_doc_clone, Some(native));
    }
}
