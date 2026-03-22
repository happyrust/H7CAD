use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "3DORBIT",
        label: "3D Orbit",
        icon: IconKind::Glyph("⟳"),
        event: ModuleEvent::Command("3DORBIT".to_string()),
    }
}
