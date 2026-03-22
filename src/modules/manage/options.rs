use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "OPTIONS",
        label: "Options",
        icon: IconKind::Glyph("⚙"),
        event: ModuleEvent::Command("OPTIONS".to_string()),
    }
}
