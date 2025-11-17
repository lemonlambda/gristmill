use crate::ecs::{System, World};
use anyhow::Result;

#[derive(Clone)]
pub struct SystemOrder {
    pub order: Vec<System>,
}

impl SystemOrder {
    pub fn empty() -> Self {
        Self { order: vec![] }
    }
}

pub trait Ordering {
    type SystemType;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder;
}

impl Ordering for System {
    type SystemType = System;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder {
        SystemOrder {
            order: vec![*self, system],
        }
    }
}

impl Ordering for SystemOrder {
    type SystemType = System;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder {
        self.order.push(system);
        self.clone()
    }
}

impl From<System> for SystemOrder {
    fn from(val: System) -> Self {
        SystemOrder { order: vec![val] }
    }
}
