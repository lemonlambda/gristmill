use std::mem::size_of;

use cgmath::{vec2, vec3};
use vulkanalia::vk::{
    Format, HasBuilder, VertexInputAttributeDescription, VertexInputBindingDescription,
    VertexInputRate,
};

pub type Vec2 = cgmath::Vector2<f32>;
pub type Vec3 = cgmath::Vector3<f32>;
pub type Mat4 = cgmath::Matrix4<f32>;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pos: Vec3,
    color: Vec3,
    tex_coord: Vec2,
}

impl Vertex {
    const fn new(pos: Vec3, color: Vec3, tex_coord: Vec2) -> Self {
        Self {
            pos,
            color,
            tex_coord,
        }
    }
    pub fn binding_description() -> VertexInputBindingDescription {
        VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(VertexInputRate::VERTEX)
            .build()
    }
    pub fn attribute_descriptions() -> [VertexInputAttributeDescription; 3] {
        let pos = VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(Format::R32G32B32_SFLOAT)
            .offset(0)
            .build();

        let color = VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(Format::R32G32B32_SFLOAT)
            .offset(size_of::<Vec2>() as u32)
            .build();

        let tex_coord = VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(Format::R32G32_SFLOAT)
            .offset((size_of::<Vec2>() + size_of::<Vec3>()) as u32)
            .build();

        [pos, color, tex_coord]
    }
}

pub static VERTICES: [Vertex; 3] = [
    Vertex::new(vec3(0.0, 0.5, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)), // A
    Vertex::new(vec3(-0.5, -0.5, 0.0), vec3(0.0, 0.0, 1.0), vec2(1.0, 1.0)), // C
    Vertex::new(vec3(0.5, -0.5, 0.0), vec3(1.0, 1.0, 1.0), vec2(1.0, 0.0)), // D
];

pub const INDICES: &[u16] = &[0, 1, 2];

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct UniformBufferObject {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}
