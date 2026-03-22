// Solid (Shaded) visual style toggle.
// id "SOLID" is special-cased in ribbon.rs for active-state highlighting.

use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "SOLID",
        label: "Solid",
        icon: IconKind::Glyph("■"),
        event: ModuleEvent::SetWireframe(false),
    }
}
