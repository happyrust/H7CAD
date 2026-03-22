use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "VIEW_RIGHT",
        label: "Right",
        icon: IconKind::Glyph("⊣"),
        event: ModuleEvent::Command("VIEW RIGHT".to_string()),
    }
}
