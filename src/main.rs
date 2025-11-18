#![feature(never_type, trait_alias, mapped_lock_guards, lock_value_accessors)]

extern crate pretty_env_logger;

use crate::ecs::ordering::SystemOrder;
use crate::ecs::{Direction, Event, EventData, EventSystem};
use crate::ecs::{Manager, System, World, ordering::Ordering};
use crate::engine::Engine;
use crate::logging::setup_logging;
use anyhow::Result;
use log::*;

mod ecs;
mod engine;
mod init;
mod logging;

fn main() -> Result<()> {
    setup_logging();

    let mut manager = Manager::new()
        .add_startup_systems(
            SystemOrder::<System>::new(test_system)
                .after(test_system_2)
                .after(test_system_3)
                .after(test_system),
        )
        .add_event_handler(
            Event::Movement,
            SystemOrder::<EventSystem>::new(movement_event_test),
        );

    manager.world.add_component(10);

    manager.run()?;

    let engine = Engine::new()?;

    engine.run()?;

    Ok(())
}

pub fn movement_event_test(world: &World, data: EventData) -> Result<()> {
    info!("does this even run?");

    Ok(())
}

pub fn test_system(world: &World) -> Result<()> {
    info!("Hello from test_system!");

    let comps = world.get_components::<i32>();

    for comp in comps {
        info!("Got component: {comp}");
    }

    Ok(())
}

pub fn test_system_2(world: &World) -> Result<()> {
    info!("Hello from test_system_2!");

    world.raise_event(Event::Movement, EventData::Movement(Direction::Up));

    Ok(())
}

pub fn test_system_3(world: &World) -> Result<()> {
    info!("Hello from test_system_3!");

    let mut comps = world.get_components_mut::<i32>();

    **comps[0] += 10;

    Ok(())
}
