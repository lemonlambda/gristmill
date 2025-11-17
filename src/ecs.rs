//! Hell where Entities and Components and Systems live

use anyhow::Result;
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

type System = fn(World) -> Result<()>;

/// Should manage everything related to the ECS
pub struct Manager {
    pub world: World,
    pub startup_systems: Vec<System>,
    pub systems: Vec<System>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            startup_systems: vec![],
            systems: vec![],
        }
    }
    pub fn add_startup_system(mut self, system: System) -> Self {
        self.startup_systems.push(system);
        self
    }
    pub fn run(mut self) -> Result<()> {
        for system in self.startup_systems {
            system(self.world.clone())?;
        }

        Ok(())
    }
}

type Resource = HashMap<Box<dyn Any>, Box<dyn Any>>;
type Component = HashMap<Box<dyn Any>, Vec<Box<dyn Any>>>;

/// A whole new world!
#[derive(Clone)]
pub struct World {
    resources: Arc<RwLock<Resource>>,
    components: Arc<RwLock<Component>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            components: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
