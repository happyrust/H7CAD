use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "XRAY",
        label: "X-Ray",
        icon: IconKind::Glyph("◈"),
        event: ModuleEvent::Command("XRAY".to_string()),
    }
}
