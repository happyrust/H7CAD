// Shapes dropdown — Rectangle and Polygon creation methods.
//
// Rectangle:
//   RECT     — Two Corners (axis-aligned)
//   RECT_ROT — Rotated (corner + adjacent corner + height)
//   RECT_CEN — Center Point + corner
//
// Polygon (regular N-gon, sides typed in command line):
//   POLY   — Inscribed in circle   (vertices ON the circle)
//   POLY_C — Circumscribed about circle (edges tangent to circle)
//   POLY_E — Edge (pick two endpoints of one edge)

use h7cad_native_model as nm;

use crate::command::{CadCommand, CmdResult};
use crate::modules::IconKind;
use crate::scene::wire_model::WireModel;
use glam::Vec3;

const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;

// ── Icons ──────────────────────────────────────────────────────────────────

const ICON_RECT: IconKind =
    IconKind::Svg(include_bytes!("../../../../assets/icons/shapes/rect.svg"));
const ICON_RECT_ROT: IconKind = IconKind::Svg(include_bytes!(
    "../../../../assets/icons/shapes/rect_rot.svg"
));
const ICON_RECT_CEN: IconKind = IconKind::Svg(include_bytes!(
    "../../../../assets/icons/shapes/rect_cen.svg"
));
const ICON_POLY_I: IconKind =
    IconKind::Svg(include_bytes!("../../../../assets/icons/shapes/poly_i.svg"));
const ICON_POLY_C: IconKind =
    IconKind::Svg(include_bytes!("../../../../assets/icons/shapes/poly_c.svg"));
const ICON_POLY_E: IconKind =
    IconKind::Svg(include_bytes!("../../../../assets/icons/shapes/poly_e.svg"));

// ── Dropdown metadata ──────────────────────────────────────────────────────

pub const DROPDOWN_ID: &str = "SHAPES";

pub const DROPDOWN_ITEMS: &[(&str, &str, IconKind)] = &[
    ("RECT", "Rectangle - Two Corners", ICON_RECT),
    ("RECT_ROT", "Rectangle - Rotated", ICON_RECT_ROT),
    ("RECT_CEN", "Rectangle - Center", ICON_RECT_CEN),
    ("POLY", "Polygon - Inscribed", ICON_POLY_I),
    ("POLY_C", "Polygon - Circumscribed", ICON_POLY_C),
    ("POLY_E", "Polygon - Edge", ICON_POLY_E),
];

pub const ICON: IconKind = ICON_RECT;

// ── Shared geometry helpers ────────────────────────────────────────────────

fn make_pline(xy_pairs: &[[f64; 2]]) -> nm::Entity {
    nm::Entity::new(nm::EntityData::LwPolyline {
        vertices: xy_pairs
            .iter()
            .map(|&[x, y]| nm::LwVertex { x, y, bulge: 0.0 })
            .collect(),
        closed: true,
    })
}

fn wire_loop(pts: Vec<[f32; 3]>) -> WireModel {
    let mut p = pts;
    if let Some(&first) = p.first() {
        p.push(first);
    }
    WireModel::solid("rubber_band".into(), p, WireModel::CYAN, false)
}

fn wire_seg(a: Vec3, b: Vec3) -> WireModel {
    WireModel::solid(
        "rubber_band".into(),
        vec![[a.x, a.y, a.z], [b.x, b.y, b.z]],
        WireModel::CYAN,
        false,
    )
}

// ── Polygon geometry ───────────────────────────────────────────────────────

fn poly_verts_xy(center: Vec3, vertex_r: f32, sides: u32, start_angle: f32) -> Vec<[f64; 2]> {
    (0..sides)
        .map(|i| {
            let a = start_angle + (i as f32) * TAU / sides as f32;
            [
                (center.x + vertex_r * a.cos()) as f64,
                (center.y + vertex_r * a.sin()) as f64,
            ]
        })
        .collect()
}

fn poly_wire(center: Vec3, vertex_r: f32, sides: u32, start_angle: f32) -> WireModel {
    let pts: Vec<[f32; 3]> = (0..sides)
        .map(|i| {
            let a = start_angle + (i as f32) * TAU / sides as f32;
            [
                center.x + vertex_r * a.cos(),
                center.y + vertex_r * a.sin(),
                center.z,
            ]
        })
        .collect();
    wire_loop(pts)
}

