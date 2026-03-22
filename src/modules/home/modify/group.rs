// Group tool — ribbon definition.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "GROUP",
        label: "Group",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/group.svg")),
        event: ModuleEvent::Command("GROUP".to_string()),
    }
}
