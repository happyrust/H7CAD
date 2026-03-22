// Manage module — application settings and customization tools.

mod options;

use crate::modules::{CadModule, RibbonGroup};

pub struct ManageModule;

impl CadModule for ManageModule {
    fn id(&self) -> &'static str {
        "manage"
    }
    fn title(&self) -> &'static str {
        "Manage"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![RibbonGroup {
            title: "Settings",
            tools: vec![options::tool().into()],
        }]
    }
}
