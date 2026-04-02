// Command system — trait for interactive CAD commands.
//
// Each tool that requires user interaction (point picks, object selection,
// numeric input) implements `CadCommand`.  The active command receives
// viewport events from main.rs and returns `CmdResult` tokens that tell
// the host what to do next.

use crate::scene::hatch_model::HatchModel;
use crate::scene::wire_model::WireModel;
use acadrust::{EntityType, Handle};
use glam::Vec3;

// ── Transform ─────────────────────────────────────────────────────────────

/// A geometric transformation applied to existing entities.
#[derive(Clone)]
pub enum EntityTransform {
    /// Move every point by the given world-space delta (Y-up XZ plane).
    Translate(Vec3),
    /// Rotate around `center` by `angle_rad` in the XZ plane (Y-up).
    Rotate { center: Vec3, angle_rad: f32 },
    /// Uniform scale from `center` by `factor`.
    Scale { center: Vec3, factor: f32 },
    /// Mirror across the line through `p1`→`p2` in the XZ plane (Y-up).
    Mirror { p1: Vec3, p2: Vec3 },
}

// ── Tangent object ─────────────────────────────────────────────────────────

/// Geometric representation of a tangent-snap target.
#[derive(Clone, Copy, Debug)]
pub enum TangentObject {
    /// Infinite line through two world-space XZ-plane points.
    Line { p1: Vec3, p2: Vec3 },
    /// Circle in the XZ plane.
    Circle { center: Vec3, radius: f32 },
}

// ── Result token ──────────────────────────────────────────────────────────

/// Returned by every `CadCommand` method to tell main.rs what to do.
#[allow(dead_code)]
pub enum CmdResult {
    /// Command is still waiting for the next point; show updated prompt.
    NeedPoint,
    /// Update the committed-segment wire (normal colour) and keep collecting points.
    InterimWire(WireModel),
    /// Update the in-progress (cyan) preview wire in the viewport.
    Preview(WireModel),
    /// Commit an acadrust entity to the document; keep the command active.
    CommitEntity(EntityType),
    /// Commit an acadrust entity to the document and end the command.
    CommitAndExit(EntityType),
    /// Create a block definition from existing entities and insert one reference.
    CreateBlock {
        handles: Vec<Handle>,
        name: String,
        base: Vec3,
    },
    /// Apply a transform to selected entities and end the command.
    TransformSelected(Vec<Handle>, EntityTransform),
    /// Copy selected entities with a transform; command stays active for more copies.
    CopySelected(Vec<Handle>, EntityTransform),
    /// Commit a hatch fill (stored in Scene::hatches, not the DXF document).
    CommitHatch(HatchModel),
    /// Copy selected entities with multiple transforms (e.g. rectangular array); end command.
    BatchCopy(Vec<Handle>, Vec<EntityTransform>),
    /// Erase `handle` and replace with new entities; command stays active.
    ReplaceEntity(Handle, Vec<EntityType>),
    /// Replace / delete multiple entities and add new ones; command ends.
    /// Each pair: (handle_to_erase, replacement_entities) — empty vec = delete only.
    ReplaceMany(Vec<(Handle, Vec<EntityType>)>, Vec<EntityType>),
    /// Cancel: discard any preview and end the command.
    Cancel,
    /// End the selection-gather phase and re-dispatch the named command
    /// with the gathered handles installed as the active scene selection.
    Relaunch(String, Vec<Handle>),
    /// Move `dest` entities to the layer of the `src` entity; end command.
    MatchEntityLayer { dest: Vec<Handle>, src: Handle },
    /// Copy all visual properties (layer/color/linetype/lineweight) from `src` to `dest`; end command.
    MatchProperties { dest: Vec<Handle>, src: Handle },
    /// Create a named group from the given entity handles; end command.
    CreateGroup { handles: Vec<Handle>, name: String },
    /// Dissolve all groups that contain any of the given handles; end command.
    DeleteGroups { handles: Vec<Handle> },
    /// Freeze or thaw layers by name in the given viewport; command stays active.
    VpLayerUpdate {
        vp_handle: Handle,
        freeze: Vec<String>,
        thaw: Vec<String>,
    },
    /// Paste clipboard entities translated so their centroid lands at `base_pt`; end command.
    PasteClipboard { base_pt: Vec3 },
    /// Zoom the model-space camera to fit the given corner points; end command.
    ZoomToWindow { p1: Vec3, p2: Vec3 },
    /// Print a measurement result to the command line and end the command.
    Measurement(String),
    /// Break `handle` at points `p1` and `p2`; replace with computed fragments.
    BreakEntity { handle: Handle, p1: Vec3, p2: Vec3 },
    /// Attempt to join the given entities into fewer merged entities.
    JoinEntities(Vec<Handle>),
}

