use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "ZOOM_EXT",
        label: "Zoom Ext",
        icon: IconKind::Glyph("⊡"),
        event: ModuleEvent::Command("ZOOM EXTENTS".to_string()),
    }
}
