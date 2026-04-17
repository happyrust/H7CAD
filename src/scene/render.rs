// GPU rendering primitives, shader::Program / shader::Primitive impls,
// and entity render-style helpers for the Scene.

use acadrust::tables::LineType;
use crate::types::aci_table::aci_to_rgb;
use crate::types::{Color as AcadColor, LineWeight};
use acadrust::EntityType;
use h7cad_native_model as nm;
use glam::Mat4;
use iced::mouse;
use iced::widget::shader::{self, Viewport};
use iced::{Event, Rectangle, Size};

use super::pipeline::viewcube::{hover_id, VIEWCUBE_PX};
use super::pipeline::Pipeline;
use super::tessellate;
use super::{HatchModel, ImageModel, MeshModel, Scene, Uniforms, WireModel};

// ── Camera hover state (shader::Program::State) ───────────────────────────

#[derive(Clone, Default)]
pub struct CameraState {
    pub hover_region: Option<usize>,
}

// ── GPU primitive ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Primitive {
    pub(super) wires: Vec<WireModel>,
    pub(super) hatches: Vec<HatchModel>,
    /// Wipeout fills — rendered in a separate pass AFTER wires.
    pub(super) wipeout_hatches: Vec<HatchModel>,
    pub(super) images: Vec<ImageModel>,
    pub(super) meshes: Vec<MeshModel>,
    pub(super) uniforms: Uniforms,
    /// Camera rotation matrix derived from the quaternion.
    /// Used by the ViewCube pipeline — no gimbal lock.
    pub(super) cam_rotation: Mat4,
    pub(super) hover_region: Option<usize>,
    /// Background color used to clear the MSAA buffer at the start of each frame.
    pub(super) bg_color: [f32; 4],
    /// Whether to draw the ViewCube pipeline in `render()`.
    pub(super) show_viewcube: bool,
}

// ── shader::Program impl ──────────────────────────────────────────────────

impl<Msg: std::fmt::Debug + Clone> shader::Program<Msg> for Scene {
    type State = CameraState;
    type Primitive = Primitive;

    fn draw(
        &self,
        state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let cam = self.camera.borrow();
        self.selection.borrow_mut().vp_size = (bounds.width, bounds.height);

        let mut all_wires = self.entity_wires();
        if let Some(iw) = &self.interim_wire {
            all_wires.push(iw.clone());
        }
        all_wires.extend(self.preview_wires.iter().cloned());

        let bg_color = if self.current_layout == "Model" {
            self.bg_color
        } else {
            self.paper_bg_color
        };

        Primitive {
            wires: all_wires,
            hatches: self.synced_hatch_models(),
            wipeout_hatches: self.wipeout_models(),
            images: self.images.values().cloned().collect(),
            meshes: self.meshes.values().cloned().collect(),
            uniforms: Uniforms::new(&cam, bounds),
            cam_rotation: cam.view_rotation_mat(),
            hover_region: state.hover_region,
            bg_color,
            show_viewcube: self.show_viewcube,
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Msg>> {
        let pos = cursor.position_in(bounds);
        let cam_rotation = { self.camera.borrow().view_rotation_mat() };
        if self.show_viewcube {
            if let Some(p) = pos {
                state.hover_region = hover_id(
                    p.x,
                    p.y,
                    bounds.width,
                    bounds.height,
                    cam_rotation,
                    VIEWCUBE_PX,
                );
            } else {
                state.hover_region = None;
            }
        } else {
            state.hover_region = None;
        }
        let _ = event;
        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _b: Rectangle,
        _c: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.hover_region.is_some() {
            return mouse::Interaction::Pointer;
        }
        mouse::Interaction::default()
    }
}

// ── shader::Primitive impl ────────────────────────────────────────────────

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let phys = viewport.physical_size();
        let full_size = Size::new(phys.width, phys.height);
        // MSAA and depth textures are sized to the shader widget's clip bounds,
        // not the full surface — so the MSAA resolve can't overwrite other widgets.
        let scale = viewport.scale_factor() as f32;
        let clip_size = Size::new(
            (_bounds.width * scale).ceil() as u32,
            (_bounds.height * scale).ceil() as u32,
        );
        pipeline.ensure_depth_texture(device, clip_size);
        pipeline.viewcube.ensure_depth_texture(device, full_size);
        pipeline.upload_uniforms(queue, &self.uniforms);
        pipeline.upload_hatches(device, &self.hatches);
        pipeline.upload_wipeouts(device, &self.wipeout_hatches);
        pipeline.upload_images(device, queue, &self.images);
        pipeline.upload_meshes(device, &self.meshes);
        pipeline.upload_wires(device, &self.wires);
        let logical = viewport.logical_size();
        pipeline.viewcube.upload(
            queue,
            self.cam_rotation,
            logical.width as u32,
            logical.height as u32,
            self.hover_region,
        );
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut iced::wgpu::CommandEncoder,
        target: &iced::wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        pipeline.render(encoder, target, *clip, self.bg_color);
        if self.show_viewcube {
            pipeline.viewcube.render(encoder, target, *clip);
        }
    }
}

