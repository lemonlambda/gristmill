#![feature(never_type)]

use crate::engine::Engine;

use anyhow::Result;

mod engine;
mod init;

fn main() -> Result<()> {
    pretty_env_logger::init();

    let mut engine = Engine::new()?;

    engine.run()?;

    Ok(())
}
