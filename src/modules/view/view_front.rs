use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "VIEW_FRONT",
        label: "Front",
        icon: IconKind::Glyph("⊥"),
        event: ModuleEvent::Command("VIEW FRONT".to_string()),
    }
}
