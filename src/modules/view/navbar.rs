use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/navbar.svg"));
pub fn tool() -> ToolDef {
    ToolDef { id: "NAVBAR", label: "Navigation\nBar", icon: ICON, event: ModuleEvent::Command("NAVBAR".to_string()) }
}
