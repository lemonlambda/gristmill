/// A trait to abstract over a `Vec<T>` where `T` will be a `System` type.
///
/// # Example
/// ```rs
/// let system = SystemOrder::new(system);
/// ```
#[derive(Clone)]
pub struct SystemOrder<T> {
    pub order: Vec<T>,
}

impl<T> SystemOrder<T> {
    /// If you need an empty system use `SystemOrder::empty()` instead.
    pub fn new(system: T) -> Self {
        SystemOrder {
            order: vec![system],
        }
    }

    /// Extend out this `SystemOrder` with another `SystemOrder`.
    ///
    /// # Example
    /// ```rs
    /// let systems1 = SystemOrder::new(system1).after(system2);
    /// let systems2 = SystemOrder::new(system3).extend(systems1);
    /// ```
    pub fn extend(mut self, other: SystemOrder<T>) -> Self {
        self.order.extend(other.order);
        self
    }

    /// Extend out this `SystemOrder` with `SystemOrder` by using a `&mut self` instead.
    ///
    /// # Example
    /// ```rs
    /// let systems1 = SystemOrder::new(system1).after(system2);
    /// let mut systems2 = SystemOrder::new(system3);
    ///
    /// systems2.extend_mut_ref(systems1);
    /// ```
    pub fn extend_mut_ref(&mut self, other: SystemOrder<T>) {
        self.order.extend(other.order);
    }

    /// An empty `SystemOrder` where it has no systems.
    pub fn empty() -> Self {
        Self { order: vec![] }
    }
}

/// A trait to make ordering slightly easier in a lot of cases.
///
/// # Example
/// ```rs
/// let systems = SystemOrder::new(system1).after(system2).after(system3);
/// ```
pub trait Ordering<T> {
    type SystemType;

    /// Chain systems together like so
    /// ```rs
    /// let systems = SystemOrder::new(system1).after(system2).after(system3);
    /// ```
    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T>;
}

impl<T: Clone> Ordering<T> for SystemOrder<T> {
    type SystemType = T;

    fn after(&mut self, system: Self::SystemType) -> SystemOrder<T> {
        self.order.push(system);
        // Clone required because mut ref
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
