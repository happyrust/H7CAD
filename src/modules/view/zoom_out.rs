use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "ZOOM_OUT",
        label: "Zoom Out",
        icon: IconKind::Glyph("🔎"),
        event: ModuleEvent::Command("ZOOM OUT".to_string()),
    }
}
