use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "OPEN",
        label: "Open",
        icon: IconKind::Glyph("📂"),
        event: ModuleEvent::OpenFileDialog,
    }
}
