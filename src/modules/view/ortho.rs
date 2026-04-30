use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/ortho.svg"));
pub fn tool() -> ToolDef {
    ToolDef { id: "ORTHO", label: "Ortho", icon: ICON, event: ModuleEvent::Command("ORTHO".into()) }
}
