use std::{
    any::{Any, TypeId},
    collections::HashMap,
    rc::Rc,
    sync::RwLock,
};

use crate::ecs::{
    Component, EventSystem, Resource, StartupSystem, System, WinitEventSystem,
    events::{EventWrapper, LemgineEvent},
    ordering::SystemOrder,
};

/// A way to create a local version of [`Manager`] that can be tacked onto
/// a main [`Manager`] later on.
///
/// # Common Use Case
/// ```rs
/// fn partial_man() -> PartialManager {
///     PartialManager::new().add_systems((system1, system2).order_up())
/// }
///
/// fn main() {
///     Manager::new().integrate(partial_man());
/// }
/// ```
pub struct PartialManager {
    pub resources: HashMap<TypeId, Resource>,
    pub components: HashMap<TypeId, Vec<Component>>,
    pub startup_systems: SystemOrder<StartupSystem>,
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

    /// Add a resource to this. Only one copy of a type can exist as a resource.
    pub fn add_resource<T: Any>(mut self, resource: T) -> Self {
        self.resources
            .entry(TypeId::of::<T>())
            .or_insert(Rc::new(RwLock::new(Box::new(resource))));
        self
    }

    /// Add a component. Multiple copies of a type can exist.
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

    /// Add a system that will run once at the beginning.
    ///
    /// Uses the [`StartupSystem`] type.
    pub fn add_startup_systems<S: Into<SystemOrder<StartupSystem>>>(mut self, systems: S) -> Self {
        self.startup_systems = systems.into();
        self
    }

    /// Runs everytime there is a winit event.
    ///
    /// Uses the [`WinitEventSystem`] type.
    pub fn add_winit_event_systems<S: Into<SystemOrder<WinitEventSystem>>>(
        mut self,
        systems: S,
    ) -> Self {
        self.winit_event_systems = systems.into();
        self
    }

    /// Runs every frame.
    ///
    /// Uses the [`System`] type.
    pub fn add_systems<S: Into<SystemOrder<System>>>(mut self, systems: S) -> Self {
        self.systems = systems.into();
        self
    }

    /// An event handler. Only called when an event is raised in a system.
    ///
    /// Uses the [`EventSystem`] type.
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