fn angle_xy(from: Vec3, to: Vec3) -> f32 {
    (to.y - from.y).atan2(to.x - from.x)
}

// ── Command: Rectangle — Two Corners  (RECT) ──────────────────────────────

pub struct RectCommand {
    a: Option<Vec3>,
}

impl RectCommand {
    pub fn new() -> Self {
        Self { a: None }
    }
}

impl CadCommand for RectCommand {
    fn name(&self) -> &'static str {
        "RECT"
    }
    fn prompt(&self) -> String {
        if self.a.is_none() {
            "RECT  Specify first corner:".into()
        } else {
            "RECT  Specify opposite corner:".into()
        }
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.a {
            None => {
                self.a = Some(pt);
                CmdResult::NeedPoint
            }
            Some(a) => CmdResult::CommitAndExitNative(make_pline(&[
                [a.x as f64, a.y as f64],
                [pt.x as f64, a.y as f64],
                [pt.x as f64, pt.y as f64],
                [a.x as f64, pt.y as f64],
            ])),
        }
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        let a = self.a?;
        Some(wire_loop(vec![
            [a.x, a.y, a.z],
            [pt.x, a.y, a.z],
            [pt.x, pt.y, a.z],
            [a.x, pt.y, a.z],
        ]))
    }
}

// ── Command: Rectangle — Rotated  (RECT_ROT) ──────────────────────────────
//   Step 0: pick corner A
//   Step 1: pick adjacent corner B  (defines one edge direction + length)
//   Step 2: pick height point  (perpendicular offset from the A-B edge)

pub struct RectRotCommand {
    step: u8,
    a: Vec3,
    b: Vec3,
}

impl RectRotCommand {
    pub fn new() -> Self {
        Self {
            step: 0,
            a: Vec3::ZERO,
            b: Vec3::ZERO,
        }
    }
}

impl CadCommand for RectRotCommand {
    fn name(&self) -> &'static str {
        "RECT_ROT"
    }
    fn prompt(&self) -> String {
        match self.step {
            0 => "RECT ROT  Specify first corner:".into(),
            1 => "RECT ROT  Specify adjacent corner (defines edge direction):".into(),
            _ => "RECT ROT  Specify height (perpendicular pick):".into(),
        }
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            0 => {
                self.a = pt;
                self.step = 1;
                CmdResult::NeedPoint
            }
            1 => {
                self.b = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            _ => {
                let (a, b) = (self.a, self.b);
                let dir = (b - a).normalize_or_zero();
                let perp = Vec3::new(-dir.y, dir.x, 0.0);
                let h = (pt - b).dot(perp); // signed height
                let c = b + perp * h;
                let d = a + perp * h;
                CmdResult::CommitAndExitNative(make_pline(&[
                    [a.x as f64, a.y as f64],
                    [b.x as f64, b.y as f64],
                    [c.x as f64, c.y as f64],
                    [d.x as f64, d.y as f64],
                ]))
            }
        }
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        match self.step {
            1 => Some(wire_seg(self.a, pt)),
            2 => {
                let (a, b) = (self.a, self.b);
                let dir = (b - a).normalize_or_zero();
                let perp = Vec3::new(-dir.y, dir.x, 0.0);
                let h = (pt - b).dot(perp);
                let c = b + perp * h;
                let d = a + perp * h;
                Some(wire_loop(vec![
                    [a.x, a.y, a.z],
                    [b.x, b.y, b.z],
                    [c.x, c.y, c.z],
                    [d.x, d.y, d.z],
                ]))
            }
            _ => None,
        }
    }
}

// ── Command: Rectangle — Center  (RECT_CEN) ───────────────────────────────
//   Step 0: pick center
//   Step 1: pick any corner  (half-width = |cx|, half-height = |cy|)

pub struct RectCenCommand {
    center: Option<Vec3>,
}

impl RectCenCommand {
    pub fn new() -> Self {
        Self { center: None }
    }
}

