pub use std::fmt::Debug;
pub use std::hash::Hash;
pub use std::sync::Arc;
pub use std::sync::Mutex;
pub use std::sync::OnceLock;
pub use derive_more::Deref;
pub use derive_more::DerefMut;
pub use anyhow::Error;
pub use anyhow::anyhow; 
pub use anyhow::Context;
pub use std::mem::swap;

// ==============
// === Errors ===
// ==============

pub type Result<T=(), E=Error> = anyhow::Result<T, E>;

// ===============
// === Default ===
// ===============

pub fn default<T: Default>() -> T {
    T::default()
}

// =============
// === Tuple ===
// =============

pub trait Map0 {
    type Item;
    type Output<T>;
    fn map0<T>(self, f: impl FnOnce(Self::Item) -> T) -> Self::Output<T>;
}

pub trait Map1 {
    type Item;
    type Output<T>;
    fn map1<T>(self, f: impl FnOnce(Self::Item) -> T) -> Self::Output<T>;
}

impl<T0, T1> Map0 for (T0, T1) {
    type Item = T0;
    type Output<T> = (T, T1);
    fn map0<U>(self, f: impl FnOnce(T0) -> U) -> Self::Output<U> {
        (f(self.0), self.1)
    }
}

impl<T0, T1> Map1 for (T0, T1) {
    type Item = T1;
    type Output<T> = (T0, T);
    fn map1<U>(self, f: impl FnOnce(T1) -> U) -> Self::Output<U> {
        (self.0, f(self.1))
    }
}
