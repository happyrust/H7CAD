// DDEDIT — edit text content of a Text or MText entity in-place.
//
// Workflow:
//   1. Pick a Text or MText entity (or fire from double-click with handle pre-set).
//   2. Enter new text. Press Enter to commit, Escape to cancel.

use acadrust::{EntityType, Handle};
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};

enum DdeditStep {
    PickEntity,
    EnterText { handle: Handle, current: String },
}

pub struct DdeditCommand {
    step: DdeditStep,
}

impl DdeditCommand {
    pub fn new() -> Self {
        Self { step: DdeditStep::PickEntity }
    }

    /// Start with a pre-picked entity (for double-click use).
    pub fn with_handle(handle: Handle, current: String) -> Self {
        Self { step: DdeditStep::EnterText { handle, current } }
    }
}

impl CadCommand for DdeditCommand {
    fn name(&self) -> &'static str { "DDEDIT" }

    fn prompt(&self) -> String {
        match &self.step {
            DdeditStep::PickEntity => "DDEDIT  Select text entity:".into(),
            DdeditStep::EnterText { current, .. } => {
                format!("DDEDIT  Enter new text <{current}>:")
            }
        }
    }

    fn needs_entity_pick(&self) -> bool {
        matches!(self.step, DdeditStep::PickEntity)
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: Vec3) -> CmdResult {
        if handle.is_null() { return CmdResult::NeedPoint; }
        // The current value will be filled in by the caller (commands.rs dispatch)
        // via on_text_input once the entity is known. Store handle here.
        self.step = DdeditStep::EnterText { handle, current: String::new() };
        CmdResult::NeedPoint
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, DdeditStep::EnterText { .. })
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let (handle, current) = match &self.step {
            DdeditStep::EnterText { handle, current } => (*handle, current.clone()),
            _ => return None,
        };
        // Empty input → keep existing text
        let new_text = if text.trim().is_empty() { current } else { text.to_string() };
        Some(CmdResult::DdeditEntity { handle, new_text })
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult { CmdResult::NeedPoint }
    fn on_enter(&mut self) -> CmdResult { CmdResult::Cancel }
    fn on_escape(&mut self) -> CmdResult { CmdResult::Cancel }
}

/// Extract the text content from a Text or MText entity.
pub fn entity_text(entity: &EntityType) -> Option<String> {
    match entity {
        EntityType::Text(t)  => Some(t.value.clone()),
        EntityType::MText(t) => Some(t.value.clone()),
        EntityType::AttributeDefinition(a) => Some(a.default_value.clone()),
        EntityType::AttributeEntity(a)     => Some(a.get_value().to_string()),
        _ => None,
    }
}

/// Apply new text content to a cloned entity, returning the updated entity.
pub fn set_entity_text(mut entity: EntityType, new_text: &str) -> Option<EntityType> {
    match &mut entity {
        EntityType::Text(t)  => { t.value = new_text.to_string(); Some(entity) }
        EntityType::MText(t) => { t.value = new_text.to_string(); Some(entity) }
        EntityType::AttributeDefinition(a) => { a.default_value = new_text.to_string(); Some(entity) }
        EntityType::AttributeEntity(a)     => { a.set_value(new_text.to_string()); Some(entity) }
        _ => None,
    }
}
