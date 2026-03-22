use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DONATE",
        label: "Donate",
        icon: IconKind::Svg(include_bytes!("../../../assets/icons/donate.svg")),
        event: ModuleEvent::Command("DONATE".to_string()),
    }
}
