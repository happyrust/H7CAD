use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "CLEAR",
        label: "Clear",
        icon: IconKind::Glyph("🗑"),
        event: ModuleEvent::ClearModels,
    }
}
