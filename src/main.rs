#![feature(never_type, trait_alias)]

extern crate pretty_env_logger;

use crate::engine::Engine;
use anyhow::Result;
use std::env;

mod engine;
mod init;

fn main() -> Result<()> {
    // There is multiple logging levels in highest priority to lowest
    // error
    // warn
    // info
    // debug
    // trace
    // off (no logs)
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "info") };
    }

    pretty_env_logger::init();

    let engine = Engine::new()?;

    engine.run()?;

    Ok(())
}
