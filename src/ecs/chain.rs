use crate::ecs::ordering::SystemOrder;

use paste::paste;
use seq_macro::seq;

pub trait Chain<S> {
    fn chain(self) -> SystemOrder<S>;
}

impl<S> Chain<S> for (S, S) {
    fn chain(self) -> SystemOrder<S> {
        SystemOrder {
            order: vec![self.0, self.1],
        }
    }
}

impl<S> Chain<S> for (S, S, S) {
    fn chain(self) -> SystemOrder<S> {
        SystemOrder {
            order: vec![self.0, self.1, self.2],
        }
    }
}

seq!(T in 2..=5 {
    impl<S> Chain<S> for (#(S,)*) {
        fn chain(self) -> SystemOrder<S> {
            paste!{
            SystemOrder {
                order: vec![#(self.[<T-2>],)*],
            }
            }
        }
    }
});
