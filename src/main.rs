use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::time;
use vulkano::render_pass::Framebuffer;
use vulkano::{
    command_buffer::allocator::StandardCommandBufferAllocator,
    sync::{self, GpuFuture},
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::rendering::{MyVertex, VulkanContext, window_size_dependent_setup};

mod rendering;

fn main() {
    let event_loop = EventLoop::new().unwrap();

    let mut context = VulkanContext::new(&event_loop);
    let mut framebuffers = window_size_dependent_setup(
        &context.images,
        context.render_pass.clone(),
        &mut context.viewport,
    );

    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(context.device.clone(), Default::default());

    let mut previous_frame_end = Some(sync::now(context.device.clone()).boxed());

    let mut vertices = vec![
        MyVertex {
            position: [0.0, -0.5],
            color: [1.0, 0.0, 0.0],
        },
        MyVertex {
            position: [-0.5, 0.5],
            color: [0.0, 1.0, 0.0],
        },
        MyVertex {
            position: [0.5, 0.5],
            color: [0.0, 0.0, 1.0],
        },
    ];

    let vulkan_info = Arc::new(RwLock::new(VulkanInfo {
        context,
        framebuffers,
        command_buffer_allocator,
        previous_frame_end,
        vertices,
    }));

    let mut app = App {
        vulkan_info,

        first: true,

        mode: Mode::default(),
        request_redraw: false,
        recreate_swapchain: false,
        wait_cancelled: false,
        close_requested: false,
        window: None,
    };

    event_loop.run_app(&mut app).unwrap()
}

const WAIT_TIME: time::Duration = time::Duration::from_millis(100);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(100);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    Wait,
    WaitUntil,
    Poll,
}

struct VulkanInfo {
    framebuffers: Vec<Arc<Framebuffer>>,
    context: VulkanContext,
    command_buffer_allocator: StandardCommandBufferAllocator,
    previous_frame_end: Option<Box<(dyn GpuFuture + 'static)>>,
    vertices: Vec<MyVertex>,
}

struct App {
    mode: Mode,
    request_redraw: bool,
    recreate_swapchain: bool,
    wait_cancelled: bool,
    close_requested: bool,
    window: Option<Window>,

    vulkan_info: Arc<RwLock<VulkanInfo>>,

    first: bool,
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        // info!("new_events: {cause:?}");

        self.wait_cancelled = match cause {
            StartCause::WaitCancelled { .. } => self.mode == Mode::WaitUntil,
            _ => false,
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // info!("{event:?}");

        match event {
            WindowEvent::CloseRequested => {
                self.close_requested = true;
            }
            WindowEvent::Resized(_) => {
                self.request_redraw = true;
                self.recreate_swapchain = true;
            }
            WindowEvent::RedrawRequested => self.request_redraw = true,
            _ => (),
        }

        let mut vk_info_locked = self.vulkan_info.write().unwrap();
        if self.first {
            vk_info_locked.vertices.push(MyVertex {
                position: [-0.75, -0.75],
                color: [0.0, 0.0, 1.0],
            });
            vk_info_locked.vertices.push(MyVertex {
                position: [0.75, -0.75],
                color: [0.0, 1.0, 0.0],
            });
            vk_info_locked.vertices.push(MyVertex {
                position: [0.75, 0.75],
                color: [1.0, 0.0, 0.0],
            });

            vk_info_locked
                .context
                .update_vertices(&vk_info_locked.vertices);

            self.first = false
        }

        vk_info_locked.context.viewport.offset[0] += 0.01;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.request_redraw && !self.wait_cancelled && !self.close_requested {
            let vk_info_locked = self.vulkan_info.write().unwrap();
            vk_info_locked.context.handle_redraw_events_cleared(
                vk_info_locked.context.window.clone(), // `surface` must come from a `Window`
                &vk_info_locked.command_buffer_allocator,
                &mut vk_info_locked.previous_frame_end,
                &mut vk_info_locked.recreate_swapchain,
                &mut vk_info_locked.framebuffers,
            );
        }

        match self.mode {
            Mode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            Mode::WaitUntil => {
                if !self.wait_cancelled {
                    event_loop
                        .set_control_flow(ControlFlow::WaitUntil(time::Instant::now() + WAIT_TIME));
                }
            }
            Mode::Poll => {
                thread::sleep(POLL_SLEEP_TIME);
                event_loop.set_control_flow(ControlFlow::Poll);
            }
        };

        if self.close_requested {
            event_loop.exit();
        }
    }
}
