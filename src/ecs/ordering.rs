use crate::ecs::{EventSystem, WinitEventSystem};

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

    pub fn extend(mut self, other: SystemOrder<T>) -> Self {
        self.order.extend(other.order);
        self
    }

    pub fn extend_mut_ref(&mut self, other: SystemOrder<T>) {
        self.order.extend(other.order);
    }

    pub fn empty() -> Self {
        Self { order: vec![] }
    }
}

pub trait Ordering<T> {
    type SystemType;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T>;
}

impl<T: Clone> Ordering<T> for SystemOrder<T> {
    type SystemType = T;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T> {
        self.order.push(system);
        self.clone()
    }
}

impl<T: Copy> Ordering<T> for T {
    type SystemType = T;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T> {
        SystemOrder {
            order: vec![*self, system],
        }
    }
}
