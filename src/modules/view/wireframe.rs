// Wireframe visual style toggle.
// id "WIREFRAME" is special-cased in ribbon.rs for active-state highlighting.

use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub fn tool() -> ToolDef {
    ToolDef {
        id: "WIREFRAME",
        label: "Wireframe",
        icon: IconKind::Glyph("□"),
        event: ModuleEvent::SetWireframe(true),
    }
}
