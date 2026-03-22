use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "BOX",
        label: "Box",
        icon: IconKind::Glyph("⬜"),
        event: ModuleEvent::Command("BOX".to_string()),
    }
}
