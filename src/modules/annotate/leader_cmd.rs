// LEADER command
//
// Flow:
//   1. CollectPoints  — click arrowhead, then bend points; Enter (≥2) to finish
//   2. AskCreationType— wants_text_input; N/T/B/TL → default Text on blank Enter
//   3. AskAnnotation  — text string (Text) or block name (Block); blank = skip
//   → commit Leader  [+ MText | + Insert]

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/leader.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "LEADER",
        label: "Leader",
        icon: ICON,
        event: ModuleEvent::Command("LEADER".to_string()),
    }
}

// native::EntityData::Leader 不含 text_height 字段；bridge 投影到 acadrust 时
// 使用 `ar::Leader::new` 的默认 2.5，这里保留相同常量用于 landing_pt / MText 高度
const LEADER_TEXT_HEIGHT: f64 = 2.5;

/// Creation choice parsed from the user's annotation-type prompt.
#[derive(Clone, Copy)]
enum CreationChoice {
    None,
    Text,
    Block,
    Tolerance,
}

enum Step {
    CollectPoints { verts: Vec<Vec3> },
    AskCreationType { verts: Vec<Vec3> },
    AskText { verts: Vec<Vec3> },
    AskBlock { verts: Vec<Vec3> },
}

pub struct LeaderCommand {
    step: Step,
}

impl LeaderCommand {
    pub fn new() -> Self {
        Self { step: Step::CollectPoints { verts: Vec::new() } }
    }
}

impl CadCommand for LeaderCommand {
    fn name(&self) -> &'static str { "LEADER" }

    fn prompt(&self) -> String {
        match &self.step {
            Step::CollectPoints { verts } if verts.is_empty() =>
                "LEADER  Specify arrowhead point:".into(),
            Step::CollectPoints { verts } =>
                format!("LEADER  Specify next point [{} pts — Enter to finish]:", verts.len()),
            Step::AskCreationType { .. } =>
                "LEADER  Annotation type [None/Text/Block/Tolerance] <Text>:".into(),
            Step::AskText { verts } =>
                format!("LEADER  Annotation text [{} pts — blank = skip]:", verts.len()),
            Step::AskBlock { verts } =>
                format!("LEADER  Block name [{} pts — blank = skip]:", verts.len()),
        }
    }

    fn wants_text_input(&self) -> bool {
        !matches!(self.step, Step::CollectPoints { .. })
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        if let Step::CollectPoints { verts } = &mut self.step {
            verts.push(pt);
        }
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if let Step::CollectPoints { verts } = &self.step {
            if verts.len() < 2 { return CmdResult::Cancel; }
            let verts = verts.clone();
            self.step = Step::AskCreationType { verts };
            CmdResult::NeedPoint
        } else {
            CmdResult::Cancel
        }
    }

    fn on_text_input(&mut self, raw: &str) -> Option<CmdResult> {
        let text = raw.trim();
        match &self.step {
            Step::AskCreationType { verts } => {
                let verts = verts.clone();
                match parse_ct(text) {
                    CreationChoice::None | CreationChoice::Tolerance => {
                        Some(CmdResult::CommitAndExitNative(build_leader_native(&verts)))
                    }
                    CreationChoice::Block => {
                        self.step = Step::AskBlock { verts };
                        Some(CmdResult::NeedPoint)
                    }
                    CreationChoice::Text => {
                        self.step = Step::AskText { verts };
                        Some(CmdResult::NeedPoint)
                    }
                }
            }
            Step::AskText { verts } => {
                let verts = verts.clone();
                let leader = build_leader_native(&verts);
                if text.is_empty() {
                    return Some(CmdResult::CommitAndExitNative(leader));
                }
                let mtext = build_mtext_native(
                    text,
                    landing_pt(&verts, LEADER_TEXT_HEIGHT),
                    LEADER_TEXT_HEIGHT,
                );
                Some(CmdResult::CommitManyAndExitNative(vec![leader, mtext]))
            }
            Step::AskBlock { verts } => {
                let verts = verts.clone();
                let leader = build_leader_native(&verts);
                if text.is_empty() {
                    return Some(CmdResult::CommitAndExitNative(leader));
                }
                let insert = build_insert_native(text, landing_pt(&verts, LEADER_TEXT_HEIGHT));
                Some(CmdResult::CommitManyAndExitNative(vec![leader, insert]))
            }
            Step::CollectPoints { .. } => None,
        }
    }

