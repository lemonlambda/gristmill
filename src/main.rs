#![feature(never_type, trait_alias, mapped_lock_guards, lock_value_accessors)]

extern crate pretty_env_logger;

use crate::ecs::Manager;
use crate::logging::setup_logging;
use anyhow::Result;

mod ecs;
mod engine;
mod init;
mod logging;

fn main() -> Result<()> {
    setup_logging();

    let mut manager = Manager::new()?;

    manager.world.add_component(10);

    manager.run()?;

    Ok(())
}
