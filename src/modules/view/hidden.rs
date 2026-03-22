use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "HIDDENLINE",
        label: "Hidden",
        icon: IconKind::Glyph("◫"),
        event: ModuleEvent::Command("HIDDENLINE".to_string()),
    }
}
