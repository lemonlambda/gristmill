use std::any::Any;
use std::hash::{Hash, Hasher};

pub type LemgineEvent = Box<dyn EventWrapper>;
pub type LemgineEventData = Box<dyn EventDataWrapper>;

/// Type to denote what's an ECS Event
pub trait EcsEvent {}

/// Type to denote what's an ECS Event's Data
pub trait EcsEventData {}

/// This type is to wrap the [`EcsEventData`] to provide it with certain capabilities.
/// These capabilities aren't possible without this wrapper type.
///
/// These capabilities are required by [`Manager`].
pub trait EventDataWrapper: Any {
    /// Wrapper for `clone` on a [`Box<EcsEventData>`]
    fn clone_box(&self) -> Box<dyn EventDataWrapper>;

    /// Convert to `&dyn Any`
    fn as_any(&self) -> &dyn Any;

    /// Convert to `&mut dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl dyn EventDataWrapper {
    /// Downcast to ref a the specified type.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    /// Downcast to a mut ref of the specified type.
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

/// Wrapper over [`EcsEvent`] to get additional functionality.
/// This functionality isn't possible without this wrapper.
///
/// These capabilities are required by [`Manager`].
pub trait EventWrapper: EcsEvent {
    /// Convert to `&dyn Any`.
    fn as_any(&self) -> &dyn Any;

    /// Wrapper for `eq` function.
    fn eq_dyn(&self, other: &dyn EventWrapper) -> bool;

    /// Wrapper for hashing the type.
    fn hash_dyn(&self, state: &mut dyn Hasher);

    /// Wrapper for cloning
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
