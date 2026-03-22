use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "LAYMATCH",
        label: "Match Layer",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/layers/laymatch.svg")),
        event: ModuleEvent::Command("LAYMATCH".to_string()),
    }
}
