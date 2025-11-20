use std::time::Instant;

use anyhow::Result;
use log::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::WindowBuilder;
use winit::{dpi::LogicalSize, event_loop::EventLoop};

use crate::ecs::order_up::OrderUp;
use crate::ecs::{WinitEventSystem, World};
use crate::engine::vulkan::VulkanApp;
use crate::systems::prelude::PartialManager;

mod vertex;
mod vulkan;

pub struct Engine {
    pub vulkan_app: VulkanApp,
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

const DT_FPS_60_NANO: u128 = 1_000_000_000 / 60;
pub struct RedrawTime(Instant);
pub struct FPSCounter(u32);

pub fn engine_partial() -> PartialManager {
    PartialManager::new()
        .add_winit_event_systems((engine_main as WinitEventSystem,).order_up())
        .add_resource(RedrawTime(Instant::now()))
        .add_resource(FPSCounter(0))
}

pub fn engine_main(
    world: &World,
    event: Event<()>,
    elwt: &EventLoopWindowTarget<()>,
) -> Result<()> {
    let engine = world.try_get_resource_mut::<Engine>();

    if engine.is_none() {
        warn!("Couldn't get engine resource!");
        return Ok(());
    }

    let mut engine = engine.unwrap();
    let mut redraw_time = world.get_resource_mut::<RedrawTime>();
    let mut fps_counter = world.get_resource_mut::<FPSCounter>();

    fps_counter += 1;
    info!("FPS: {}", fps_counter.0);

    match event {
        // Request a redraw when all events were processed.
        Event::AboutToWait => engine.vulkan_app.window.request_redraw(),
        Event::WindowEvent { event, .. } => match event {
            // Render a frame if our Vulkan app is not being destroyed.
            WindowEvent::RedrawRequested
                if !elwt.exiting()
                    && !engine.minimized
                    && redraw_time.0.elapsed().as_nanos() > DT_FPS_60_NANO =>
            unsafe {
                engine.vulkan_app.render().unwrap();
                redraw_time.0 = Instant::now();
                fps_counter.0 = 0;
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
