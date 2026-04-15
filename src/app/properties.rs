use super::{H7CAD, VARIES_LABEL};
use super::helpers::{
    entity_type_key, entity_type_key_native, entity_type_label, entity_type_label_native,
    title_case_word,
};
use crate::scene::dispatch;
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

#[cfg(test)]
mod tests {
    use super::*;
    use h7cad_native_model as nm;

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
}
