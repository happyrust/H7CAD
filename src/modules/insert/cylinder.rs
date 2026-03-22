use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "CYLINDER",
        label: "Cylinder",
        icon: IconKind::Glyph("⬡"),
        event: ModuleEvent::Command("CYLINDER".to_string()),
    }
}
