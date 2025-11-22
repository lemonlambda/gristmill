use std::fmt::{Debug, Display};
use std::hash::Hash;

use anyhow::Result;
use cgmath::{vec2, vec3};
use egui::epaint::Primitive;
use egui::{CentralPanel, ClippedPrimitive, Context, ViewportId};
use egui_winit::State;
use log::*;
use vulkanalia::vk::*;
use winit::window::Window;

use crate::engine::vulkan::VulkanData;
use crate::engine::vulkan::buffer_manager::{
    AllocateBufferType, BufferManagerCopyType, BufferManagerDataType, BufferManagerRequirements,
    UniformBufferMaps,
};
use crate::engine::{
    vertex::Vertex,
    vulkan::buffer_manager::{BufferManager, StandardBufferMaps},
};

pub struct GuiVulkanInfo {
    pub buffer_count: u32,
    pub vertex_lengths: Vec<u32>,
    pub index_lengths: Vec<u32>,
}

impl GuiVulkanInfo {
    pub fn add_to_vertex_buffers(
        &mut self,
        buffer_manager: &mut BufferManager<StandardBufferMaps, UniformBufferMaps>,
        vertex_buffers: &mut Vec<Buffer>,
        vertex_lengths: &mut Vec<u32>,
    ) {
        for i in 0..=self.buffer_count {
            vertex_lengths.push(self.vertex_lengths[i as usize]);
            vertex_buffers.push(
                buffer_manager
                    .get_standard_buffer(StandardBufferMaps::GuiVertices(i as usize))
                    .buffer,
            );
        }
    }

    pub fn add_to_index_buffers(
        &mut self,
        buffer_manager: &mut BufferManager<StandardBufferMaps, UniformBufferMaps>,
        index_buffers: &mut Vec<Buffer>,
        index_lengths: &mut Vec<u32>,
    ) {
        for i in 0..=self.buffer_count {
            index_lengths.push(self.index_lengths[i as usize]);
            index_buffers.push(
                buffer_manager
                    .get_standard_buffer(StandardBufferMaps::GuiIndices(i as usize))
                    .buffer,
            );
        }
    }
}

pub struct GuiApp {
    state: State,
}

impl GuiApp {
    pub fn new(window: &Window) -> Self {
        let ctx = Context::default();
        let state = State::new(ctx, ViewportId::ROOT, &window, None, None);

        Self { state }
    }

    pub fn prepare(&mut self, window: &Window) -> Result<()> {
        Ok(())
    }

    pub fn render(&mut self, window: &Window) -> Result<Vec<(Vec<Vertex>, Vec<u16>)>> {
        let raw_input = self.state.egui_input();

        let full_output = self.state.egui_ctx().run(raw_input.clone(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                ui.label("Hello, World!");
                let _ = ui.button("Hello!!");
            });
        });

        self.state
            .handle_platform_output(window, full_output.platform_output);
        let primitives = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point)
            .into_iter()
            .map(|p| (p.to_vertices(), p.to_indices()))
            .collect();

        info!("Primitives: {:?}", primitives);

        Ok(primitives)
    }

    pub unsafe fn create_gui_buffers(
        &mut self,
        data: &mut VulkanData,
        window: &Window,
    ) -> Result<GuiVulkanInfo> {
        self.prepare(window)?;
        let vertices = self.render(window)?;
        let mut buffer_count = 0;
        let mut vertex_lengths = vec![];
        let mut index_lengths = vec![];
        for (i, (vertices, indices)) in vertices.into_iter().enumerate() {
            vertex_lengths.push(vertices.len() as u32);
            index_lengths.push(indices.len() as u32);
            unsafe {
                Self::create_gui_vertex_buffer(data, vertices, i)?;
                Self::create_gui_index_buffer(data, indices, i)?;
            }
            buffer_count = i;
        }

        Ok(GuiVulkanInfo {
            buffer_count: buffer_count as u32,
            vertex_lengths,
            index_lengths,
        })
    }

    pub unsafe fn create_gui_vertex_buffer(
        data: &mut VulkanData,
        vertices: Vec<Vertex>,
        idx: usize,
    ) -> Result<()> {
        unsafe {
            let vertex_buffer_size = (size_of::<Vertex>() * vertices.len()) as u64;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Temp,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                vertex_buffer_size,
            )?;

            data.buffer_manager
                .copy_data_to_buffer_with_size::<Vertex>(
                    BufferManagerDataType::Data(&vertices),
                    BufferManagerCopyType::TempBuffer,
                    vertex_buffer_size,
                )?;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Standard {
                    name: StandardBufferMaps::GuiVertices(idx),
                },
                BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
                vertex_buffer_size,
            )?;

            data.buffer_manager
                .copy_data_to_buffer_with_size::<Vertex>(
                    BufferManagerDataType::TempBuffer {
                        graphics_queue: data.graphics_queue,
                        command_pool: data.command_pool,
                    },
                    BufferManagerCopyType::StandardBuffer(StandardBufferMaps::GuiVertices(idx)),
                    vertex_buffer_size,
                )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }

    pub unsafe fn create_gui_index_buffer(
        data: &mut VulkanData,
        indices: Vec<u16>,
        idx: usize,
    ) -> Result<()> {
        unsafe {
            let index_buffer_size = (size_of::<u16>() * indices.len()) as u64;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Temp,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                index_buffer_size,
            )?;

            data.buffer_manager.copy_data_to_buffer(
                BufferManagerDataType::Data(&indices),
                BufferManagerCopyType::TempBuffer,
            )?;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Standard {
                    name: StandardBufferMaps::GuiIndices(idx),
                },
                BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
                index_buffer_size,
            )?;

            data.buffer_manager.copy_data_to_buffer_with_size::<u16>(
                BufferManagerDataType::TempBuffer {
                    graphics_queue: data.graphics_queue,
                    command_pool: data.command_pool,
                },
                BufferManagerCopyType::StandardBuffer(StandardBufferMaps::GuiIndices(idx)),
                index_buffer_size,
            )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }
}

pub trait ConvertForVulkan {
    fn to_vertices(&self) -> Vec<Vertex>;
    fn to_indices(&self) -> Vec<u16>;
}

impl ConvertForVulkan for ClippedPrimitive {
    fn to_vertices(&self) -> Vec<Vertex> {
        match self.primitive.clone() {
            Primitive::Mesh(mesh) => mesh
                .vertices
                .into_iter()
                .map(|v| {
                    Vertex::new(
                        vec3(v.pos.x, v.pos.y, 0.0),
                        vec3(
                            v.color.r() as f32 / 255.0,
                            v.color.g() as f32 / 255.0,
                            v.color.b() as f32 / 255.0,
                        ),
                        vec2(v.uv.x, v.uv.y),
                    )
                })
                .collect(),
            Primitive::Callback(_) => {
                panic!("Got a Primitive::Callback.");
            }
        }
    }

    fn to_indices(&self) -> Vec<u16> {
        match self.primitive.clone() {
            Primitive::Mesh(mesh) => mesh.indices.into_iter().map(|x| x as u16).collect(),
            Primitive::Callback(_) => {
                panic!("Got a Primitive::Callback.");
            }
        }
    }
}