    fn on_escape(&mut self) -> CmdResult { CmdResult::Cancel }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if let Step::CollectPoints { verts } = &self.step {
            if verts.is_empty() { return None; }
            let mut pts = verts.clone();
            pts.push(pt);
            Some(preview_wire(&pts))
        } else {
            None
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn parse_ct(s: &str) -> CreationChoice {
    match s.to_ascii_uppercase().as_str() {
        "N" | "NONE"      => CreationChoice::None,
        "B" | "BLOCK"     => CreationChoice::Block,
        "TL"| "TOLERANCE" => CreationChoice::Tolerance,
        _                 => CreationChoice::Text,
    }
}

fn build_leader_native(verts: &[Vec3]) -> nm::Entity {
    nm::Entity::new(nm::EntityData::Leader {
        vertices: verts.iter().map(|p| [p.x as f64, p.y as f64, p.z as f64]).collect(),
        has_arrowhead: true,
    })
}

fn landing_pt(verts: &[Vec3], text_height: f64) -> Vec3 {
    let last = *verts.last().unwrap();
    let prev = verts[verts.len() - 2];
    let sign = if last.x >= prev.x { 1.0_f32 } else { -1.0_f32 };
    Vec3::new(last.x + sign * text_height as f32 * 1.5, last.y, last.z)
}

fn build_mtext_native(text: &str, pos: Vec3, height: f64) -> nm::Entity {
    nm::Entity::new(nm::EntityData::MText {
        insertion: [pos.x as f64, pos.y as f64, pos.z as f64],
        height,
        width: 0.0,
        rectangle_height: None,
        value: text.to_string(),
        rotation: 0.0,
        style_name: "Standard".into(),
        attachment_point: 1,
        line_spacing_factor: 1.0,
        drawing_direction: 1,
    })
}

fn build_insert_native(block_name: &str, pos: Vec3) -> nm::Entity {
    nm::Entity::new(nm::EntityData::Insert {
        block_name: block_name.to_string(),
        insertion: [pos.x as f64, pos.y as f64, pos.z as f64],
        scale: [1.0, 1.0, 1.0],
        rotation: 0.0,
        has_attribs: false,
        attribs: Vec::new(),
    })
}

fn preview_wire(pts: &[Vec3]) -> WireModel {
    let mut points: Vec<[f32; 3]> = pts.iter().map(|p| [p.x, p.y, p.z]).collect();
    if pts.len() >= 2 {
        let [w1, w2] = arrowhead_wings(pts[0], pts[1], 2.0);
        points.push([f32::NAN; 3]);
        points.push([w1.x, w1.y, w1.z]);
        points.push([pts[0].x, pts[0].y, pts[0].z]);
        points.push([w2.x, w2.y, w2.z]);
    }
    WireModel {
        name: "leader_preview".into(),
        points,
        color: WireModel::CYAN,
        selected: false,
        pattern_length: 0.0,
        pattern: [0.0; 8],
        line_weight_px: 1.0,
        snap_pts: vec![],
        tangent_geoms: vec![],
        aci: 0,
            key_vertices: vec![],
    }
}

pub fn arrowhead_wings(tip: Vec3, next: Vec3, size: f32) -> [Vec3; 2] {
    let d = next - tip;
    let len = (d.x * d.x + d.y * d.y).sqrt().max(1e-9);
    let (dx, dy) = (d.x / len, d.y / len);
    let angle = std::f32::consts::PI / 6.0;
    let (s, c) = angle.sin_cos();
    [
        Vec3::new(tip.x + (dx*c - dy*s)*size, tip.y + (dx*s + dy*c)*size, tip.z),
        Vec3::new(tip.x + (dx*c + dy*s)*size, tip.y + (-dx*s + dy*c)*size, tip.z),
    ]
}
