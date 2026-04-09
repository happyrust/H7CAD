use crate::modules::{IconKind, ModuleEvent, ToolDef};
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/base_point.svg"));
pub fn tool() -> ToolDef {
    ToolDef { id: "BASE", label: "Set Base\nPoint", icon: ICON, event: ModuleEvent::Command("BASE".to_string()) }
}
