//! Hell where Entities and Components and Systems live

use anyhow::{Result, anyhow};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    mem::transmute,
    sync::{
        Arc, MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard,
        RwLockWriteGuard,
    },
};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};

type WinitEvent = winit::event::Event<()>;

use crate::{
    ecs::{
        events::{
            EcsEvent, EcsEventData, EventDataWrapper, EventWrapper, LemgineEvent, LemgineEventData,
        },
        ordering::SystemOrder,
    },
    engine::Engine,
};

pub mod events;
pub mod ordering;

pub type System = fn(&World) -> Result<()>;
pub type EventSystem = fn(&World, LemgineEventData) -> Result<()>;
pub type WinitEventSystem = fn(&World, WinitEvent, &EventLoopWindowTarget<()>) -> Result<()>;

/// Should manage everything related to the ECS
pub struct Manager {
    pub world: World,
    pub startup_systems: SystemOrder<System>,
    pub systems: SystemOrder<System>,
    pub winit_event_systems: SystemOrder<WinitEventSystem>,
    pub event_systems: HashMap<LemgineEvent, SystemOrder<EventSystem>>,
}

impl Manager {
    pub fn new() -> Result<Self> {
        let world = World::new();

        Ok(Self {
            world,
            startup_systems: SystemOrder::empty(),
            systems: SystemOrder::empty(),
            winit_event_systems: SystemOrder::empty(),
            event_systems: HashMap::new(),
        })
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

    pub fn raise_event(&self, event: LemgineEvent, data: LemgineEventData) -> Result<()> {
        if let Some(systems) = self.event_systems.get(&event) {
            for system in systems.clone().order {
                match system(&self.world, data.clone()) {
                    Ok(_) => {}
                    Err(err) => return Err(anyhow!(err)),
                };
            }
        }

        Ok(())
    }

    pub fn check_events(&mut self) -> Result<()> {
        let events = self.world.new_events.read().unwrap();

        // Check if any events have been raised
        if events.is_empty() {
            return Ok(());
        }

        drop(events);

        let mut events = self.world.new_events.write().unwrap();

        for (event, data) in events.clone().into_iter() {
            self.raise_event(event, data)?;
        }

        events.clear();

        Ok(())
    }

    pub fn run(mut self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        self.world.add_resource(Engine::new(&event_loop));

        for system in self.startup_systems.order.iter() {
            system(&self.world)?;
        }

        event_loop.run(move |event, elwt| {
            for system in self.winit_event_systems.clone().order.iter() {
                system(&self.world, event.clone(), elwt).unwrap();
            }

            self.check_events().unwrap();

            for system in self.systems.clone().order.iter() {
                system(&self.world).unwrap();
            }

            self.check_events().unwrap();
        })?;

        Ok(())
    }
}

pub type Resource = Arc<RwLock<Box<dyn Any>>>;
pub type Component = Arc<RwLock<Box<dyn Any>>>;

/// A whole new world!
#[derive(Clone)]
pub struct World {
    resources: HashMap<TypeId, Resource>,
    components: HashMap<TypeId, Vec<Component>>,
    new_events: Arc<RwLock<Vec<(LemgineEvent, LemgineEventData)>>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            components: HashMap::new(),
            new_events: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn add_resource<T: Any>(&mut self, resource: T) {
        self.resources
            .entry(TypeId::of::<T>())
            .or_insert(Arc::new(RwLock::new(Box::new(resource))));
    }

    pub fn get_resource<T: Any>(&self) -> MappedRwLockReadGuard<'_, Box<T>> {
        let reading = self
            .resources
            .get(&TypeId::of::<T>())
            .unwrap()
            .read()
            .unwrap();
        RwLockReadGuard::map(reading, |r| unsafe { transmute(r) })
    }

    pub fn get_resource_mut<T: Any>(&self) -> MappedRwLockWriteGuard<'_, Box<T>> {
        let reading = self
            .resources
            .get(&TypeId::of::<T>())
            .unwrap()
            .write()
            .unwrap();
        RwLockWriteGuard::map(reading, |r| unsafe { transmute(r) })
    }

    pub fn add_component<T: Any>(&mut self, component: T) {
        if let Some(value) = self.components.get_mut(&TypeId::of::<T>()) {
            value.push(Arc::new(RwLock::new(Box::new(component))));
        } else {
            self.components.insert(
                TypeId::of::<T>(),
                vec![Arc::new(RwLock::new(Box::new(component)))],
            );
        }
    }

    pub fn get_components<T: Any>(&self) -> Vec<MappedRwLockReadGuard<'_, Box<T>>> {
        let reading = self.components.get(&TypeId::of::<T>()).unwrap();
        reading
            .iter()
            .map(|v| RwLockReadGuard::map(v.read().unwrap(), |r| unsafe { transmute(r) }))
            .collect()
    }

    pub fn get_components_mut<T: Any>(&self) -> Vec<MappedRwLockWriteGuard<'_, Box<T>>> {
        let reading = self.components.get(&TypeId::of::<T>()).unwrap();
        reading
            .iter()
            .map(|v| RwLockWriteGuard::map(v.write().unwrap(), |r| unsafe { transmute(r) }))
            .collect()
    }

    pub fn raise_event<E: EventWrapper + 'static, D: EventDataWrapper + 'static>(
        &self,
        event: E,
        data: D,
    ) {
        self.new_events
            .write()
            .unwrap()
            .push((Box::new(event), Box::new(data)));
    }
}
