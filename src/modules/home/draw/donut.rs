// DONUT command — create a filled circular ring (thick LwPolyline).
//
// A donut is an LwPolyline with:
//   - 2 vertices at (cx ± r_avg, 0), both with bulge = 1.0  (two 180° CCW arcs)
//   - start_width = end_width = (outer - inner) / 2
//   - is_closed = true
//
// Workflow:
//   1. Type inner diameter (or 0 for a filled circle)
//   2. Type outer diameter
//   3. Click center point(s); Enter to finish

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};

pub struct DonutCommand {
    state: DonutState,
    inner_r: f64,
    outer_r: f64,
}

enum DonutState {
    AskInner,
    AskOuter,
    PlaceCenter,
}

impl DonutCommand {
    pub fn new() -> Self {
        Self {
            state: DonutState::AskInner,
            inner_r: 0.0,
            outer_r: 1.0,
        }
    }
}

impl CadCommand for DonutCommand {
    fn name(&self) -> &'static str { "DONUT" }

    fn prompt(&self) -> String {
        match &self.state {
            DonutState::AskInner  => "DONUT  Specify inside diameter <0>:".into(),
            DonutState::AskOuter  => "DONUT  Specify outside diameter:".into(),
            DonutState::PlaceCenter => "DONUT  Specify center of donut (Enter to exit):".into(),
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.state, DonutState::AskInner | DonutState::AskOuter)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let val: f64 = text.trim().replace(',', ".").parse().ok().filter(|&v: &f64| v >= 0.0)?;
        match &self.state {
            DonutState::AskInner => {
                self.inner_r = val / 2.0;
                self.state = DonutState::AskOuter;
                Some(CmdResult::NeedPoint)
            }
            DonutState::AskOuter => {
                if val <= 0.0 { return None; }
                self.outer_r = val / 2.0;
                if self.inner_r > self.outer_r {
                    std::mem::swap(&mut self.inner_r, &mut self.outer_r);
                }
                self.state = DonutState::PlaceCenter;
                Some(CmdResult::NeedPoint)
            }
            _ => None,
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.state {
            DonutState::PlaceCenter => {
                let entity = make_donut_native(pt.x as f64, pt.z as f64, self.inner_r, self.outer_r);
                CmdResult::CommitEntityNative(entity)
            }
            _ => CmdResult::NeedPoint,
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match &self.state {
            DonutState::AskInner => {
                // Accept default 0 for inner diameter
                self.inner_r = 0.0;
                self.state = DonutState::AskOuter;
                CmdResult::NeedPoint
            }
            DonutState::PlaceCenter => CmdResult::Cancel,
            _ => CmdResult::Cancel,
        }
    }
}

fn make_donut_native(cx: f64, cy: f64, inner_r: f64, outer_r: f64) -> nm::Entity {
    let r_avg = (inner_r + outer_r) / 2.0;
    let width = outer_r - inner_r;

    nm::Entity::new(nm::EntityData::LwPolyline {
        vertices: vec![
            nm::LwVertex {
                x: cx - r_avg,
                y: cy,
                bulge: 1.0,
                start_width: width,
                end_width: width,
            },
            nm::LwVertex {
                x: cx + r_avg,
                y: cy,
                bulge: 1.0,
                start_width: width,
                end_width: width,
            },
        ],
        closed: true,
        constant_width: width,
    })
}
