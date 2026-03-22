use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "SPHERE",
        label: "Sphere",
        icon: IconKind::Glyph("⬤"),
        event: ModuleEvent::Command("SPHERE".to_string()),
    }
}
