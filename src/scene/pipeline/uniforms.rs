use crate::scene::camera::Camera;
use iced::Rectangle;

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Uniforms {
    pub view_proj: glam::Mat4,
    pub camera_pos: glam::Vec4,
    pub viewport_size: [f32; 2],
    pub _pad: [f32; 2],
}

impl Uniforms {
    pub fn new(camera: &Camera, bounds: Rectangle) -> Self {
        Self {
            view_proj: camera.view_proj(bounds),
            camera_pos: camera.position_vec4(),
            viewport_size: [bounds.width, bounds.height],
            _pad: [0.0; 2],
        }
    }
}