impl CadCommand for RectCenCommand {
    fn name(&self) -> &'static str {
        "RECT_CEN"
    }
    fn prompt(&self) -> String {
        if self.center.is_none() {
            "RECT CEN  Specify center point:".into()
        } else {
            "RECT CEN  Specify corner point:".into()
        }
    }
    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.center {
            None => {
                self.center = Some(pt);
                CmdResult::NeedPoint
            }
            Some(c) => {
                let hw = (pt.x - c.x).abs() as f64;
                let hh = (pt.y - c.y).abs() as f64;
                let cx = c.x as f64;
                let cy = c.y as f64;
                CmdResult::CommitAndExitNative(make_pline(&[
                    [cx - hw, cy - hh],
                    [cx + hw, cy - hh],
                    [cx + hw, cy + hh],
                    [cx - hw, cy + hh],
                ]))
            }
        }
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        let c = self.center?;
        let hw = (pt.x - c.x).abs();
        let hh = (pt.y - c.y).abs();
        Some(wire_loop(vec![
            [c.x - hw, c.y - hh, c.z],
            [c.x + hw, c.y - hh, c.z],
            [c.x + hw, c.y + hh, c.z],
            [c.x - hw, c.y + hh, c.z],
        ]))
    }
}

// ── Command: Polygon — Inscribed  (POLY) ──────────────────────────────────
//   Type number of sides (default 6) → pick center → pick vertex
//   Vertices lie ON the circle of the picked radius.

pub struct PolyCommand {
    sides: u32,
    step: u8,
    center: Vec3,
}

impl PolyCommand {
    pub fn new() -> Self {
        Self {
            sides: 6,
            step: 0,
            center: Vec3::ZERO,
        }
    }
}

