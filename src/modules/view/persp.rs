use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "PERSP",
        label: "Persp",
        icon: IconKind::Glyph("⟁"),
        event: ModuleEvent::Command("PERSP".into()),
    }
}