// ── Render-style helpers (impl Scene) ────────────────────────────────────

impl Scene {
    /// Returns (entity_color, pattern_length, pattern, line_weight_px, aci).
    pub(super) fn render_style(&self, e: &EntityType) -> ([f32; 4], f32, [f32; 8], f32, u8) {
        let layer_name = &e.common().layer;
        let (entity_color, aci) = {
            let ec = &e.common().color;
            let resolved = if *ec == AcadColor::ByLayer {
                self.document
                    .layers
                    .get(layer_name)
                    .map(|l| &l.color)
                    .unwrap_or(&AcadColor::WHITE)
            } else {
                ec
            };
            let aci = match resolved {
                AcadColor::Index(i) => *i,
                _ => 0,
            };
            let [r, g, b, _] = tessellate::aci_to_rgba(resolved);
            let alpha = 1.0 - e.common().transparency.as_percent() as f32;
            ([r, g, b, alpha], aci)
        };

        let lt_name = self.resolved_linetype_name(e);
        let lt_scale = e.common().linetype_scale as f32;
        let (pattern_length, pattern) =
            resolve_pattern(&self.document.line_types, lt_name, lt_scale);

        let line_weight_px = {
            let ew = &e.common().line_weight;
            let resolved = match ew {
                LineWeight::ByLayer | LineWeight::ByBlock | LineWeight::Default => self
                    .document
                    .layers
                    .get(layer_name)
                    .map(|l| &l.line_weight)
                    .unwrap_or(&LineWeight::Default),
                _ => ew,
            };
            const MM_TO_PX: f32 = 96.0 / 25.4;
            resolved
                .millimeters()
                .map(|mm| (mm as f32 * MM_TO_PX).max(1.0))
                .unwrap_or(1.0)
        };

        (entity_color, pattern_length, pattern, line_weight_px, aci)
    }

    pub(super) fn resolved_linetype_name<'a>(&'a self, e: &'a EntityType) -> &'a str {
        let elt = &e.common().linetype;
        if elt.is_empty() || elt.eq_ignore_ascii_case("bylayer") {
            self.document
                .layers
                .get(&e.common().layer)
                .map(|l| l.line_type.as_str())
                .unwrap_or("Continuous")
        } else {
            elt.as_str()
        }
    }
}

// ── Linetype pattern helper ───────────────────────────────────────────────

pub(super) fn resolve_pattern(
    table: &acadrust::tables::Table<LineType>,
    name: &str,
    scale: f32,
) -> (f32, [f32; 8]) {
    let solid = (0.0, [0.0f32; 8]);
    if name.eq_ignore_ascii_case("continuous")
        || name.eq_ignore_ascii_case("bylayer")
        || name.eq_ignore_ascii_case("byblock")
        || name.is_empty()
    {
        return solid;
    }
    let lt = match table.get(name) {
        Some(lt) => lt,
        None => return solid,
    };
    if lt.is_continuous() || lt.elements.is_empty() {
        return solid;
    }

    let mut pat = [0.0f32; 8];
    let mut pat_len = 0.0f32;
    for (i, el) in lt.elements.iter().take(8).enumerate() {
        let raw = el.length as f32 * scale;
        let encoded = if raw == 0.0 {
            0.01 * scale.max(0.01)
        } else {
            raw
        };
        pat[i] = encoded;
        pat_len += encoded.abs();
    }
    if pat_len < 1e-6 {
        return solid;
    }
    (pat_len, pat)
}

