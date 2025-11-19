#![feature(never_type, trait_alias, mapped_lock_guards, lock_value_accessors)]

extern crate pretty_env_logger;

use crate::ecs::order_up::OrderUp;
use crate::ecs::ordering::{Ordering, SystemOrder};
use crate::ecs::{EventSystem, Manager, WinitEventSystem};
use crate::engine::engine_main;
use crate::logging::setup_logging;
use crate::systems::movement::{MovementEvent, get_movement, handle_movement, movement_partial};
use anyhow::Result;

mod ecs;
mod engine;
mod init;
mod logging;
mod systems;

fn main() -> Result<()> {
    setup_logging();

    let manager = Manager::new()?
        .add_winit_event_systems((engine_main as WinitEventSystem,).order_up())
        .integrate(movement_partial())?;

    manager.run()?;

    Ok(())
}
