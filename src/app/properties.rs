use super::{H7CAD, VARIES_LABEL};
use super::helpers::{
    entity_type_key, entity_type_key_native, entity_type_label, entity_type_label_native,
    title_case_word,
};
use crate::io::pid_import::PidNodeKey;
use crate::scene::dispatch;
use crate::scene::object::{PropSection, PropValue, Property};
use crate::ui;
use crate::linetypes;
use acadrust::{EntityType, Handle};
use h7cad_native_model as nm;

#[derive(Clone, Copy)]
enum SelectedEntityRef<'a> {
    Compat(Handle, &'a EntityType),
    Native(Handle, &'a nm::Entity),
}

impl SelectedEntityRef<'_> {
    fn handle(self) -> Handle {
        match self {
            Self::Compat(handle, _) | Self::Native(handle, _) => handle,
        }
    }

    fn title(self) -> String {
        match self {
            Self::Compat(_, entity) => entity_type_label(entity),
            Self::Native(_, entity) => entity_type_label_native(entity),
        }
    }

    fn type_key(self) -> String {
        match self {
            Self::Compat(_, entity) => entity_type_key(entity),
            Self::Native(_, entity) => entity_type_key_native(entity),
        }
    }

    fn sections(self, text_style_names: &[String]) -> Vec<crate::scene::object::PropSection> {
        match self {
            Self::Compat(handle, entity) => {
                dispatch::properties_sectioned(handle, entity, text_style_names)
            }
            Self::Native(handle, entity) => dispatch::properties_sectioned_native(
                nm::Handle::new(handle.value()),
                entity,
                text_style_names,
            ),
        }
    }
}

fn selected_entity_refs(scene: &crate::scene::Scene) -> Vec<SelectedEntityRef<'_>> {
    scene
        .selected
        .iter()
        .filter_map(|&handle| {
            scene
                .document
                .get_entity(handle)
                .map(|entity| SelectedEntityRef::Compat(handle, entity))
                .or_else(|| scene.native_entity(handle).map(|entity| SelectedEntityRef::Native(handle, entity)))
        })
        .collect()
}

fn inject_group_property(
    sections: &mut [crate::scene::object::PropSection],
    label: String,
) {
    if let Some(general) = sections.first_mut() {
        general.props.push(crate::scene::object::Property {
            label: "Group".to_string(),
            field: "group",
            value: crate::scene::object::PropValue::ReadOnly(label),
        });
    }
}

