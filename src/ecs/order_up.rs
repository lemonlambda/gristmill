use crate::ecs::ordering::SystemOrder;
use crate::ecs::{EventSystem, Manager, WinitEvent, WinitEventSystem, World};

use anyhow::Result;
use paste::paste;
use seq_macro::seq;
use winit::event_loop::EventLoopWindowTarget;

pub trait OrderUp<S> {
    fn order_up(self) -> SystemOrder<S>;
}

macro_rules! gen_order_up_impl {
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
    }
}

gen_order_up_impl!(1, WinitEventSystem);
gen_order_up_impl!(2, WinitEventSystem);
gen_order_up_impl!(3, WinitEventSystem);
gen_order_up_impl!(4, WinitEventSystem);
gen_order_up_impl!(5, WinitEventSystem);
gen_order_up_impl!(6, WinitEventSystem);
gen_order_up_impl!(7, WinitEventSystem);
gen_order_up_impl!(8, WinitEventSystem);