// ── Trait ─────────────────────────────────────────────────────────────────

/// An interactive CAD command that collects user input step-by-step.
pub trait CadCommand: Send {
    /// Short name shown in the command line prompt, e.g. `"LINE"`.
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    /// Current prompt string to display in the command line.
    fn prompt(&self) -> String;

    /// Called when the user left-clicks in the viewport (point pick).
    fn on_point(&mut self, pt: Vec3) -> CmdResult;

    /// Called when the user presses Enter (finalize / next option).
    fn on_enter(&mut self) -> CmdResult;

    /// Called when the user presses Escape (cancel).
    #[allow(dead_code)]
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    /// Returns `true` when the command needs entity picking (hit-test) instead of point picking.
    fn needs_entity_pick(&self) -> bool {
        false
    }

    /// Called when the user clicks and `needs_entity_pick()` is true.
    /// `handle` is the nearest wire's entity handle (Handle::NULL if nothing found).
    fn on_entity_pick(&mut self, _handle: Handle, _pt: Vec3) -> CmdResult {
        CmdResult::Cancel
    }

    /// Called after `CmdResult::ReplaceEntity` is applied to the document.
    /// `old` is the erased handle; `new_handles` are the handles assigned to the replacement entities.
    /// Commands that stay active across replaces should update their internal snapshots here.
    fn on_entity_replaced(&mut self, _old: Handle, _new_handles: &[Handle]) {}

    /// Called on every mouse-move when `needs_entity_pick()` is true.
    /// Return preview wires showing the operation result under the cursor.
    /// Default: empty (no preview).
    fn on_hover_entity(&mut self, _handle: Handle, _pt: Vec3) -> Vec<WireModel> {
        vec![]
    }

    /// Called on every mouse-move in the viewport.
    /// Return `Some(WireModel)` to update the rubber-band preview, `None` to skip.
    fn on_mouse_move(&mut self, _pt: Vec3) -> Option<WireModel> {
        None
    }

    /// Called on every mouse-move; return all preview wires to show (object ghosts + rubber-band).
    /// Default: forwards to `on_mouse_move` for backwards compatibility.
    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        self.on_mouse_move(pt).into_iter().collect()
    }

    /// Returns `true` when the command is waiting for text typed in the command line.
    fn wants_text_input(&self) -> bool {
        false
    }

    /// Called when the user submits text via the command line while `wants_text_input` is true.
    fn on_text_input(&mut self, _text: &str) -> Option<CmdResult> {
        None
    }

    /// Returns `true` when the command is in a selection-gathering phase.
    /// While true, viewport clicks are routed through the normal selection
    /// system (single / box / polygon) instead of the command's point-pick path.
    /// After each completed selection action the host calls `on_selection_complete`.
    fn is_selection_gathering(&self) -> bool {
        false
    }

    /// Called after a selection action completes while `is_selection_gathering` is true.
    /// `handles` is the full set of currently selected entities.
    /// Return `Relaunch` to fire the pending command, or `NeedPoint` to keep gathering.
    fn on_selection_complete(&mut self, _handles: Vec<Handle>) -> CmdResult {
        CmdResult::Cancel
    }

    /// Returns `true` when the command wants object picks via Tangent snap.
    fn needs_tangent_pick(&self) -> bool {
        false
    }

    /// Called instead of `on_point` when the command needs a tangent pick
    /// and the snap system found a tangent object.
    fn on_tangent_point(&mut self, obj: TangentObject, hit: Vec3) -> CmdResult {
        let _ = obj;
        self.on_point(hit)
    }
}
