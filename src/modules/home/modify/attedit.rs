// ATTEDIT command — edit attribute values of a selected INSERT entity.
//
// Workflow:
//   Step 1: pick an INSERT entity that has attributes.
//   Step 2+: for each attribute tag, show "TAG = <value>" and accept new
//            text via text input.  Enter with empty string keeps the old value.
//            After all attributes are processed, commit via ReplaceMany.

use acadrust::EntityType;
use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::scene::wire_model::WireModel;

pub struct AtteditCommand {
    step: Step,
}

enum Step {
    /// Waiting for the user to pick an INSERT entity.
    SelectInsert,
    /// Editing the attribute at index `idx` of the collected insert data.
    EditAttr {
        handle: acadrust::Handle,
        /// (tag, current_value) pairs collected from the insert.
        attrs: Vec<(String, String)>,
        /// Index of the attribute currently being edited.
        idx: usize,
    },
}

impl AtteditCommand {
    pub fn new() -> Self {
        Self { step: Step::SelectInsert }
    }
}

impl CadCommand for AtteditCommand {
    fn name(&self) -> &'static str { "ATTEDIT" }

    fn prompt(&self) -> String {
        match &self.step {
            Step::SelectInsert => "ATTEDIT  Select block with attributes:".to_string(),
            Step::EditAttr { attrs, idx, .. } => {
                let (tag, val) = &attrs[*idx];
                format!("ATTEDIT  {} = <{}>  (Enter to keep, type new value):", tag, val)
            }
        }
    }

    fn needs_entity_pick(&self) -> bool {
        matches!(self.step, Step::SelectInsert)
    }

    fn on_entity_pick(&mut self, handle: acadrust::Handle, _pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }
        // We can't inspect the document here — store the handle and let the
        // command host inject the attribute list via `init_with_attrs`.
        // Instead, signal the host to call prepare_attedit().
        self.step = Step::EditAttr {
            handle,
            attrs: vec![],  // will be filled by init_with_attrs() in cmd_result.rs
            idx: 0,
        };
        CmdResult::NeedPoint
    }

    fn wants_text_input(&self) -> bool {
        if let Step::EditAttr { ref attrs, idx, .. } = self.step {
            !attrs.is_empty() && idx < attrs.len()
        } else {
            false
        }
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let Step::EditAttr { handle, attrs, idx } = &mut self.step else { return None; };
        let handle = *handle;

        // Update the current attribute value if the user typed something.
        if !text.trim().is_empty() {
            attrs[*idx].1 = text.trim().to_string();
        }
        *idx += 1;

        if *idx >= attrs.len() {
            // All attributes done — build new Insert entity.
            // We signal via a special CmdResult; the host will call apply_attedit().
            let pairs = attrs.clone();
            return Some(CmdResult::ReplaceEntity(
                handle,
                vec![make_attedit_sentinel(handle, &pairs)],
            ));
        }

        // More attributes to edit.
        None
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult { CmdResult::NeedPoint }
    fn on_enter(&mut self) -> CmdResult { CmdResult::Cancel }
    fn on_preview_wires(&mut self, _pt: Vec3) -> Vec<WireModel> { vec![] }

    fn attedit_pending_handle(&self) -> Option<acadrust::Handle> {
        if let Step::EditAttr { handle, attrs, .. } = &self.step {
            if attrs.is_empty() {
                return Some(*handle);
            }
        }
        None
    }

    fn attedit_set_attrs(&mut self, new_attrs: Vec<(String, String)>) {
        if let Step::EditAttr { attrs, idx, .. } = &mut self.step {
            *attrs = new_attrs;
            *idx = 0;
        }
    }
}

/// Make a sentinel entity carrying the edited attribute values.
/// Encodes all (tag=value) pairs in the layer field as "tag1\x01val1\x02tag2\x01val2...".
fn make_attedit_sentinel(
    _handle: acadrust::Handle,
    pairs: &[(String, String)],
) -> EntityType {
    let encoded: String = pairs
        .iter()
        .map(|(t, v)| format!("{}\x01{}", t, v))
        .collect::<Vec<_>>()
        .join("\x02");
    let mut xl = acadrust::entities::XLine::new(
        crate::types::Vector3::zero(),
        crate::types::Vector3::new(1.0, 0.0, 0.0),
    );
    xl.common.layer = format!("__ATTEDIT__{}", encoded);
    EntityType::XLine(xl)
}

pub fn native_insert_attrs(entity: &nm::Entity) -> Option<Vec<(String, String)>> {
    let nm::EntityData::Insert { attribs, .. } = &entity.data else {
        return None;
    };
    Some(
        attribs
            .iter()
            .filter_map(|attrib| match &attrib.data {
                nm::EntityData::Attrib { tag, value, .. } => Some((tag.clone(), value.clone())),
                _ => None,
            })
            .collect(),
    )
}

pub fn apply_attedit_native(
    doc: &mut nm::CadDocument,
    handle: acadrust::Handle,
    encoded: &str,
) {
    let Some(entity) = doc.get_entity_mut(nm::Handle::new(handle.value())) else {
        return;
    };
    let nm::EntityData::Insert { attribs, .. } = &mut entity.data else {
        return;
    };
    for pair in encoded.split('\x02') {
        let mut parts = pair.splitn(2, '\x01');
        let Some(tag) = parts.next() else { continue; };
        let Some(val) = parts.next() else { continue; };
        if let Some(attrib) = attribs.iter_mut().find(|attrib| {
            matches!(&attrib.data, nm::EntityData::Attrib { tag: attrib_tag, .. } if attrib_tag == tag)
        }) {
            if let nm::EntityData::Attrib { value, .. } = &mut attrib.data {
                *value = val.to_string();
            }
        }
    }
}

