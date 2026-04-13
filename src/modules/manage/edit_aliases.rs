use crate::modules::{IconKind, ModuleEvent, ToolDef};
#[allow(dead_code)]
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/edit_aliases.svg"));
#[allow(dead_code)]
pub fn tool() -> ToolDef {
    ToolDef { id: "ALIASEDIT", label: "Edit Aliases", icon: ICON, event: ModuleEvent::Command("ALIASEDIT".to_string()) }
}
