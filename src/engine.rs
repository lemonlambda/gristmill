use anyhow::Result;
use log::info;
use winit::event::{Event, WindowEvent};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowBuilder};
use winit::{dpi::LogicalSize, event_loop::EventLoop};

use crate::engine::vulkan::VulkanApp;

mod vertex;
mod vulkan;

pub struct PositionState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

pub struct Engine {
    event_loop: EventLoop<()>,
    window: Window,
    vulkan_app: VulkanApp,
    minimized: bool,
    position_state: PositionState,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title("Factory Game")
            .with_inner_size(LogicalSize::new(1024, 768))
            .build(&event_loop)?;
        info!("Creating vulkan app");
        let mut vulkan_app = unsafe { VulkanApp::create(&window)? };
        unsafe { vulkan_app.setup_vulkan(&window)? };

        Ok(Self {
            event_loop,
            window,
            vulkan_app,
            minimized: false,
            position_state: PositionState {
                up: false,
                down: false,
                left: false,
                right: false,
            },
        })
    }

    pub fn run(mut self) -> Result<()> {
        self.event_loop.run(move |event, elwt| {
            if self.position_state.up {
                self.vulkan_app.camera_position[1] -= 0.01;
            }
            if self.position_state.down {
                self.vulkan_app.camera_position[1] += 0.01;
            }
            if self.position_state.left {
                self.vulkan_app.camera_position[0] += 0.01;
            }
            if self.position_state.right {
                self.vulkan_app.camera_position[0] -= 0.01;
            }

            match event {
                // Request a redraw when all events were processed.
                Event::AboutToWait => self.window.request_redraw(),
                Event::WindowEvent { event, .. } => match event {
                    // Render a frame if our Vulkan app is not being destroyed.
                    WindowEvent::RedrawRequested if !elwt.exiting() && !self.minimized => {
                        unsafe { self.vulkan_app.render(&self.window) }.unwrap()
                    }
                    WindowEvent::Resized(size) => {
                        if size.width == 0 || size.height == 0 {
                            self.minimized = true;
                        } else {
                            self.minimized = false;
                            self.vulkan_app.resized = true;
                        }
                    }
                    WindowEvent::KeyboardInput {
                        device_id,
                        event,
                        is_synthetic,
                    } => {
                        if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                            match key_code {
                                KeyCode::KeyA => {
                                    self.position_state.left = event.state.is_pressed();
                                }
                                KeyCode::KeyD => {
                                    self.position_state.right = event.state.is_pressed();
                                }
                                KeyCode::KeyW => {
                                    self.position_state.up = event.state.is_pressed();
                                }
                                KeyCode::KeyS => {
                                    self.position_state.down = event.state.is_pressed();
                                }
                                _ => {}
                            }
                        }
                    }
                    // Destroy our Vulkan app.
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                        unsafe {
                            self.vulkan_app.destroy();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        })?;

        Ok(())
    }
}
