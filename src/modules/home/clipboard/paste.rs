use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub fn tool() -> ToolDef {
    ToolDef {
        id: "PASTECLIP",
        label: "Paste",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/paste.svg")),
        event: ModuleEvent::Command("PASTECLIP".to_string()),
    }
}
