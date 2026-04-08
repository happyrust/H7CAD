// Insert module — file import, solid primitives, and blocks.

mod box_prim;
mod clear;
pub(crate) mod create_block;
mod cylinder;
pub(crate) mod insert_block;
mod open_obj;
pub(crate) mod solid3d_cmds;
mod sphere;
pub(crate) mod wblock;
pub(crate) mod xattach;

use crate::modules::{CadModule, RibbonGroup};

pub struct InsertModule;

impl CadModule for InsertModule {
    fn id(&self) -> &'static str {
        "insert"
    }
    fn title(&self) -> &'static str {
        "Insert"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![
            RibbonGroup {
                title: "Import",
                tools: vec![open_obj::tool().into(), clear::tool().into()],
            },
            RibbonGroup {
                title: "Primitives",
                tools: vec![
                    box_prim::tool().into(),
                    sphere::tool().into(),
                    cylinder::tool().into(),
                ],
            },
            RibbonGroup {
                title: "Block",
                tools: vec![
                    create_block::tool().into(),
                    insert_block::tool().into(),
                    wblock::tool().into(),
                    xattach::tool().into(),
                ],
            },
        ]
    }
}
