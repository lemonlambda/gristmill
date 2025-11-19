use anyhow::Result;
use log::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoopWindowTarget,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::ecs::{
    World,
    events::{EcsEvent, EcsEventData, LemgineEventData},
};

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum MovementEvent {
    Moved,
}

impl EcsEvent for MovementEvent {}

#[derive(Clone)]
pub struct MovementData {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl EcsEventData for MovementData {}

pub fn handle_movement(_world: &World, _event_data: LemgineEventData) -> Result<()> {
    info!("Got some movement here!");

    Ok(())
}

pub fn get_movement(world: &World, event: Event<()>, _: &EventLoopWindowTarget<()>) -> Result<()> {
    let mut movement_data = MovementData {
        up: false,
        down: false,
        left: false,
        right: false,
    };
    let mut changed = false;

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
                movement_data.left = true;
                changed = true;
            }
            KeyCode::KeyD => {
                movement_data.right = true;
                changed = true;
            }
            KeyCode::KeyW => {
                movement_data.up = true;
                changed = true;
            }
            KeyCode::KeyS => {
                movement_data.down = true;
                changed = true;
            }
            _ => {}
        }
    }

    if changed {
        world.raise_event(MovementEvent::Moved, movement_data);
    }

    Ok(())
}
