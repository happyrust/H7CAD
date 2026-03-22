use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "PAN",
        label: "Pan",
        icon: IconKind::Glyph("✥"),
        event: ModuleEvent::Command("PAN".to_string()),
    }
}