impl H7CAD {
    /// Rebuild the PropertiesPanel from the current entity selection.
    /// Preserves UI state (open pickers, edit buffer) across refreshes.
    pub(super) fn refresh_properties(&mut self) {
        let i = self.active_tab;
        if self.tabs[i].is_pid() {
            self.sync_pid_selection_from_scene(i);
            let panel = {
                let tab = &self.tabs[i];
                build_pid_properties_panel(tab)
            };
            self.tabs[i].properties = panel;
            self.refresh_selected_grips();
            return;
        }

        let color_picker_open = self.tabs[i].properties.color_picker_open;
        let color_palette_open = self.tabs[i].properties.color_palette_open;
        let edit_buf = std::mem::take(&mut self.tabs[i].properties.edit_buf);
        let selected_group = self.tabs[i].properties.selected_group.clone();

        let layer_names: Vec<String> = self.tabs[i]
            .scene
            .document
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        let linetype_items: Vec<ui::properties::LinetypeItem> = self.tabs[i]
            .scene
            .document
            .line_types
            .iter()
            .map(|lt| {
                let name = if lt.name.is_empty() {
                    "ByLayer".to_string()
                } else {
                    lt.name.clone()
                };
                let art = linetypes::extract_pattern(&lt.description);
                ui::properties::LinetypeItem { name, art }
            })
            .collect();
        let text_style_names: Vec<String> = self.tabs[i]
            .scene
            .document
            .text_styles
            .iter()
            .map(|style| style.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();

        let new_panel = {
            let selected = selected_entity_refs(&self.tabs[i].scene);
            let mut panel = match selected.len() {
                0 => ui::PropertiesPanel::empty(),
                1 => {
                    match selected[0] {
                        SelectedEntityRef::Compat(handle, entity) => {
                            let group_names = self.tabs[i].scene.group_names_for_entity(handle);
                            let mut sections =
                                dispatch::properties_sectioned(handle, entity, &text_style_names);

                            // Inject viewport-only properties that require doc access.
                            if let acadrust::EntityType::Viewport(vp) = entity {
                                let frozen_names: Vec<String> = vp
                                    .frozen_layers
                                    .iter()
                                    .filter_map(|&h| {
                                        self.tabs[i].scene.document.layers.iter()
                                            .find(|l| l.handle == h)
                                            .map(|l| l.name.clone())
                                    })
                                    .collect();

                                // Collect available UCS names for the name picker.
                                let ucs_names: Vec<String> = self.tabs[i].scene.document.ucss
                                    .iter()
                                    .map(|u| u.name.clone())
                                    .filter(|n| !n.is_empty())
                                    .collect();

                                // Current UCS name (resolved from vp.ucs_handle).
                                let current_ucs = self.tabs[i].scene.document.ucss
                                    .iter()
                                    .find(|u| u.handle == vp.ucs_handle)
                                    .map(|u| u.name.clone())
                                    .unwrap_or_default();

                                // Collect available named view names.
                                let view_names: Vec<String> = self.tabs[i].scene.document.views
                                    .iter()
                                    .map(|v| v.name.clone())
                                    .filter(|n| !n.is_empty())
                                    .collect();

                                if let Some(geom) = sections.last_mut() {
                                    geom.props.push(crate::scene::object::Property {
                                        label: "Frozen Layers".to_string(),
                                        field: "frozen_layers",
                                        value: crate::scene::object::PropValue::EditText(
                                            frozen_names.join(", "),
                                        ),
                                    });
                                    if !ucs_names.is_empty() {
                                        geom.props.push(crate::scene::object::Property {
                                            label: "UCS Name".to_string(),
                                            field: "vp_ucs_name",
                                            value: crate::scene::object::PropValue::Choice {
                                                selected: current_ucs,
                                                options: ucs_names,
                                            },
                                        });
                                    }
                                    if !view_names.is_empty() {
                                        geom.props.push(crate::scene::object::Property {
                                            label: "Named View".to_string(),
                                            field: "vp_named_view",
                                            value: crate::scene::object::PropValue::Choice {
                                                selected: String::new(),
                                                options: view_names,
                                            },
                                        });
                                    }
                                }
                            }

                            // Inject DimStyle picker for Dimension entities.
                            if let acadrust::EntityType::Dimension(_) = entity {
                                let dim_style_names: Vec<String> = self.tabs[i].scene.document.dim_styles
                                    .iter()
                                    .map(|s| s.name.clone())
                                    .filter(|n| !n.is_empty())
                                    .collect();
                                if !dim_style_names.is_empty() {
                                    // Current style is already shown as EditText in the geom section;
                                    // replace/upgrade it to a Choice if we have a list.
                                    if let Some(geom) = sections.last_mut() {
                                        // Find and replace the style_name EditText with a Choice.
                                        if let Some(prop) = geom.props.iter_mut().find(|p| p.field == "style_name") {
                                            let current = match &prop.value {
                                                crate::scene::object::PropValue::EditText(s) => s.clone(),
                                                _ => String::new(),
                                            };
                                            prop.value = crate::scene::object::PropValue::Choice {
                                                selected: current,
                                                options: dim_style_names,
                                            };
                                        }
                                    }
                                }
                            }

                            if !group_names.is_empty() {
                                inject_group_property(&mut sections, group_names.join(", "));
                            }
                            let title = selected[0].title();
                            ui::PropertiesPanel {
                                choice_combos: sections
                                    .iter()
                                    .flat_map(|section| section.props.iter())
                                    .filter_map(|prop| match &prop.value {
                                        crate::scene::object::PropValue::Choice { options, .. } => Some((
                                            prop.field.to_string(),
                                            iced::widget::combo_box::State::new(options.clone()),
                                        )),
                                        _ => None,
                                    })
                                    .collect(),
                                sections,
                                title,
                                layer_combo: iced::widget::combo_box::State::new(layer_names.clone()),
                                linetype_combo: iced::widget::combo_box::State::new(linetype_items.clone()),
                                hatch_pattern_combo: iced::widget::combo_box::State::new(
                                    crate::scene::hatch_patterns::names(),
                                ),
                                lineweight_combo: iced::widget::combo_box::State::new(
                                    ui::properties::lw_options(),
                                ),
                                linetype_items,
                                ..Default::default()
                            }
                        }
                        SelectedEntityRef::Native(handle, entity) => {
                            let group_names = self.tabs[i].scene.group_names_for_entity(handle);
                            let mut sections = dispatch::properties_sectioned_native(
                                nm::Handle::new(handle.value()),
                                entity,
                                &text_style_names,
                            );
                            if !group_names.is_empty() {
                                inject_group_property(&mut sections, group_names.join(", "));
                            }
                            let title = selected[0].title();
                            ui::PropertiesPanel {
                                choice_combos: sections
                                    .iter()
                                    .flat_map(|section| section.props.iter())
                                    .filter_map(|prop| match &prop.value {
                                        crate::scene::object::PropValue::Choice { options, .. } => Some((
                                            prop.field.to_string(),
                                            iced::widget::combo_box::State::new(options.clone()),
                                        )),
                                        _ => None,
                                    })
                                    .collect(),
                                sections,
                                title,
                                layer_combo: iced::widget::combo_box::State::new(layer_names.clone()),
                                linetype_combo: iced::widget::combo_box::State::new(linetype_items.clone()),
                                hatch_pattern_combo: iced::widget::combo_box::State::new(
                                    crate::scene::hatch_patterns::names(),
                                ),
                                lineweight_combo: iced::widget::combo_box::State::new(
                                    ui::properties::lw_options(),
                                ),
                                linetype_items,
                                ..Default::default()
                            }
                        }
                    }
                }
                _ => {
                    let groups = build_selection_groups(&selected);
                    let active_group = selected_group
                        .and_then(|group| groups.iter().find(|g| g.label == group.label).cloned())
                        .or_else(|| groups.first().cloned());

                    let filtered: Vec<SelectedEntityRef<'_>> = active_group
                        .as_ref()
                        .map(|group| {
                            selected
                                .iter()
                                .filter(|entity| group.handles.contains(&entity.handle()))
                                .copied()
                                .collect()
                        })
                        .unwrap_or_default();

                    let sections = aggregate_sections(&filtered, &text_style_names);
                    ui::PropertiesPanel {
                        choice_combos: sections
                            .iter()
                            .flat_map(|section| section.props.iter())
                            .filter_map(|prop| match &prop.value {
                                crate::scene::object::PropValue::Choice { options, .. } => Some((
                                    prop.field.to_string(),
                                    iced::widget::combo_box::State::new(options.clone()),
                                )),
                                _ => None,
                            })
                            .collect(),
                        sections,
                        title: format!("{} objects selected", selected.len()),
                        selection_group_combo: iced::widget::combo_box::State::new(groups.clone()),
                        selection_groups: groups,
                        selected_group: active_group,
                        layer_combo: iced::widget::combo_box::State::new(layer_names.clone()),
                        linetype_combo: iced::widget::combo_box::State::new(linetype_items.clone()),
                        hatch_pattern_combo: iced::widget::combo_box::State::new(
                            crate::scene::hatch_patterns::names(),
                        ),
                        lineweight_combo: iced::widget::combo_box::State::new(
                            ui::properties::lw_options(),
                        ),
                        linetype_items,
                        ..Default::default()
                    }
                }
            };
            panel.color_picker_open = color_picker_open;
            panel.color_palette_open = color_palette_open;
            panel.edit_buf = edit_buf;
            panel
        };

        self.tabs[i].properties = new_panel;
        self.refresh_selected_grips();
    }

    /// Rebuild the cached selected_grips from the current entity selection.
    pub(super) fn refresh_selected_grips(&mut self) {
        let i = self.active_tab;
        if self.tabs[i].is_pid() {
            self.tabs[i].selected_handle = None;
            self.tabs[i].selected_grips.clear();
            return;
        }
        let (new_handle, new_grips) = {
            let selected = selected_entity_refs(&self.tabs[i].scene);
            if selected.len() == 1 {
                match selected[0] {
                    SelectedEntityRef::Compat(handle, entity) => {
                        (Some(handle), dispatch::grips(entity))
                    }
                    SelectedEntityRef::Native(handle, entity) => {
                        (Some(handle), dispatch::grips_native(entity))
                    }
                }
            } else {
                (None, vec![])
            }
        };
        self.tabs[i].selected_handle = new_handle;
        self.tabs[i].selected_grips = new_grips;
    }

    pub(super) fn property_target_handles(&self, i: usize) -> Vec<Handle> {
        if self.tabs[i].is_pid() {
            return vec![];
        }
        let handles = self.tabs[i].properties.selected_handles();
        if !handles.is_empty() {
            handles
        } else {
            self.tabs[i].selected_handle.into_iter().collect()
        }
    }

    /// Add an entity to the correct space (model or paper space layout).
    pub(super) fn commit_entity(&mut self, mut entity: acadrust::EntityType) {
        let i = self.active_tab;
        let layer = &self.tabs[i].active_layer;
        if layer != "0" || entity.as_entity().layer().is_empty() {
            entity.as_entity_mut().set_layer(layer.clone());
        }

        crate::scene::dispatch::apply_color(&mut entity, self.ribbon.active_color);
        crate::scene::dispatch::apply_common_prop(
            &mut entity,
            "linetype",
            &self.ribbon.active_linetype.clone(),
        );
        crate::scene::dispatch::apply_line_weight(&mut entity, self.ribbon.active_lineweight);

        if matches!(&entity, acadrust::EntityType::Viewport(_))
            && self.tabs[i].scene.current_layout != "Model"
        {
            // Assign a unique viewport ID (max existing id + 1, min 2).
            if let acadrust::EntityType::Viewport(ref mut vp) = entity {
                let layout_block = self.tabs[i].scene.current_layout_block_handle_pub();
                let max_id = self.tabs[i]
                    .scene
                    .document
                    .entities()
                    .filter_map(|e| {
                        if let acadrust::EntityType::Viewport(v) = e {
                            if v.common.owner_handle == layout_block { Some(v.id) } else { None }
                        } else {
                            None
                        }
                    })
                    .max()
                    .unwrap_or(1);
                vp.id = (max_id + 1).max(2);
            }

            let new_handle = self.tabs[i].scene.add_entity(entity);
            if !new_handle.is_null() {
                // Auto-fit the new viewport to show model-space content.
                self.tabs[i].scene.auto_fit_viewport(new_handle);
            } else {
                self.command_line
                    .push_error("Viewport could not be added.");
            }
        } else {
            self.tabs[i].scene.add_entity(entity);
        }
    }

    fn sync_pid_selection_from_scene(&mut self, i: usize) {
        let scene_selection: Vec<Handle> = self.tabs[i].scene.selected.iter().copied().collect();
        let Some(pid_state) = self.tabs[i].pid_state.as_mut() else {
            return;
        };

        if scene_selection.is_empty() {
            let keep_non_graphic = pid_state
                .selected_key
                .as_ref()
                .map(|key| pid_state.preview_index.handles_for(key).is_empty())
                .unwrap_or(false);
            if !keep_non_graphic {
                pid_state.selected_key = None;
            }
            return;
        }

        let next_key = scene_selection
            .iter()
            .find_map(|handle| pid_state.preview_index.key_for_handle(*handle).cloned());
        if let Some(key) = next_key {
            pid_state.active_section = pid_section_for_key(&key);
            pid_state.selected_key = Some(key);
        }
    }
}

// ── Multi-selection property aggregation ───────────────────────────────────

fn build_selection_groups(
    selected: &[SelectedEntityRef<'_>],
) -> Vec<ui::properties::SelectionGroup> {
    let mut groups = vec![ui::properties::SelectionGroup {
        label: format!("All({})", selected.len()),
        handles: selected.iter().map(|entity| entity.handle()).collect(),
    }];

    let mut by_type: std::collections::BTreeMap<String, Vec<Handle>> =
        std::collections::BTreeMap::new();
    for entity in selected {
        by_type
            .entry(entity.type_key())
            .or_default()
            .push(entity.handle());
    }

    for (kind, handles) in by_type {
        groups.push(ui::properties::SelectionGroup {
            label: format!("{}({})", title_case_word(&kind), handles.len()),
            handles,
        });
    }

    groups
}

fn aggregate_sections(
    selected: &[SelectedEntityRef<'_>],
    text_style_names: &[String],
) -> Vec<crate::scene::object::PropSection> {
    if selected.is_empty() {
        return vec![];
    }

    let mut all_sections: Vec<Vec<crate::scene::object::PropSection>> = selected
        .iter()
        .map(|entity| entity.sections(text_style_names))
        .collect();

    let mut result = all_sections.remove(0);
    for sections in all_sections {
        result = merge_sections(&result, &sections);
    }
    result
}

fn merge_sections(
    left: &[crate::scene::object::PropSection],
    right: &[crate::scene::object::PropSection],
) -> Vec<crate::scene::object::PropSection> {
    left.iter()
        .filter_map(|section| {
            let rhs = right.iter().find(|candidate| candidate.title == section.title)?;
            let props: Vec<crate::scene::object::Property> = section
                .props
                .iter()
                .filter_map(|prop| {
                    let other =
                        rhs.props.iter().find(|candidate| candidate.field == prop.field)?;
                    Some(crate::scene::object::Property {
                        label: prop.label.clone(),
                        field: prop.field,
                        value: merge_prop_value(&prop.value, &other.value),
                    })
                })
                .collect();
            if props.is_empty() {
                None
            } else {
                Some(crate::scene::object::PropSection {
                    title: section.title.clone(),
                    props,
                })
            }
        })
        .collect()
}

fn merge_prop_value(
    left: &crate::scene::object::PropValue,
    right: &crate::scene::object::PropValue,
) -> crate::scene::object::PropValue {
    use crate::scene::object::PropValue;

    if left == right {
        return left.clone();
    }

    match (left, right) {
        (PropValue::LayerChoice(_), PropValue::LayerChoice(_)) => {
            PropValue::LayerChoice(VARIES_LABEL.into())
        }
        (PropValue::ColorChoice(_), PropValue::ColorChoice(_))
        | (PropValue::ColorVaries, _)
        | (_, PropValue::ColorVaries) => PropValue::ColorVaries,
        (PropValue::LwChoice(_), PropValue::LwChoice(_))
        | (PropValue::LwVaries, _)
        | (_, PropValue::LwVaries) => PropValue::LwVaries,
        (PropValue::LinetypeChoice(_), PropValue::LinetypeChoice(_)) => {
            PropValue::LinetypeChoice(VARIES_LABEL.into())
        }
        (
            PropValue::Choice { options, .. },
            PropValue::Choice {
                options: other_options,
                ..
            },
        ) if options == other_options => PropValue::Choice {
            selected: VARIES_LABEL.into(),
            options: options.clone(),
        },
        (PropValue::EditText(_), PropValue::EditText(_)) => {
            PropValue::EditText(VARIES_LABEL.into())
        }
        (PropValue::ReadOnly(_), PropValue::ReadOnly(_)) => {
            PropValue::ReadOnly(VARIES_LABEL.into())
        }
        (PropValue::HatchPatternChoice(_), PropValue::HatchPatternChoice(_)) => {
            PropValue::HatchPatternChoice(VARIES_LABEL.into())
        }
        (
            PropValue::BoolToggle { field, .. },
            PropValue::BoolToggle {
                field: other_field,
                ..
            },
        ) if field == other_field => PropValue::ReadOnly(VARIES_LABEL.into()),
        _ => left.clone(),
    }
}

fn build_pid_properties_panel(tab: &super::document::DocumentTab) -> ui::PropertiesPanel {
    let Some(pid_state) = tab.pid_state.as_ref() else {
        return ui::PropertiesPanel::empty().with_width(300.0);
    };

    let (title, sections) = match pid_state.selected_key.as_ref() {
        Some(key) => pid_sections_for_key(pid_state, key),
        None => ("P&ID Overview".to_string(), pid_overview_sections(pid_state)),
    };

    ui::PropertiesPanel {
        title,
        sections,
        ..Default::default()
    }
    .with_width(300.0)
}

fn pid_section_for_key(key: &PidNodeKey) -> crate::ui::PidBrowserSection {
    match key {
        PidNodeKey::Overview => crate::ui::PidBrowserSection::Overview,
        PidNodeKey::Object { .. } => crate::ui::PidBrowserSection::Objects,
        PidNodeKey::Relationship { .. } => crate::ui::PidBrowserSection::Relationships,
        PidNodeKey::Sheet { .. } => crate::ui::PidBrowserSection::Sheets,
        PidNodeKey::Stream { .. }
        | PidNodeKey::TaggedStorage { .. }
        | PidNodeKey::DynamicAttributes
        | PidNodeKey::ClusterCoverage => crate::ui::PidBrowserSection::Streams,
        PidNodeKey::Cluster { .. } => crate::ui::PidBrowserSection::Sheets,
        PidNodeKey::Symbol { .. }
        | PidNodeKey::AttributeClass { .. }
        | PidNodeKey::Root { .. }
        | PidNodeKey::Unresolved { .. } => crate::ui::PidBrowserSection::CrossRef,
    }
}

fn pid_sections_for_key(
    pid_state: &super::document::PidTabState,
    key: &PidNodeKey,
) -> (String, Vec<PropSection>) {
    match key {
        PidNodeKey::Overview => ("P&ID Overview".into(), pid_overview_sections(pid_state)),
        PidNodeKey::Object { drawing_id } => {
            let title = format!("Object {}", short_id(drawing_id));
            let Some(object) = pid_state
                .document
                .object_graph
                .as_ref()
                .and_then(|graph| graph.objects.iter().find(|item| item.drawing_id == *drawing_id))
            else {
                return (title, vec![ro_section("Object", vec![ro_prop("Drawing ID", drawing_id.clone())])]);
            };

            let details = vec![
                ro_prop("Drawing ID", object.drawing_id.clone()),
                ro_prop("Item Type", object.item_type.clone()),
                ro_prop(
                    "Drawing Item Type",
                    object
                        .drawing_item_type
                        .clone()
                        .unwrap_or_else(|| "-".into()),
                ),
                ro_prop("Model ID", object.model_id.clone().unwrap_or_else(|| "-".into())),
                ro_prop(
                    "Record ID",
                    object
                        .record_id
                        .map(|id| format!("0x{id:08X}"))
                        .unwrap_or_else(|| "-".into()),
                ),
                ro_prop(
                    "Field X",
                    object
                        .field_x
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".into()),
                ),
            ];
            let mut attrs = Vec::new();
            for (name, value) in &object.extra {
                attrs.push(ro_prop(name, value.clone()));
            }
            if attrs.is_empty() {
                attrs.push(ro_prop("Attributes", "None".into()));
            }
            let mut sections = vec![ro_section("Object", details), ro_section("Attributes", attrs)];
            if let Some(layout_item) = pid_layout_item_for_object(pid_state, drawing_id) {
                let mut symbol_props = Vec::new();
                symbol_props.push(ro_prop(
                    "Layout ID",
                    layout_item.layout_id.clone(),
                ));
                symbol_props.push(ro_prop(
                    "Symbol Name",
                    layout_item
                        .symbol_name
                        .clone()
                        .unwrap_or_else(|| "-".into()),
                ));
                symbol_props.push(ro_prop(
                    "Symbol Path",
                    layout_item
                        .symbol_path
                        .clone()
                        .unwrap_or_else(|| "-".into()),
                ));
                symbol_props.push(ro_prop(
                    "Graphic OID",
                    layout_item
                        .graphic_oid
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".into()),
                ));
                sections.push(ro_section("Symbol Evidence", symbol_props));
            }
            (title, sections)
        }
        PidNodeKey::Relationship { guid } => {
            let title = format!("Relationship {}", short_id(guid));
            let Some(relationship) = pid_state
                .document
                .object_graph
                .as_ref()
                .and_then(|graph| graph.relationships.iter().find(|item| item.guid == *guid))
            else {
                return (
                    title,
                    vec![ro_section("Relationship", vec![ro_prop("GUID", guid.clone())])],
                );
            };
            (
                title,
                vec![ro_section(
                    "Relationship",
                    vec![
                        ro_prop("GUID", relationship.guid.clone()),
                        ro_prop("Model ID", relationship.model_id.clone()),
                        ro_prop(
                            "Record ID",
                            relationship
                                .record_id
                                .map(|id| format!("0x{id:08X}"))
                                .unwrap_or_else(|| "-".into()),
                        ),
                        ro_prop(
                            "Field X",
                            relationship
                                .field_x
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".into()),
                        ),
                        ro_prop(
                            "Source",
                            relationship
                                .source_drawing_id
                                .clone()
                                .unwrap_or_else(|| "-".into()),
                        ),
                        ro_prop(
                            "Target",
                            relationship
                                .target_drawing_id
                                .clone()
                                .unwrap_or_else(|| "-".into()),
                        ),
                    ],
                )],
            )
        }
        PidNodeKey::Sheet { name } => {
            let title = format!("Sheet {name}");
            let Some(sheet) = pid_state
                .document
                .sheet_streams
                .iter()
                .find(|sheet| sheet.name == *name)
            else {
                return (title, vec![ro_section("Sheet", vec![ro_prop("Name", name.clone())])]);
            };
            (
                title,
                vec![ro_section(
                    "Sheet",
                    vec![
                        ro_prop("Name", sheet.name.clone()),
                        ro_prop("Path", sheet.path.clone()),
                        ro_prop("Size", sheet.size.to_string()),
                        ro_prop(
                            "Magic",
                            sheet
                                .magic_tag
                                .clone()
                                .or_else(|| sheet.magic_u32_le.map(|tag| format!("0x{tag:08X}")))
                                .unwrap_or_else(|| "-".into()),
                        ),
                        ro_prop("Endpoints", sheet.endpoint_records.len().to_string()),
                        ro_prop("Attributes", sheet.attribute_records.len().to_string()),
                    ],
                )],
            )
        }
        PidNodeKey::Stream { name } => {
            let title = format!("Stream {name}");
            if let Some(sheet) = pid_state
                .document
                .sheet_streams
                .iter()
                .find(|sheet| sheet.name == *name)
            {
                return (
                    title,
                    vec![ro_section(
                        "Stream",
                        vec![
                            ro_prop("Name", sheet.name.clone()),
                            ro_prop("Path", sheet.path.clone()),
                            ro_prop("Size", sheet.size.to_string()),
                            ro_prop("Endpoint Records", sheet.endpoint_records.len().to_string()),
                            ro_prop("Attribute Records", sheet.attribute_records.len().to_string()),
                            ro_prop("Preview Text Count", sheet.extracted_texts.len().to_string()),
                        ],
                    )],
                );
            }
            (title, vec![ro_section("Stream", vec![ro_prop("Name", name.clone())])])
        }
        PidNodeKey::Cluster { name } => {
            let title = format!("Cluster {name}");
            let Some(cluster) = pid_state
                .document
                .clusters
                .iter()
                .find(|cluster| cluster.name == *name)
            else {
                return (
                    title,
                    vec![ro_section("Cluster", vec![ro_prop("Name", name.clone())])],
                );
            };
            (
                title,
                vec![ro_section(
                    "Cluster",
                    vec![
                        ro_prop("Name", cluster.name.clone()),
                        ro_prop("Path", cluster.path.clone()),
                        ro_prop("Kind", format!("{:?}", cluster.kind)),
                        ro_prop("Size", cluster.size.to_string()),
                        ro_prop(
                            "Header",
                            cluster
                                .header
                                .as_ref()
                                .map(|header| format!("type=0x{:04X}", header.stream_type))
                                .unwrap_or_else(|| "None".into()),
                        ),
                        ro_prop(
                            "Strings",
                            cluster
                                .string_table
                                .as_ref()
                                .map(|table| table.len())
                                .unwrap_or(cluster.extracted_strings.len())
                                .to_string(),
                        ),
                    ],
                )],
            )
        }
        PidNodeKey::Symbol { symbol_path } => {
            let title = format!("Symbol {}", short_id(symbol_path));
            let Some(symbol) = pid_state
                .document
                .cross_reference
                .as_ref()
                .and_then(|cross| {
                    cross
                        .symbol_usage
                        .iter()
                        .find(|usage| usage.symbol_path == *symbol_path)
                })
            else {
                return (
                    title,
                    vec![ro_section("Symbol", vec![ro_prop("Path", symbol_path.clone())])],
                );
            };

            let mut jsites = symbol
                .jsite_names
                .iter()
                .take(8)
                .cloned()
                .map(|name| ro_prop("JSite", name))
                .collect::<Vec<_>>();
            if jsites.is_empty() {
                jsites.push(ro_prop("JSite", "None".into()));
            }
            (
                title,
                vec![
                    ro_section(
                        "Symbol",
                        vec![
                            ro_prop(
                                "Name",
                                symbol.symbol_name.clone().unwrap_or_else(|| "-".into()),
                            ),
                            ro_prop("Path", symbol.symbol_path.clone()),
                            ro_prop("Usage Count", symbol.usage_count.to_string()),
                        ],
                    ),
                    ro_section("JSites", jsites),
                ],
            )
        }
        PidNodeKey::AttributeClass { class_name } => {
            let title = format!("Class {class_name}");
            let Some(class) = pid_state
                .document
                .cross_reference
                .as_ref()
                .and_then(|cross| {
                    cross
                        .attribute_classes
                        .iter()
                        .find(|class| class.class_name == *class_name)
                })
            else {
                return (
                    title,
                    vec![ro_section("Class", vec![ro_prop("Name", class_name.clone())])],
                );
            };

            (
                title,
                vec![
                    ro_section(
                        "Class",
                        vec![
                            ro_prop("Name", class.class_name.clone()),
                            ro_prop("Record Count", class.record_count.to_string()),
                            ro_prop("Drawing IDs", class.drawing_ids.len().to_string()),
                            ro_prop("Model IDs", class.model_ids.len().to_string()),
                        ],
                    ),
                    ro_section(
                        "Attributes",
                        if class.unique_attribute_names.is_empty() {
                            vec![ro_prop("Attribute", "None".into())]
                        } else {
                            class
                                .unique_attribute_names
                                .iter()
                                .take(10)
                                .cloned()
                                .map(|name| ro_prop("Attribute", name))
                                .collect()
                        },
                    ),
                ],
            )
        }
        PidNodeKey::Root { name } => {
            let title = format!("Root {name}");
            let Some(root) = pid_state
                .document
                .cross_reference
                .as_ref()
                .and_then(|cross| cross.root_presence.iter().find(|root| root.name == *name))
            else {
                return (title, vec![ro_section("Root", vec![ro_prop("Name", name.clone())])]);
            };
            (
                title,
                vec![ro_section(
                    "Root",
                    vec![
                        ro_prop("Name", root.name.clone()),
                        ro_prop("ID", format!("0x{:08X}", root.id)),
                        ro_prop("Found As Storage", yes_no(root.found_as_storage)),
                        ro_prop("Found As Stream", yes_no(root.found_as_stream)),
                    ],
                )],
            )
        }
        PidNodeKey::TaggedStorage { storage_name } => {
            let title = format!("TaggedText {storage_name}");
            let Some(tagged) = pid_state.document.tagged_storages.as_ref() else {
                return (
                    title,
                    vec![ro_section("TaggedText", vec![ro_prop("Storage", storage_name.clone())])],
                );
            };
            (
                title,
                vec![ro_section(
                    "TaggedText",
                    vec![
                        ro_prop("List", tagged.list_name.clone()),
                        ro_prop("Storage", storage_name.clone()),
                        ro_prop("Entry Count", tagged.entries.len().to_string()),
                    ],
                )],
            )
        }
        PidNodeKey::DynamicAttributes => {
            let Some(dynamic) = pid_state.document.dynamic_attributes.as_ref() else {
                return (
                    "Dynamic Attributes".into(),
                    vec![ro_section("Dynamic Attributes", vec![ro_prop("Status", "Unavailable".into())])],
                );
            };
            (
                "Dynamic Attributes".into(),
                vec![
                    ro_section(
                        "Dynamic Attributes",
                        vec![
                            ro_prop("Path", dynamic.path.clone()),
                            ro_prop("Size", dynamic.size.to_string()),
                            ro_prop("Class Names", dynamic.class_names.len().to_string()),
                            ro_prop("Attribute Records", dynamic.attribute_records.len().to_string()),
                            ro_prop("Record Trailers", dynamic.record_trailers.len().to_string()),
                            ro_prop("Relationship Probes", dynamic.relationship_probes.len().to_string()),
                        ],
                    ),
                    ro_section(
                        "Class Names",
                        if dynamic.class_names.is_empty() {
                            vec![ro_prop("Class", "None".into())]
                        } else {
                            dynamic.class_names.iter().take(12).map(|name| ro_prop("Class", name.clone())).collect()
                        },
                    ),
                ],
            )
        }
        PidNodeKey::ClusterCoverage => {
            let Some(cross) = pid_state.document.cross_reference.as_ref() else {
                return (
                    "Cluster Coverage".into(),
                    vec![ro_section("Coverage", vec![ro_prop("Status", "Unavailable".into())])],
                );
            };
            (
                "Cluster Coverage".into(),
                vec![
                    ro_section(
                        "Coverage",
                        vec![
                            ro_prop("Declared", cross.cluster_coverage.declared.len().to_string()),
                            ro_prop("Found", cross.cluster_coverage.found.len().to_string()),
                            ro_prop("Matched", cross.cluster_coverage.matched.len().to_string()),
                            ro_prop(
                                "Declared Missing",
                                cross.cluster_coverage.declared_missing.len().to_string(),
                            ),
                            ro_prop("Found Extra", cross.cluster_coverage.found_extra.len().to_string()),
                        ],
                    ),
                    ro_section(
                        "Names",
                        if cross.cluster_coverage.declared_missing.is_empty()
                            && cross.cluster_coverage.found_extra.is_empty()
                        {
                            vec![ro_prop("Status", "No mismatch".into())]
                        } else {
                            cross
                                .cluster_coverage
                                .declared_missing
                                .iter()
                                .chain(cross.cluster_coverage.found_extra.iter())
                                .take(10)
                                .cloned()
                                .map(|name| ro_prop("Entry", name))
                                .collect()
                        },
                    ),
                ],
            )
        }
        PidNodeKey::Unresolved { label } => (
            "Unresolved".into(),
            vec![ro_section("Evidence", vec![ro_prop("Message", label.clone())])],
        ),
    }
}

fn pid_layout_item_for_object<'a>(
    pid_state: &'a super::document::PidTabState,
    drawing_id: &str,
) -> Option<&'a pid_parse::PidLayoutItem> {
    pid_state
        .document
        .layout
        .as_ref()
        .and_then(|layout| {
            layout
                .items
                .iter()
                .find(|item| item.drawing_id.as_deref() == Some(drawing_id))
        })
}

fn pid_overview_sections(pid_state: &super::document::PidTabState) -> Vec<PropSection> {
    let summary = &pid_state.summary;
    let mut document_props = vec![
        ro_prop("Title", summary.title.clone()),
        ro_prop("Object Graph", yes_no(summary.object_graph_available)),
        ro_prop(
            "Drawing Number",
            pid_state
                .document
                .drawing_meta
                .as_ref()
                .and_then(|meta| meta.drawing_number.clone())
                .unwrap_or_else(|| "-".into()),
        ),
        ro_prop(
            "Project Number",
            pid_state
                .import_view
                .project_number
                .clone()
                .unwrap_or_else(|| "-".into()),
        ),
    ];

    if let Some(summary_info) = &pid_state.document.summary {
        if let Some(created) = &summary_info.created_time {
            document_props.push(ro_prop("Created", created.clone()));
        }
        if let Some(modified) = &summary_info.modified_time {
            document_props.push(ro_prop("Modified", modified.clone()));
        }
    }

    let mut sections = vec![
        ro_section("Document", document_props),
        ro_section(
            "Counts",
            vec![
                ro_prop("Objects", summary.object_count.to_string()),
                ro_prop("Relationships", summary.relationship_count.to_string()),
                ro_prop("Symbols", summary.symbol_count.to_string()),
                ro_prop("Clusters", summary.cluster_count.to_string()),
                ro_prop("Sheets", summary.sheet_count.to_string()),
                ro_prop("Streams", summary.stream_count.to_string()),
                ro_prop("Attribute Classes", summary.attribute_class_count.to_string()),
                ro_prop("TaggedText", summary.tagged_text_count.to_string()),
                ro_prop(
                    "Dynamic Attribute Records",
                    summary.dynamic_attribute_record_count.to_string(),
                ),
                ro_prop(
                    "Unresolved",
                    summary.unresolved_relationship_count.to_string(),
                ),
            ],
        ),
    ];

    if let Some(drawing_meta) = &pid_state.document.drawing_meta {
        let mut meta_props = Vec::new();
        if let Some(category) = &drawing_meta.document_category {
            meta_props.push(ro_prop("Category", category.clone()));
        }
        if let Some(template) = &drawing_meta.template_name {
            meta_props.push(ro_prop("Template", template.clone()));
        }
        if let Some(symbology) = &drawing_meta.symbology_uid {
            meta_props.push(ro_prop("Symbology UID", symbology.clone()));
        }
        if !meta_props.is_empty() {
            sections.push(ro_section("Drawing Meta", meta_props));
        }
    }

    if let Some(general_meta) = &pid_state.document.general_meta {
        let mut general_props = Vec::new();
        if let Some(path) = &general_meta.file_path {
            general_props.push(ro_prop("File Path", path.clone()));
        }
        if let Some(size) = &general_meta.file_size {
            general_props.push(ro_prop("File Size", size.clone()));
        }
        if !general_props.is_empty() {
            sections.push(ro_section("General Meta", general_props));
        }
    }

    // Version History
    if let Some(history) = &pid_state.document.version_history {
        let mut history_props = vec![];
        for record in history.records.iter().take(6) {
            history_props.push(ro_prop(
                &record.operation,
                format!("{} {} {}", record.timestamp, record.product, record.version),
            ));
        }
        if !history_props.is_empty() {
            sections.push(ro_section("Version History", history_props));
        }
    }

    // PSM Roots
    if let Some(roots) = &pid_state.document.psm_roots {
        let root_props: Vec<Property> = roots
            .entries
            .iter()
            .take(8)
            .map(|entry| ro_prop(&entry.name, format!("id=0x{:08X}", entry.id)))
            .collect();
        if !root_props.is_empty() {
            sections.push(ro_section("PSM Roots", root_props));
        }
    }

    // PSM Cluster Table
    if let Some(table) = &pid_state.document.psm_cluster_table {
        let cluster_props: Vec<Property> = table
            .entries
            .iter()
            .take(8)
            .map(|entry| ro_prop("Cluster", entry.name.clone()))
            .collect();
        if !cluster_props.is_empty() {
            sections.push(ro_section("PSM Cluster Table", cluster_props));
        }
    }

    // Drawing Metadata tags
    if let Some(meta) = &pid_state.document.drawing_meta {
        let mut meta_props = vec![];
        for (key, value) in meta.tags.iter().take(10) {
            meta_props.push(ro_prop(key, value.clone()));
        }
        if !meta_props.is_empty() {
            sections.push(ro_section("Drawing Metadata", meta_props));
        }
    }

    // General Metadata tags
    if let Some(meta) = &pid_state.document.general_meta {
        let mut meta_props = vec![];
        for (key, value) in meta.tags.iter().take(10) {
            meta_props.push(ro_prop(key, value.clone()));
        }
        if !meta_props.is_empty() {
            sections.push(ro_section("General Metadata", meta_props));
        }
    }

    // App Object Registry
    if let Some(registry) = &pid_state.document.app_object_registry {
        let mut reg_props = vec![];
        for entry in registry.entries.iter().take(6) {
            reg_props.push(ro_prop(
                &entry.clsid[..8.min(entry.clsid.len())],
                entry.path.clone(),
            ));
        }
        if !reg_props.is_empty() {
            sections.push(ro_section("App Object Registry", reg_props));
        }
    }

    sections
}

fn ro_section(title: &str, props: Vec<Property>) -> PropSection {
    PropSection {
        title: title.to_string(),
        props,
    }
}

fn ro_prop(label: impl Into<String>, value: String) -> Property {
    Property {
        label: label.into(),
        field: "pid_readonly",
        value: PropValue::ReadOnly(value),
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "Yes".into()
    } else {
        "No".into()
    }
}

fn short_id(value: &str) -> String {
    value.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::document::{DocumentTabMode, PidTabState};
    use crate::io::pid_import::{PidImportSummary, PidNodeKey, PidPreviewIndex};
    use h7cad_native_model as nm;
    use pid_parse::{
        build_import_view, ObjectGraph, PidDocument, PidLayoutItem, PidLayoutModel, PidObject,
        PidRelationship,
    };

    #[test]
    fn refresh_properties_uses_native_entity_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![
                            [0.0, 0.0, 0.0],
                            [2.0, 0.0, 0.0],
                            [2.0, 2.0, 0.0],
                            [0.0, 2.0, 0.0],
                        ],
                    }],
                }],
            }))
            .expect("native hatch");

        app.tabs[0].scene.set_native_doc(Some(native));
        app.tabs[0]
            .scene
            .select_entity(Handle::new(handle.value()), true);

        app.refresh_properties();

        assert_eq!(app.tabs[0].properties.title, "Hatch");
        assert_eq!(app.tabs[0].properties.sections.len(), 2);
        assert!(
            app.tabs[0]
                .properties
                .sections
                .iter()
                .any(|section| section.title == "Geometry"),
            "native hatch selection should populate geometry properties"
        );
    }

    #[test]
    fn refresh_selected_grips_uses_native_entity_when_compat_missing() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("native line");

        app.tabs[0].scene.set_native_doc(Some(native));
        app.tabs[0]
            .scene
            .select_entity(Handle::new(handle.value()), true);

        app.refresh_selected_grips();

        assert_eq!(app.tabs[0].selected_handle, Some(Handle::new(handle.value())));
        assert!(
            !app.tabs[0].selected_grips.is_empty(),
            "native line selection should expose grips"
        );
    }

    #[test]
    fn refresh_properties_groups_mixed_native_only_selection() {
        let mut app = H7CAD::new();
        let mut native = nm::CadDocument::new();
        let line_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Line {
                start: [0.0, 0.0, 0.0],
                end: [5.0, 0.0, 0.0],
            }))
            .expect("native line");
        let hatch_handle = native
            .add_entity(nm::Entity::new(nm::EntityData::Hatch {
                pattern_name: "SOLID".into(),
                solid_fill: true,
                boundary_paths: vec![nm::HatchBoundaryPath {
                    flags: 2,
                    edges: vec![nm::HatchEdge::Polyline {
                        closed: true,
                        vertices: vec![
                            [0.0, 0.0, 0.0],
                            [2.0, 0.0, 0.0],
                            [2.0, 2.0, 0.0],
                            [0.0, 2.0, 0.0],
                        ],
                    }],
                }],
            }))
            .expect("native hatch");

        app.tabs[0].scene.set_native_doc(Some(native));
        app.tabs[0]
            .scene
            .select_entity(Handle::new(line_handle.value()), false);
        app.tabs[0]
            .scene
            .select_entity(Handle::new(hatch_handle.value()), false);

        app.refresh_properties();

        let labels: Vec<String> = app.tabs[0]
            .properties
            .selection_groups
            .iter()
            .map(|group| group.label.clone())
            .collect();
        assert_eq!(app.tabs[0].properties.title, "2 objects selected");
        assert!(labels.iter().any(|label| label == "All(2)"));
        assert!(labels.iter().any(|label| label == "Line(1)"));
        assert!(labels.iter().any(|label| label == "Hatch(1)"));
        assert!(
            !app.tabs[0].properties.sections.is_empty(),
            "mixed native selection should still aggregate shared sections"
        );
    }

    #[test]
    fn commit_entity_syncs_native_viewport_in_paper_space() {
        let mut app = H7CAD::new();
        let native = crate::io::native_bridge::acadrust_doc_to_native(&app.tabs[0].scene.document);
        app.tabs[0].scene.set_native_doc(Some(native));
        app.tabs[0].scene.current_layout = "Layout1".into();

        let viewport = acadrust::EntityType::Viewport(acadrust::entities::Viewport::new());
        app.commit_entity(viewport);

        let native_doc = app.tabs[0]
            .scene
            .native_doc()
            .expect("native document should remain available");
        assert_eq!(
            native_doc
                .entities
                .iter()
                .filter(|entity| matches!(entity.data, nm::EntityData::Viewport { .. }))
                .count(),
            1,
            "paper-space viewport commit should mirror into native document"
        );
    }

    fn sample_pid_doc() -> PidDocument {
        let mut doc = PidDocument::default();
        doc.object_graph = Some(ObjectGraph {
            drawing_no: Some("PID-100".into()),
            project_number: Some("P-01".into()),
            objects: vec![PidObject {
                drawing_id: "OBJ_AAAA1111".into(),
                item_type: "Instrument".into(),
                drawing_item_type: Some("Symbol".into()),
                model_id: Some("MODEL-01".into()),
                extra: std::collections::BTreeMap::from([("Tag".into(), "FIT-001".into())]),
                record_id: Some(0x6001),
                field_x: Some(10),
            }],
            relationships: vec![PidRelationship {
                model_id: "Relationship.R1".into(),
                guid: "R1".into(),
                record_id: Some(0x7001),
                field_x: Some(11),
                source_drawing_id: Some("OBJ_AAAA1111".into()),
                target_drawing_id: None,
            }],
            by_drawing_id: std::collections::BTreeMap::new(),
            counts_by_type: std::collections::BTreeMap::new(),
        });
        doc.layout = Some(PidLayoutModel {
            items: vec![PidLayoutItem {
                layout_id: "item:OBJ_AAAA1111".into(),
                drawing_id: Some("OBJ_AAAA1111".into()),
                graphic_oid: Some(3406),
                kind: "Instrument".into(),
                anchor: [120.0, 80.0],
                bounds: None,
                symbol_name: Some("Instrument".into()),
                symbol_path: Some(
                    r"\\srv\sym\Instrumentation\System Functions\D C S\DCS Field Mounted.sym"
                        .into(),
                ),
                label: Some("FIT-001".into()),
                model_id: Some("MODEL-01".into()),
            }],
            ..Default::default()
        });
        doc
    }

    fn sample_pid_summary() -> PidImportSummary {
        PidImportSummary {
            title: "PID-100".into(),
            object_count: 1,
            relationship_count: 1,
            unresolved_relationship_count: 1,
            symbol_count: 0,
            cluster_count: 0,
            sheet_count: 0,
            stream_count: 0,
            attribute_class_count: 0,
            tagged_text_count: 0,
            dynamic_attribute_record_count: 0,
            object_graph_available: true,
        }
    }

    #[test]
    fn refresh_properties_shows_pid_overview_without_selection() {
        let mut app = H7CAD::new();
        let doc = sample_pid_doc();
        let pid_state = PidTabState::new(
            doc.clone(),
            build_import_view(&doc),
            sample_pid_summary(),
            PidPreviewIndex::default(),
        );

        app.tabs[0].tab_mode = DocumentTabMode::Pid;
        app.tabs[0].pid_state = Some(pid_state);

        app.refresh_properties();

        assert_eq!(app.tabs[0].properties.title, "P&ID Overview");
        assert!(
            app.tabs[0]
                .properties
                .sections
                .iter()
                .any(|section| section.title == "Counts"),
            "pid overview should expose counts section"
        );
    }

    #[test]
    fn refresh_properties_shows_pid_object_details_when_selected_key_exists() {
        let mut app = H7CAD::new();
        let doc = sample_pid_doc();
        let mut pid_state = PidTabState::new(
            doc.clone(),
            build_import_view(&doc),
            sample_pid_summary(),
            PidPreviewIndex::default(),
        );
        pid_state.selected_key = Some(PidNodeKey::Object {
            drawing_id: "OBJ_AAAA1111".into(),
        });

        app.tabs[0].tab_mode = DocumentTabMode::Pid;
        app.tabs[0].pid_state = Some(pid_state);

        app.refresh_properties();

        assert!(
            app.tabs[0].properties.title.contains("Object"),
            "pid object selection should switch inspector title"
        );
        assert!(
            app.tabs[0]
                .properties
                .sections
                .iter()
                .flat_map(|section| section.props.iter())
                .any(|prop| prop.label == "Drawing ID"),
            "pid object selection should expose drawing identifier"
        );
    }

    #[test]
    fn refresh_properties_shows_pid_layout_symbol_evidence_when_available() {
        let mut app = H7CAD::new();
        let doc = sample_pid_doc();
        let mut pid_state = PidTabState::new(
            doc.clone(),
            build_import_view(&doc),
            sample_pid_summary(),
            PidPreviewIndex::default(),
        );
        pid_state.selected_key = Some(PidNodeKey::Object {
            drawing_id: "OBJ_AAAA1111".into(),
        });

        app.tabs[0].tab_mode = DocumentTabMode::Pid;
        app.tabs[0].pid_state = Some(pid_state);

        app.refresh_properties();

        assert!(
            app.tabs[0]
                .properties
                .sections
                .iter()
                .any(|section| section.title == "Symbol Evidence"),
            "pid object inspector should surface layout-backed symbol evidence"
        );
        assert!(
            app.tabs[0]
                .properties
                .sections
                .iter()
                .flat_map(|section| section.props.iter())
                .any(|prop| prop.label == "Symbol Path"),
            "symbol evidence should expose the representative .sym path"
        );
    }
}
