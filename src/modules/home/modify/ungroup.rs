// Ungroup tool — ribbon definition.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "UNGROUP",
        label: "Ungroup",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/ungroup.svg")),
        event: ModuleEvent::Command("UNGROUP".to_string()),
    }
}
