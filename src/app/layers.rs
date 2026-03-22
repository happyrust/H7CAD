use super::H7CAD;
use crate::ui;

impl H7CAD {
    pub(super) fn sync_ribbon_layers(&mut self) {
        let i = self.active_tab;
        let active = self.tabs[i].active_layer.clone();
        let infos: Vec<crate::ui::ribbon::LayerInfo> = self.tabs[i]
            .layers
            .layers
            .iter()
            .map(|l| crate::ui::ribbon::LayerInfo {
                name: l.name.clone(),
                color: l.color,
                visible: l.visible,
                frozen: l.frozen,
                locked: l.locked,
            })
            .collect();
        let names: Vec<String> = infos.iter().map(|l| l.name.clone()).collect();
        let active = if names.contains(&active) { active } else { "0".to_string() };
        self.tabs[i].active_layer = active.clone();
        self.tabs[i].layers.current_layer = active.clone();
        self.ribbon.set_layers(infos, &active);
        let lt_items: Vec<ui::properties::LinetypeItem> = self.tabs[i]
            .scene
            .document
            .line_types
            .iter()
            .map(|lt| {
                let name = if lt.name.eq_ignore_ascii_case("bylayer") {
                    "ByLayer".to_string()
                } else {
                    lt.name.clone()
                };
                let art = crate::linetypes::extract_pattern(&lt.description);
                ui::properties::LinetypeItem { name, art }
            })
            .collect();
        self.tabs[i].layers.sync_linetypes(lt_items.clone());
        self.ribbon.set_available_linetypes(lt_items);
    }
}
