// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

// Welcome to the triangle example!
//
// This is the only example that is entirely detailed. All the other examples avoid code
// duplication by using helper functions.
//
// This example assumes that you are already more or less familiar with graphics programming and
// that you want to learn Vulkan. This means that for example it won't go into details about what a
// vertex or a shader is.

use vulkano::{
    command_buffer::allocator::StandardCommandBufferAllocator,
    sync::{self, GpuFuture},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::rendering::{VulkanContext, window_size_dependent_setup};

mod rendering;

fn main() {
    let event_loop = EventLoop::new();
    let mut context = VulkanContext::new(&event_loop);

    let mut framebuffers = window_size_dependent_setup(
        &context.images,
        context.render_pass.clone(),
        &mut context.viewport,
    );

    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(context.device.clone(), Default::default());

    let mut recreate_swapchain = false;
    let mut previous_frame_end = Some(sync::now(context.device.clone()).boxed());

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => recreate_swapchain = true,

            Event::RedrawEventsCleared => {
                context.handle_redraw_events_cleared(
                    context.window.clone(), // `surface` must come from a `Window`
                    &command_buffer_allocator,
                    &mut previous_frame_end,
                    &mut recreate_swapchain,
                    &mut framebuffers,
                );
            }

            _ => (),
        }
    });
}
