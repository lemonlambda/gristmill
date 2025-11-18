#![feature(never_type, trait_alias, mapped_lock_guards, lock_value_accessors)]

extern crate pretty_env_logger;

use crate::ecs::ordering::{Ordering, SystemOrder};
use crate::ecs::{EventSystem, Manager, WinitEventSystem};
use crate::engine::engine_main;
use crate::logging::setup_logging;
use crate::systems::movement::{MovementEvent, get_movement, handle_movement};
use anyhow::Result;

mod ecs;
mod engine;
mod init;
mod logging;
mod systems;

fn main() -> Result<()> {
    setup_logging();

    let manager = Manager::new()?
        .add_winit_event_systems(
            SystemOrder::<WinitEventSystem>::new(engine_main).after(get_movement),
        )
        .add_event_handler(
            MovementEvent::Moved,
            SystemOrder::<EventSystem>::new(handle_movement),
        );

    manager.run()?;

    Ok(())
}
