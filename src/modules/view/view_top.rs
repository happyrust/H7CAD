use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "VIEW_TOP",
        label: "Top",
        icon: IconKind::Glyph("⊤"),
        event: ModuleEvent::Command("VIEW TOP".to_string()),
    }
}
