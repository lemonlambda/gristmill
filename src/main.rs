#![feature(never_type, trait_alias)]

extern crate pretty_env_logger;

use crate::ecs::{Manager, World};
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

    let manager = Manager::new().add_startup_system(test_system).run()?;

    let engine = Engine::new()?;

    engine.run()?;

    Ok(())
}

pub fn test_system(world: World) -> Result<()> {
    info!("Hello from test_system!");

    Ok(())
}
