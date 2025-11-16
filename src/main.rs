#![feature(never_type, trait_alias)]

extern crate pretty_env_logger;

use crate::engine::Engine;
use crate::logging::setup_logging;
use anyhow::Result;

mod engine;
mod init;
mod logging;

fn main() -> Result<()> {
    setup_logging();

    let engine = Engine::new()?;

    engine.run()?;

    Ok(())
}
