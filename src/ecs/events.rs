use std::any::Any;
use std::hash::{Hash, Hasher};
use std::mem::transmute;

pub type LemgineEvent = Box<dyn EventWrapper>;
pub type LemgineEventData = Box<dyn EventDataWrapper>;

pub trait EcsEvent {}
pub trait EcsEventData {}

pub trait EventDataWrapper: Any {
    fn clone_box(&self) -> Box<dyn EventDataWrapper>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl dyn EventDataWrapper {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

impl<T> EventDataWrapper for T
where
    T: EcsEventData + Clone + 'static + Any,
{
    fn clone_box(&self) -> Box<dyn EventDataWrapper> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Clone for Box<dyn EventDataWrapper> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait EventWrapper: EcsEvent {
    fn as_any(&self) -> &dyn Any;

    fn eq_dyn(&self, other: &dyn EventWrapper) -> bool;
    fn hash_dyn(&self, state: &mut dyn Hasher);
    fn clone_box(&self) -> Box<dyn EventWrapper>;
}

impl<T> EventWrapper for T
where
    T: EcsEvent + Clone + Eq + Hash + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq_dyn(&self, other: &dyn EventWrapper) -> bool {
        // Only equal if same concrete type AND Eq says so
        if let Some(other) = other.as_any().downcast_ref::<T>() {
            self == other
        } else {
            false
        }
    }

    fn hash_dyn(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }

    fn clone_box(&self) -> Box<dyn EventWrapper> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn EventWrapper> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for dyn EventWrapper {
    fn eq(&self, other: &Self) -> bool {
        EventWrapper::eq_dyn(self, other)
    }
}

impl Eq for dyn EventWrapper {}

impl Hash for dyn EventWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        EventWrapper::hash_dyn(self, state);
    }
}
