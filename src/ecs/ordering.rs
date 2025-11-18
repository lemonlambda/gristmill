use crate::ecs::System;

#[derive(Clone)]
pub struct SystemOrder<T> {
    pub order: Vec<T>,
}

impl<T> SystemOrder<T> {
    pub fn new(system: T) -> Self {
        SystemOrder {
            order: vec![system],
        }
    }

    pub fn empty() -> Self {
        Self { order: vec![] }
    }
}

pub trait Ordering<T> {
    type SystemType;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T>;
}

impl Ordering<System> for System {
    type SystemType = System;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<System> {
        SystemOrder {
            order: vec![*self, system],
        }
    }
}

impl Ordering<System> for SystemOrder<System> {
    type SystemType = System;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<System> {
        self.order.push(system);
        self.clone()
    }
}

impl From<System> for SystemOrder<System> {
    fn from(val: System) -> Self {
        SystemOrder { order: vec![val] }
    }
}
