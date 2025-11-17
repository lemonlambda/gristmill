//! Hell where Entities and Components and Systems live

use anyhow::Result;
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::ecs::ordering::{Ordering, SystemOrder};

pub mod ordering;

pub type System = fn(World) -> Result<()>;

/// Should manage everything related to the ECS
pub struct Manager {
    pub world: World,
    pub startup_systems: SystemOrder,
    pub systems: SystemOrder,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            startup_systems: SystemOrder::empty(),
            systems: SystemOrder::empty(),
        }
    }
    pub fn add_startup_systems<S: Into<SystemOrder>>(mut self, systems: S) -> Self {
        self.startup_systems = systems.into();
        self
    }
    pub fn run(mut self) -> Result<()> {
        for system in self.startup_systems.order {
            system(self.world.clone())?;
        }

        Ok(())
    }
}

pub type Resource = HashMap<Box<dyn Any>, Box<dyn Any>>;
pub type Component = HashMap<Box<dyn Any>, Vec<Box<dyn Any>>>;

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
