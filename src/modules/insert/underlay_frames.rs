use crate::modules::{IconKind, ModuleEvent, ToolDef};
#[allow(dead_code)]
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/underlay_frames.svg"));
#[allow(dead_code)]
pub fn tool() -> ToolDef {
    ToolDef { id: "FRAMES", label: "Frames", icon: ICON, event: ModuleEvent::Command("FRAMES".to_string()) }
}
