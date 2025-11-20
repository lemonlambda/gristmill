#![feature(never_type, trait_alias, mapped_lock_guards, lock_value_accessors)]

extern crate pretty_env_logger;

use std::time::Instant;

use crate::ecs::order_up::OrderUp;
use crate::ecs::{Manager, System, WinitEventSystem, World};
use crate::engine::engine_main;
use crate::logging::setup_logging;
use crate::systems::movement::movement_partial;
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
        .add_systems((update_delta_time as System,).order_up())
        .add_resource(DeltaTime(0.0))
        .add_resource(LastTime(Instant::now()))
        .integrate(movement_partial())?;

    manager.run()?;

    Ok(())
}

pub struct DeltaTime(pub f32);
pub struct LastTime(Instant);

pub fn update_delta_time(world: &World) -> Result<()> {
    let now = Instant::now();

    let dt = {
        let mut t = world.get_resource_mut::<LastTime>();
        let delta = now - t.0;
        t.0 = now;
        delta.as_secs_f32()
    };

    let mut delta_time = world.get_resource_mut::<DeltaTime>();
    delta_time.0 = dt;

    Ok(())
}