pub(super) fn render_style_native(
    document: &nm::CadDocument,
    entity: &nm::Entity,
) -> ([f32; 4], f32, [f32; 8], f32, u8) {
    let (entity_color, aci) = if entity.true_color != 0 {
        let r = ((entity.true_color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((entity.true_color >> 8) & 0xFF) as f32 / 255.0;
        let b = (entity.true_color & 0xFF) as f32 / 255.0;
        ([r, g, b, 1.0], 0)
    } else if entity.color_index == 256 {
        let layer = document.layers.get(&entity.layer_name);
        if let Some(layer) = layer {
            if layer.true_color != 0 {
                let r = ((layer.true_color >> 16) & 0xFF) as f32 / 255.0;
                let g = ((layer.true_color >> 8) & 0xFF) as f32 / 255.0;
                let b = (layer.true_color & 0xFF) as f32 / 255.0;
                ([r, g, b, 1.0], 0)
            } else {
                let aci = document.resolve_color(entity).max(0) as u8;
                let (r, g, b) = aci_to_rgb(aci).unwrap_or((255, 255, 255));
                ([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0], aci)
            }
        } else {
            (WireModel::WHITE, 0)
        }
    } else {
        let aci = document.resolve_color(entity).max(0) as u8;
        let (r, g, b) = aci_to_rgb(aci).unwrap_or((255, 255, 255));
        ([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0], aci)
    };

    let (pattern_length, pattern) =
        resolve_pattern_native(&document.linetypes, document.resolve_linetype(entity), 1.0);

    let line_weight_px = {
        const MM_TO_PX: f32 = 96.0 / 25.4;
        let hundredth_mm = document.resolve_lineweight(entity).max(0) as f32;
        let mm = hundredth_mm / 100.0;
        (mm * MM_TO_PX).max(1.0)
    };

    let alpha = 1.0 - (entity.transparency.clamp(0, 255) as f32 / 255.0);
    ([entity_color[0], entity_color[1], entity_color[2], alpha], pattern_length, pattern, line_weight_px, aci)
}

pub(super) fn resolve_pattern_native(
    table: &std::collections::BTreeMap<String, nm::LinetypeProperties>,
    name: &str,
    scale: f32,
) -> (f32, [f32; 8]) {
    let solid = (0.0, [0.0f32; 8]);
    if name.eq_ignore_ascii_case("continuous")
        || name.eq_ignore_ascii_case("bylayer")
        || name.eq_ignore_ascii_case("byblock")
        || name.is_empty()
    {
        return solid;
    }
    let lt = match table.get(name) {
        Some(lt) => lt,
        None => return solid,
    };
    if lt.is_continuous() || lt.segments.is_empty() {
        return solid;
    }

    let mut pat = [0.0f32; 8];
    let mut pat_len = 0.0f32;
    for (i, segment) in lt.segments.iter().take(8).enumerate() {
        let raw = segment.length as f32 * scale;
        let encoded = if raw == 0.0 {
            0.01 * scale.max(0.01)
        } else {
            raw
        };
        pat[i] = encoded;
        pat_len += encoded.abs();
    }
    if pat_len < 1e-6 {
        return solid;
    }
    (pat_len, pat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_style_native_uses_layer_aci_and_native_pattern() {
        let mut doc = nm::CadDocument::new();
        doc.layers.insert(
            "Walls".into(),
            nm::LayerProperties {
                name: "Walls".into(),
                color: 1,
                linetype_name: "Dash".into(),
                lineweight: 50,
                ..nm::LayerProperties::new("Walls")
            },
        );
        doc.linetypes.insert(
            "Dash".into(),
            nm::LinetypeProperties {
                name: "Dash".into(),
                pattern_length: 1.0,
                segments: vec![
                    nm::LinetypeSegment { length: 0.5 },
                    nm::LinetypeSegment { length: -0.25 },
                ],
                ..nm::LinetypeProperties::new("Dash")
            },
        );
        let mut entity = nm::Entity::new(nm::EntityData::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        });
        entity.layer_name = "Walls".into();
        entity.color_index = 256;
        entity.lineweight = -1;

        let (_color, pat_len, pattern, line_px, aci) = render_style_native(&doc, &entity);
        assert_eq!(aci, 1);
        assert!(pat_len > 0.0);
        assert_eq!(pattern[0], 0.5);
        assert_eq!(pattern[1], -0.25);
        assert!(line_px >= 1.0);
    }
}
