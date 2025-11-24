use std::f32::consts::SQRT_2;

use anyhow::Result;
use log::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoopWindowTarget,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    DeltaTime,
    ecs::{System, WinitEventSystem, World, order_up::OrderUp, partial_manager::PartialManager},
    engine::Engine,
};

#[derive(Clone)]
pub struct MovementData {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    pressed: bool,
}

impl MovementData {
    fn diagonal(&self) -> bool {
        ((self.up || self.down) && (self.left || self.right))
            && (!self.up || !self.down || (!self.left || !self.right))
    }
}

pub fn movement_partial() -> PartialManager {
    PartialManager::new()
        .add_winit_event_systems((get_movement as WinitEventSystem,).order_up())
        .add_systems((update_movement as System,).order_up())
        .add_resource(MovementData {
            up: false,
            down: false,
            left: false,
            right: false,
            pressed: false,
        })
}

pub fn update_movement(world: &World) -> Result<()> {
    let movement_data_resource = world.get_resource::<MovementData>();
    let mut engine_resource = world.get_resource_mut::<Engine>();

    let delta_time = world.get_resource::<DeltaTime>().0;

    let mut value = 1.0;

    if movement_data_resource.diagonal() {
        value = 1.0 / SQRT_2;
    }

    if movement_data_resource.up {
        engine_resource.vulkan_app.camera_position[1] -= value * delta_time;
    }
    if movement_data_resource.down {
        engine_resource.vulkan_app.camera_position[1] += value * delta_time;
    }
    if movement_data_resource.left {
        engine_resource.vulkan_app.camera_position[0] += value * delta_time;
    }
    if movement_data_resource.right {
        engine_resource.vulkan_app.camera_position[0] -= value * delta_time;
    }

    info!(
        "Camera Position: {:?}",
        engine_resource.vulkan_app.camera_position
    );

    Ok(())
}

pub fn get_movement(world: &World, event: Event<()>, _: &EventLoopWindowTarget<()>) -> Result<()> {
    let mut movement_data = world.get_resource_mut::<MovementData>();

    if let Event::WindowEvent {
        window_id: _,
        event,
    } = event
        && let WindowEvent::KeyboardInput {
            device_id: _,
            event,
            is_synthetic: _,
        } = event
        && let PhysicalKey::Code(code) = event.physical_key
    {
        match code {
            KeyCode::KeyA => {
                movement_data.left = event.state.is_pressed();
            }
            KeyCode::KeyD => {
                movement_data.right = event.state.is_pressed();
            }
            KeyCode::KeyW => {
                movement_data.up = event.state.is_pressed();
            }
            KeyCode::KeyS => {
                movement_data.down = event.state.is_pressed();
            }
            _ => {}
        }
    }

    Ok(())
}
