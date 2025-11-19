use std::{
    any::{Any, TypeId},
    collections::HashMap,
    rc::Rc,
    sync::RwLock,
};

use crate::ecs::{
    Component, EventSystem, Resource, System, WinitEventSystem,
    events::{EventWrapper, LemgineEvent},
    ordering::SystemOrder,
};

pub struct PartialManager {
    pub resources: HashMap<TypeId, Resource>,
    pub components: HashMap<TypeId, Vec<Component>>,
    pub startup_systems: SystemOrder<System>,
    pub systems: SystemOrder<System>,
    pub winit_event_systems: SystemOrder<WinitEventSystem>,
    pub event_systems: HashMap<LemgineEvent, SystemOrder<EventSystem>>,
}

impl PartialManager {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            components: HashMap::new(),
            startup_systems: SystemOrder::empty(),
            systems: SystemOrder::empty(),
            winit_event_systems: SystemOrder::empty(),
            event_systems: HashMap::new(),
        }
    }

    pub fn add_resource<T: Any>(mut self, resource: T) -> Self {
        self.resources
            .entry(TypeId::of::<T>())
            .or_insert(Rc::new(RwLock::new(Box::new(resource))));
        self
    }

    pub fn add_component<T: Any>(mut self, component: T) -> Self {
        if let Some(value) = self.components.get_mut(&TypeId::of::<T>()) {
            value.push(Rc::new(RwLock::new(Box::new(component))));
        } else {
            self.components.insert(
                TypeId::of::<T>(),
                vec![Rc::new(RwLock::new(Box::new(component)))],
            );
        }
        self
    }

    pub fn add_startup_systems<S: Into<SystemOrder<System>>>(mut self, systems: S) -> Self {
        self.startup_systems = systems.into();
        self
    }

    pub fn add_winit_event_systems<S: Into<SystemOrder<WinitEventSystem>>>(
        mut self,
        systems: S,
    ) -> Self {
        self.winit_event_systems = systems.into();
        self
    }

    pub fn add_systems<S: Into<SystemOrder<System>>>(mut self, systems: S) -> Self {
        self.systems = systems.into();
        self
    }

    pub fn add_event_handler<E: EventWrapper + 'static, S: Into<SystemOrder<EventSystem>>>(
        mut self,
        event: E,
        system: S,
    ) -> Self {
        self.event_systems
            .entry(Box::new(event))
            .or_insert(system.into());
        self
    }
}
