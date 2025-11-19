use anyhow::Result;
use log::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::WindowBuilder;
use winit::{dpi::LogicalSize, event_loop::EventLoop};

use crate::ecs::World;
use crate::engine::vulkan::VulkanApp;

mod vertex;
mod vulkan;

pub struct Engine {
    vulkan_app: VulkanApp,
    minimized: bool,
}

impl Engine {
    pub fn new(event_loop: &EventLoop<()>) -> Result<Self> {
        let window = WindowBuilder::new()
            .with_title("Factory Game")
            .with_inner_size(LogicalSize::new(1024, 768))
            .build(event_loop)?;
        info!("Creating vulkan app");
        let vulkan_app = unsafe { VulkanApp::create(window)? };
        Ok(Self {
            vulkan_app,
            minimized: false,
        })
    }
}

pub fn engine_main(
    world: &World,
    event: Event<()>,
    elwt: &EventLoopWindowTarget<()>,
) -> Result<()> {
    let mut engine = world.try_get_resource_mut::<Engine>();

    if let None = engine {
        warn!("Couldn't get engine resource!");
        return Ok(());
    }

    let mut engine = engine.unwrap();

    match event {
        // Request a redraw when all events were processed.
        Event::AboutToWait => engine.vulkan_app.window.request_redraw(),
        Event::WindowEvent { event, .. } => match event {
            // Render a frame if our Vulkan app is not being destroyed.
            WindowEvent::RedrawRequested if !elwt.exiting() && !engine.minimized => unsafe {
                engine.vulkan_app.render().unwrap()
            },
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    engine.minimized = true;
                } else {
                    engine.minimized = false;
                    engine.vulkan_app.resized = true;
                }
            }
            // Destroy our Vulkan app.
            WindowEvent::CloseRequested => {
                elwt.exit();
                unsafe {
                    engine.vulkan_app.destroy();
                }
            }
            _ => {}
        },
        _ => {}
    }

    Ok(())
}
