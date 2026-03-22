// Match Properties tool — ribbon definition.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "MATCHPROP",
        label: "Match",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/match_prop.svg")),
        event: ModuleEvent::Command("MATCHPROP".to_string()),
    }
}
