use anyhow::Result;
use sdl2::EventPump;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::{Sdl, video::Window};
use std::thread::sleep;
use std::time::Duration;

use crate::engine::vulkan::VulkanProperties;

pub mod vulkan;

pub struct Engine {
    frame_number: u64,
    stop_rendering: bool,

    window_handler: WindowHandler,

    vulkan: VulkanProperties,
}

impl Engine {
    pub fn new() -> Result<Self> {
        let mut vulkan_properties = VulkanProperties::new()?;
        vulkan_properties.init_vulkan()?;

        Ok(Self {
            frame_number: 0,
            stop_rendering: false,

            window_handler: WindowHandler::new("tringl", [1920, 1080]),

            vulkan: vulkan_properties,
        })
    }

    pub fn draw(&mut self) {}

    pub fn run(&mut self) {
        let canvas = &mut self.window_handler.canvas;

        canvas.present();
        let mut i = 0;
        'running: loop {
            for event in self.window_handler.event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,
                    Event::Window {
                        timestamp,
                        window_id,
                        win_event,
                    } => match win_event {
                        WindowEvent::Minimized => {
                            self.stop_rendering = true;
                        }
                        WindowEvent::Restored => {
                            self.stop_rendering = false;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            self.draw();

            if self.stop_rendering {
                sleep(Duration::new(0, 1_000_000_000u32 / 60));
            } else {
                sleep(Duration::new(0, 1_000_000_000u32 / 10));
            }
        }
    }
}

struct WindowHandler {
    current_window_size: [u32; 2],

    sdl_context: Sdl,
    window: Window,
    canvas: Canvas<Window>,
    event_pump: EventPump,
}

impl WindowHandler {
    pub fn new<S: ToString>(name: S, size: [u32; 2]) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem
            .window(name.to_string().as_str(), size[0], size[1])
            .position_centered()
            .build()
            .unwrap();

        let canvas = window.clone().into_canvas().build().unwrap();
        let event_pump = sdl_context.event_pump().unwrap();

        Self {
            current_window_size: size,

            sdl_context,
            window,
            canvas,
            event_pump,
        }
    }
}