impl CadCommand for PolyCommand {
    fn name(&self) -> &'static str {
        "POLY"
    }

    fn wants_text_input(&self) -> bool {
        self.step == 0
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Ok(n) = text.trim().parse::<u32>() {
            if (3..=1024).contains(&n) {
                self.sides = n;
            }
        }
        self.step = 1;
        Some(CmdResult::NeedPoint)
    }

    fn prompt(&self) -> String {
        match self.step {
            0 => format!("POLYGON  Enter number of sides <{}>:", self.sides),
            1 => format!("POLYGON  Specify center [{} sides]:", self.sides),
            _ => format!(
                "POLYGON  Specify vertex on circle [{} sides inscribed]:",
                self.sides
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            0 => {
                // User clicked without typing sides: use default, treat click as center.
                self.center = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            1 => {
                self.center = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            _ => {
                let r = self.center.distance(pt);
                let sa = angle_xy(self.center, pt);
                CmdResult::CommitAndExitNative(make_pline(&poly_verts_xy(self.center, r, self.sides, sa)))
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.step == 0 {
            self.step = 1;
            return CmdResult::NeedPoint;
        }
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if self.step < 2 {
            return None;
        }
        let r = self.center.distance(pt);
        let sa = angle_xy(self.center, pt);
        Some(poly_wire(self.center, r, self.sides, sa))
    }
}

// ── Command: Polygon — Circumscribed  (POLY_C) ────────────────────────────
//   Type sides → pick center → pick edge-midpoint (on the inscribed circle).
//   vertex_radius = inradius / cos(π/N).

pub struct PolyCCommand {
    sides: u32,
    step: u8,
    center: Vec3,
}

impl PolyCCommand {
    pub fn new() -> Self {
        Self {
            sides: 6,
            step: 0,
            center: Vec3::ZERO,
        }
    }
}

impl CadCommand for PolyCCommand {
    fn name(&self) -> &'static str {
        "POLY_C"
    }

    fn wants_text_input(&self) -> bool {
        self.step == 0
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Ok(n) = text.trim().parse::<u32>() {
            if (3..=1024).contains(&n) {
                self.sides = n;
            }
        }
        self.step = 1;
        Some(CmdResult::NeedPoint)
    }

    fn prompt(&self) -> String {
        match self.step {
            0 => format!("POLYGON C  Enter number of sides <{}>:", self.sides),
            1 => format!("POLYGON C  Specify center [{} sides]:", self.sides),
            _ => format!(
                "POLYGON C  Specify edge-midpoint radius [{} sides circumscribed]:",
                self.sides
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            0 => {
                self.center = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            1 => {
                self.center = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            _ => {
                let inradius = self.center.distance(pt);
                let vr = inradius / (PI / self.sides as f32).cos();
                // The picked pt is at the midpoint of an edge; the vertex is
                // offset by half a sector (π/N) from that direction.
                let edge_angle = angle_xy(self.center, pt);
                let sa = edge_angle + PI / self.sides as f32;
                CmdResult::CommitAndExitNative(make_pline(&poly_verts_xy(
                    self.center,
                    vr,
                    self.sides,
                    sa,
                )))
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.step == 0 {
            self.step = 1;
            return CmdResult::NeedPoint;
        }
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if self.step < 2 {
            return None;
        }
        let inradius = self.center.distance(pt);
        let vr = inradius / (PI / self.sides as f32).cos();
        let sa = angle_xy(self.center, pt) + PI / self.sides as f32;
        Some(poly_wire(self.center, vr, self.sides, sa))
    }
}

// ── Command: Polygon — Edge  (POLY_E) ─────────────────────────────────────
//   Type sides → pick edge start A → pick edge end B.
//   Center is computed from the edge and the polygon geometry.

pub struct PolyECommand {
    sides: u32,
    step: u8,
    a: Vec3,
}

impl PolyECommand {
    pub fn new() -> Self {
        Self {
            sides: 6,
            step: 0,
            a: Vec3::ZERO,
        }
    }
}

impl CadCommand for PolyECommand {
    fn name(&self) -> &'static str {
        "POLY_E"
    }

    fn wants_text_input(&self) -> bool {
        self.step == 0
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Ok(n) = text.trim().parse::<u32>() {
            if (3..=1024).contains(&n) {
                self.sides = n;
            }
        }
        self.step = 1;
        Some(CmdResult::NeedPoint)
    }

    fn prompt(&self) -> String {
        match self.step {
            0 => format!("POLYGON E  Enter number of sides <{}>:", self.sides),
            1 => format!(
                "POLYGON E  Specify first endpoint of edge [{} sides]:",
                self.sides
            ),
            _ => format!(
                "POLYGON E  Specify second endpoint of edge [{} sides]:",
                self.sides
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            0 => {
                self.a = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            1 => {
                self.a = pt;
                self.step = 2;
                CmdResult::NeedPoint
            }
            _ => {
                if let Some((center, vr, sa)) = edge_poly_params(self.a, pt, self.sides) {
                    CmdResult::CommitAndExitNative(make_pline(&poly_verts_xy(center, vr, self.sides, sa)))
                } else {
                    CmdResult::Cancel
                }
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.step == 0 {
            self.step = 1;
            return CmdResult::NeedPoint;
        }
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if self.step < 2 {
            return None;
        }
        if let Some((center, vr, sa)) = edge_poly_params(self.a, pt, self.sides) {
            Some(poly_wire(center, vr, self.sides, sa))
        } else {
            Some(wire_seg(self.a, pt))
        }
    }
}

/// Compute polygon center, vertex-radius and start-angle from two edge endpoints.
/// The polygon is placed on the left side of A→B (CCW convention).
fn edge_poly_params(a: Vec3, b: Vec3, sides: u32) -> Option<(Vec3, f32, f32)> {
    let edge_len = a.distance(b);
    if edge_len < 1e-6 {
        return None;
    }
    // vertex_radius = edge_len / (2 * sin(π/N))
    let vr = edge_len / (2.0 * (PI / sides as f32).sin());
    // inradius = vr * cos(π/N) = edge_len / (2 * tan(π/N))
    let inradius = vr * (PI / sides as f32).cos();
    // Center: on the left perpendicular bisector of A→B
    let dir = (b - a) / edge_len;
    let perp = Vec3::new(-dir.y, dir.x, 0.0); // CCW left
    let mid = (a + b) * 0.5;
    let center = mid + perp * inradius;
    // First vertex = A
    let sa = angle_xy(center, a);
    Some((center, vr, sa))
}
