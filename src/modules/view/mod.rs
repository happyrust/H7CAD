// View module — navigation, visual styles, and viewport presets.

mod hidden;
mod orbit;
mod ortho;
mod pan;
mod persp;
mod solid;
mod view_front;
mod view_iso;
mod view_right;
mod view_top;
mod wireframe;
mod xray;
mod zoom_ext;
mod zoom_in;
mod zoom_out;

use crate::modules::{CadModule, RibbonGroup};

pub struct ViewModule;

impl CadModule for ViewModule {
    fn id(&self) -> &'static str {
        "view"
    }
    fn title(&self) -> &'static str {
        "View"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![
            RibbonGroup {
                title: "Navigate",
                tools: vec![
                    zoom_ext::tool().into(),
                    zoom_in::tool().into(),
                    zoom_out::tool().into(),
                    pan::tool().into(),
                    orbit::tool().into(),
                ],
            },
            RibbonGroup {
                // WIREFRAME and SOLID ids are special-cased in ribbon.rs
                // for toggle-state highlighting based on Ribbon::wireframe.
                title: "Visual Style",
                tools: vec![
                    wireframe::tool().into(),
                    solid::tool().into(),
                    hidden::tool().into(),
                    xray::tool().into(),
                ],
            },
            RibbonGroup {
                // ORTHO and PERSP ids are special-cased in ribbon.rs
                // for toggle-state highlighting based on Camera::projection.
                title: "Projection",
                tools: vec![ortho::tool().into(), persp::tool().into()],
            },
            RibbonGroup {
                title: "Preset",
                tools: vec![
                    view_top::tool().into(),
                    view_front::tool().into(),
                    view_right::tool().into(),
                    view_iso::tool().into(),
                ],
            },
        ]
    }
}
