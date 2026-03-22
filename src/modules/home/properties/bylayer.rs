// ByLayer tool — ribbon definition.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "BYLAYER",
        label: "ByLayer",
        icon: IconKind::Glyph("⊟"),
        event: ModuleEvent::Command("BYLAYER".to_string()),
    }
}
