// Layout module — paper space tools (viewports, scale, plot settings).
// This tab is only shown when the active layout is not "Model".

pub mod mview;

use crate::modules::{CadModule, RibbonGroup};

pub struct LayoutModule;

impl CadModule for LayoutModule {
    fn id(&self) -> &'static str {
        "layout"
    }
    fn title(&self) -> &'static str {
        "Layout"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![RibbonGroup {
            title: "Viewport",
            tools: vec![mview::tool().into()],
        }]
    }
}
