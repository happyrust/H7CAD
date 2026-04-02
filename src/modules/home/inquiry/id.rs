// ID command — report coordinates of a picked point.

use glam::Vec3;

use crate::command::{CadCommand, CmdResult};

pub struct IdCommand;

impl IdCommand {
    pub fn new() -> Self {
        Self
    }
}

impl CadCommand for IdCommand {
    fn name(&self) -> &'static str {
        "ID"
    }

    fn prompt(&self) -> String {
        "ID  Specify point:".into()
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        // Y-up world: X stays X, Z→drawing Y, Y→elevation Z
        let x = pt.x;
        let y = pt.z;
        let z = pt.y;
        let msg = format!("X = {x:.4},  Y = {y:.4},  Z = {z:.4}");
        CmdResult::Measurement(msg)
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}
