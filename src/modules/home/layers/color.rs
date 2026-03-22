// Color tool — ribbon definition.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

#[allow(dead_code)]
pub fn tool() -> ToolDef {
    ToolDef {
        id: "COLOR",
        label: "Color",
        icon: IconKind::Glyph("🎨"),
        event: ModuleEvent::Command("COLOR".to_string()),
    }
}
