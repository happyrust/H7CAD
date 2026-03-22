use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "ZOOM_IN",
        label: "Zoom In",
        icon: IconKind::Glyph("🔍"),
        event: ModuleEvent::Command("ZOOM IN".to_string()),
    }
}
