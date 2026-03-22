// Shared value types used by the dispatch and grip systems.

use acadrust::types::{Color as AcadColor, LineWeight};
use glam::Vec3;

/// The kind of value held by a property row.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    /// Read-only display text.
    ReadOnly(String),
    /// Editable numeric/text field.
    EditText(String),
    /// Layer name — rendered as a combo_box.
    LayerChoice(String),
    /// Generic string choice rendered as a combo_box.
    Choice {
        selected: String,
        options: Vec<String>,
    },
    /// ACI/RGB/ByLayer/ByBlock color — rendered as a color picker.
    ColorChoice(AcadColor),
    /// Color varies across the current multi-selection.
    ColorVaries,
    /// Line weight — rendered as a combo_box.
    LwChoice(LineWeight),
    /// Lineweight varies across the current multi-selection.
    LwVaries,
    /// Linetype name — rendered as a combo_box.
    LinetypeChoice(String),
    /// Boolean flag — rendered as a toggle button (e.g. Invisible).
    BoolToggle { field: &'static str, value: bool },
    /// Hatch pattern name — rendered as a combo_box from the catalog.
    HatchPatternChoice(String),
}

/// A single property row in the Properties panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub label: String,
    /// Stable field identifier used in `PropGeomInput` / `PropGeomCommit` messages.
    pub field: &'static str,
    pub value: PropValue,
}

/// A named section of properties (e.g. "General", "Geometry").
#[derive(Clone, Debug, PartialEq)]
pub struct PropSection {
    pub title: String,
    pub props: Vec<Property>,
}

// ── Grip types ─────────────────────────────────────────────────────────────

/// Describes one grip point for an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripShape {
    Square,
    Diamond,
    Triangle,
}

/// Describes one grip point for an entity.
#[derive(Clone, Debug)]
pub struct GripDef {
    /// Object-local identifier (stable index, unique per object instance).
    pub id: usize,
    /// World-space position of the grip.
    pub world: Vec3,
    /// `true` → midpoint grip (diamond, translates whole object).
    /// `false` → endpoint grip (square, moves a single vertex).
    pub is_midpoint: bool,
    /// Visual marker shape for the grip.
    pub shape: GripShape,
}

/// How to apply a grip drag result.
#[derive(Clone, Debug)]
pub enum GripApply {
    /// Move a specific vertex to this absolute world position.
    Absolute(Vec3),
    /// Translate the whole object by this delta vector.
    Translate(Vec3),
}
