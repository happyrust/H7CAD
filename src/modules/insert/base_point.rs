// BASE tool — ribbon definition + interactive command.
//
// Command:  BASE
//   Prompts the user to pick a point (or type `x,y[,z]` in the command line).
//   The point is written into the document header as `$INSBASE`, the default
//   base point used when this drawing is inserted into another as a block or
//   XREF.  Enter at the first prompt accepts the current header value (or
//   `0,0,0` if never set).

use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/base_point.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "BASE",
        label: "Set Base\nPoint",
        icon: ICON,
        event: ModuleEvent::Command("BASE".to_string()),
    }
}

/// Interactive `BASE` command — one shot: pick a point or type coordinates.
pub struct SetBasePointCommand {
    /// Current header value used as the "accept Enter" default (display only).
    current: [f64; 3],
}

impl SetBasePointCommand {
    pub fn new(current: [f64; 3]) -> Self {
        Self { current }
    }
}

impl CadCommand for SetBasePointCommand {
    fn name(&self) -> &'static str {
        "BASE"
    }

    fn prompt(&self) -> String {
        format!(
            "BASE  Specify base point <{:.4},{:.4},{:.4}>:",
            self.current[0], self.current[1], self.current[2]
        )
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        CmdResult::SetInsertionBase([pt.x as f64, pt.y as f64, pt.z as f64])
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::SetInsertionBase(self.current)
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn wants_text_input(&self) -> bool {
        true
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        parse_point(text).map(CmdResult::SetInsertionBase)
    }

    fn on_mouse_move(&mut self, _pt: Vec3) -> Option<WireModel> {
        None
    }
}

/// Parse `"x,y"` or `"x,y,z"` (whitespace and commas/spaces tolerated) into a
/// 3D point. Missing Z defaults to 0. Returns None on invalid input.
fn parse_point(text: &str) -> Option<[f64; 3]> {
    let parts: Vec<&str> = text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let x: f64 = parts[0].parse().ok()?;
    let y: f64 = parts[1].parse().ok()?;
    let z: f64 = if parts.len() == 3 {
        parts[2].parse().ok()?
    } else {
        0.0
    };
    Some([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_point_accepts_two_dim() {
        assert_eq!(parse_point("1,2"), Some([1.0, 2.0, 0.0]));
        assert_eq!(parse_point("  10.5 , -3.25 "), Some([10.5, -3.25, 0.0]));
    }

    #[test]
    fn parse_point_accepts_three_dim() {
        assert_eq!(parse_point("1,2,3"), Some([1.0, 2.0, 3.0]));
        assert_eq!(parse_point("1 2 3"), Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn parse_point_rejects_invalid() {
        assert_eq!(parse_point(""), None);
        assert_eq!(parse_point("1"), None);
        assert_eq!(parse_point("a,b"), None);
        assert_eq!(parse_point("1,2,3,4"), None);
    }

    #[test]
    fn on_enter_uses_current_default() {
        let mut cmd = SetBasePointCommand::new([7.0, 8.0, 9.0]);
        match cmd.on_enter() {
            CmdResult::SetInsertionBase(p) => assert_eq!(p, [7.0, 8.0, 9.0]),
            _ => panic!("expected SetInsertionBase"),
        }
    }

    #[test]
    fn on_point_forwards_world_coords() {
        let mut cmd = SetBasePointCommand::new([0.0, 0.0, 0.0]);
        match cmd.on_point(Vec3::new(1.5, 2.5, 3.5)) {
            CmdResult::SetInsertionBase(p) => assert_eq!(p, [1.5, 2.5, 3.5]),
            _ => panic!("expected SetInsertionBase"),
        }
    }
}
