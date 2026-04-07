// Annotate module — dimension and text annotation tools.

pub mod aligned_dim;
pub mod angular_dim;
pub mod ddedit;
pub mod diameter_dim;
pub mod dim_baseline;
pub mod dim_continue;
pub mod leader_cmd;
pub mod linear_dim;
pub mod mleader_cmd;
pub mod mtext;
pub mod radius_dim;
pub mod table_cmd;
pub mod text;
pub mod tolerance_cmd;

use crate::modules::{CadModule, RibbonGroup, RibbonItem};

pub struct AnnotateModule;

impl CadModule for AnnotateModule {
    fn id(&self) -> &'static str {
        "annotate"
    }
    fn title(&self) -> &'static str {
        "Annotate"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![
            RibbonGroup {
                title: "Text",
                tools: vec![
                    RibbonItem::LargeDropdown {
                        id: "ANNOTATE_TEXT",
                        label: "Text",
                        icon: text::ICON,
                        items: vec![
                            (text::tool().id, text::tool().label, text::tool().icon),
                            (mtext::tool().id, mtext::tool().label, mtext::tool().icon),
                        ],
                        default: "TEXT",
                    },
                ],
            },
            RibbonGroup {
                title: "Dimensions",
                tools: vec![
                    RibbonItem::LargeDropdown {
                        id: "ANNOTATE_DIM",
                        label: "Dimensions",
                        icon: linear_dim::ICON,
                        items: vec![
                            (linear_dim::tool().id, linear_dim::tool().label, linear_dim::tool().icon),
                            (radius_dim::tool().id, radius_dim::tool().label, radius_dim::tool().icon),
                            (angular_dim::tool().id, angular_dim::tool().label, angular_dim::tool().icon),
                        ],
                        default: "DIMLINEAR",
                    },
                ],
            },
            RibbonGroup {
                title: "Leaders",
                tools: vec![
                    RibbonItem::LargeDropdown {
                        id: "ANNOTATE_LEADER",
                        label: "Leader",
                        icon: leader_cmd::ICON,
                        items: vec![
                            (mleader_cmd::tool().id, mleader_cmd::tool().label, mleader_cmd::tool().icon),
                            (leader_cmd::tool().id, leader_cmd::tool().label, leader_cmd::tool().icon),
                        ],
                        default: "MLEADER",
                    },
                ],
            },
        ]
    }
}
