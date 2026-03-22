use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "ORTHO",
        label: "Ortho",
        icon: IconKind::Glyph("⊡"),
        event: ModuleEvent::Command("ORTHO".into()),
    }
}
