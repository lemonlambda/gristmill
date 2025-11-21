use crate::ecs::ordering::SystemOrder;
use crate::ecs::{EventSystem, StartupSystem, System, WinitEventSystem};

use seq_macro::seq;

/// # Why does this exist?
/// OrderUp is a trait to make my (and your) life easier because I'm extremely lazy!
/// Why does this trait matter if `.after()` exists already??
///
/// # The Difference...
/// Let's see the difference between `OrderUp` and `.after()`
/// ```rs
/// let systems = (system1, system2, system3).order_up()
/// ```
/// vs.
/// ```rs
/// let systems = SystemOrder::new(system1).after(system2).after(system3);
/// ```
///
/// I don't know if you're blind or not but the second one is way longer and more annoying to type (seriously!).
pub trait OrderUp<S> {
    fn order_up(self) -> SystemOrder<S>;
}

/// This macro generates all of the instances of OrderUp, in two separate methods.
///
/// # Generate a range of tuples (Method 1)
/// Can generate a range of tuples, with a default of `10` or can be changed to anything.
/// You can generate range of tuples by doing `gen_order_up_impl!(TraitName)` or `gen_order_up_impl!(TraitName, 5)`.
///
/// # Generate a single tuple impl (Method 2)
/// You can generate a single tuple impl if you want.
/// To use call like so `gen_order_up_impl!(5, TraitName)`.
/// This will implement `TraitName` for `(T, T, T, T, T, T)`
macro_rules! gen_order_up_impl {
    ($f:ty) => {
        seq!(N in 0..=10 {
            gen_order_up_impl!(N, $f);
        });
    };
    ($f:ty, $n:literal) => {
        seq!(N in 0..=$n {
            gen_order_up_impl!(N, $f);
        });
    };
    (0, $f:ty) => {
        impl OrderUp<$f> for ($f,) {
            fn order_up(self) -> SystemOrder<$f> {
                SystemOrder {
                    order: vec![self.0],
                }
            }
        }
    };
    ($n:literal, $f:ty) => {
        seq!(T in 0..=$n {
            impl OrderUp<$f> for (#($f,)*) {
                fn order_up(self) -> SystemOrder<$f> {
                    SystemOrder {
                        order: vec![#(self.T,)*],
                    }
                }
            }
        });
    };
}

gen_order_up_impl! {WinitEventSystem}
gen_order_up_impl! {EventSystem}
gen_order_up_impl! {System}
gen_order_up_impl! {StartupSystem}
