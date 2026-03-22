use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "VIEW_ISO",
        label: "Iso",
        icon: IconKind::Glyph("◈"),
        event: ModuleEvent::Command("VIEW ISO".to_string()),
    }
}
