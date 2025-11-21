use std::time::Instant;

use anyhow::Result;
use log::*;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::WindowBuilder;
use winit::{dpi::LogicalSize, event_loop::EventLoop};

use crate::ecs::order_up::OrderUp;
use crate::ecs::{StartupSystem, WinitEventSystem, World};
use crate::engine::gui::GuiApp;
use crate::engine::vulkan::VulkanApp;
use crate::systems::prelude::PartialManager;

mod gui;
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
        .add_startup_systems((engine_startup as StartupSystem,).order_up())
        .add_winit_event_systems((engine_main as WinitEventSystem,).order_up())
}

pub fn engine_startup(world: &mut World, event_loop: &EventLoop<()>) -> Result<()> {
    let engine = Engine::new(event_loop)?;

    world.add_resource(RedrawTime(Instant::now()));
    world.add_resource(FPSCounter(0));
    world.add_resource(engine);

    Ok(())
}

pub fn engine_main(
    world: &World,
    event: Event<()>,
    elwt: &EventLoopWindowTarget<()>,
) -> Result<()> {
    let redraw_time = world.try_get_resource_mut::<RedrawTime>();

    if redraw_time.is_none() {
        warn!("Redraw Time is none.");
        return Ok(());
    }

    let mut redraw_time = redraw_time.unwrap();

    let fps_counter = world.try_get_resource_mut::<FPSCounter>();

    if fps_counter.is_none() {
        warn!("FPS Counter is none.");
        return Ok(());
    }

    let mut fps_counter = fps_counter.unwrap();

    let engine = world.try_get_resource_mut::<Engine>();

    if engine.is_none() {
        warn!("Couldn't get engine resource!");
        return Ok(());
    }

    let mut engine = engine.unwrap();

    fps_counter.0 += 1;

    match event {
        // Request a redraw when all events were processed.
        Event::AboutToWait => {
            engine.vulkan_app.window.request_redraw();
        }
        Event::WindowEvent { event, .. } => match event {
            // Render a frame if our Vulkan app is not being destroyed.
            WindowEvent::RedrawRequested
                if !elwt.exiting()
                    && !engine.minimized
                    && redraw_time.0.elapsed().as_nanos() > DT_FPS_60_NANO =>
            unsafe {
                info!("FPS: {}", fps_counter.0);

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
