use anyhow::Result;
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowBuilder};
use winit::{dpi::LogicalSize, event_loop::EventLoop};

use crate::engine::vulkan::VulkanApp;

mod vulkan;

pub struct Engine {
    event_loop: EventLoop<()>,
    window: Window,
    vulkan_app: VulkanApp,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title("Factory Game")
            .with_inner_size(LogicalSize::new(1024, 768))
            .build(&event_loop)?;
        let vulkan_app = unsafe { VulkanApp::create(&window)? };

        Ok(Self {
            event_loop,
            window,
            vulkan_app,
        })
    }

    pub fn run(mut self) -> Result<()> {
        self.event_loop.run(move |event, elwt| {
            match event {
                // Request a redraw when all events were processed.
                Event::AboutToWait => self.window.request_redraw(),
                Event::WindowEvent { event, .. } => match event {
                    // Render a frame if our Vulkan app is not being destroyed.
                    WindowEvent::RedrawRequested if !elwt.exiting() => {
                        unsafe { self.vulkan_app.render(&self.window) }.unwrap()
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
