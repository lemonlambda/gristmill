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
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
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

    let vulkan_info = VulkanInfo {
        context,
        framebuffers,
        command_buffer_allocator,
        previous_frame_end,
        vertices,
    };

    let mut app = App {
        vulkan_info,

        first: true,
        keys_down: KeysDown::default(),

        mode: Mode::default(),
        request_redraw: false,
        recreate_swapchain: false,
        wait_cancelled: false,
        close_requested: false,
        window: None,
    };

    event_loop.run_app(&mut app).unwrap()
}

const WAIT_TIME: time::Duration = time::Duration::from_millis(1000 / 60);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(1000 / 60);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wait,
    WaitUntil,
    #[default]
    Poll,
}

pub struct VulkanInfo {
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub context: VulkanContext,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub previous_frame_end: Option<Box<(dyn GpuFuture + 'static)>>,
    pub vertices: Vec<MyVertex>,
}

#[derive(Debug, Default, Clone, Copy)]
struct KeysDown {
    w: bool,
    s: bool,
    a: bool,
    d: bool,
}

struct App {
    mode: Mode,
    request_redraw: bool,
    recreate_swapchain: bool,
    wait_cancelled: bool,
    close_requested: bool,
    window: Option<Window>,

    vulkan_info: VulkanInfo,

    keys_down: KeysDown,

    first: bool,
}

impl App {
    fn on_first(&mut self) {
        self.vulkan_info.vertices.push(MyVertex {
            position: [-0.75, -0.75],
            color: [0.0, 0.0, 1.0],
        });
        self.vulkan_info.vertices.push(MyVertex {
            position: [0.75, -0.75],
            color: [0.0, 1.0, 0.0],
        });
        self.vulkan_info.vertices.push(MyVertex {
            position: [0.75, 0.75],
            color: [1.0, 0.0, 0.0],
        });

        let vertices = self.vulkan_info.vertices.clone();

        self.vulkan_info.context.update_vertices(&vertices);
    }

    fn on_run(&mut self) {
        // self.vulkan_info.context.viewport.offset[0] += 5.0;
        println!("{:#?}", self.keys_down);

        if self.keys_down.w {
            self.vulkan_info.context.viewport.offset[1] -= 5.0;
        }
        if self.keys_down.s {
            self.vulkan_info.context.viewport.offset[1] += 5.0;
        }
        if self.keys_down.a {
            self.vulkan_info.context.viewport.offset[0] -= 5.0;
        }
        if self.keys_down.d {
            self.vulkan_info.context.viewport.offset[0] += 5.0;
        }
    }
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
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => {
                if event.repeat {
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(code) => {
                        let state = match event.state {
                            ElementState::Pressed => true,
                            ElementState::Released => false,
                        };
                        match code {
                            KeyCode::KeyW => self.keys_down.w = state,
                            KeyCode::KeyS => self.keys_down.s = state,
                            KeyCode::KeyA => self.keys_down.a = state,
                            KeyCode::KeyD => self.keys_down.d = state,
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => self.request_redraw = true,
            _ => (),
        }

        if self.first {
            self.on_first();

            self.first = false
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.request_redraw && !self.wait_cancelled && !self.close_requested {
            self.vulkan_info.context.handle_redraw_events_cleared(
                self.vulkan_info.context.window.clone(), // `surface` must come from a `Window`
                &self.vulkan_info.command_buffer_allocator,
                &mut self.vulkan_info.previous_frame_end,
                &mut self.recreate_swapchain,
                &mut self.vulkan_info.framebuffers,
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

        self.on_run();

        if self.close_requested {
            event_loop.exit();
        }
    }
}
