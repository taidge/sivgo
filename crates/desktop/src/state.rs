//! Owner-scoped editor state with borrowed access to the non-Clone engine.
//!
//! Glory owns the storage and dependency tracking. The owned lock guards let
//! commands borrow the engine without cloning its document on every read.
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use glory::{Cage, Scope};
use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};

pub struct Signal<T: 'static>(Cage<Arc<RwLock<T>>>);

impl<T> Copy for Signal<T> {}
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Signal<T> {
    pub fn new(scope: &Scope, value: T) -> Self {
        Self(scope.cage(Arc::new(RwLock::new(value))))
    }

    pub fn read(self) -> ArcRwLockReadGuard<RawRwLock, T> {
        self.0
            .get()
            .try_read_arc()
            .expect("editor state is already being edited")
    }

    pub fn peek(self) -> ArcRwLockReadGuard<RawRwLock, T> {
        self.0
            .get_untracked()
            .try_read_arc()
            .expect("editor state is already being edited")
    }

    pub fn write(self) -> Write<T> {
        let guard = self
            .0
            .get_untracked()
            .try_write_arc()
            .expect("editor state is already borrowed");
        Write {
            guard: Some(guard),
            source: self.0,
        }
    }

    pub fn set(self, value: T) {
        *self.write() = value;
    }

    pub fn map<U: 'static>(self, mapper: impl Fn(&T) -> U + 'static) -> glory::Bond<U> {
        self.0.map(move |value| mapper(&value.read()))
    }
}

pub struct Write<T: 'static> {
    guard: Option<ArcRwLockWriteGuard<RawRwLock, T>>,
    source: Cage<Arc<RwLock<T>>>,
}

impl<T> Deref for Write<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_deref().expect("live editor guard")
    }
}
impl<T> DerefMut for Write<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_deref_mut().expect("live editor guard")
    }
}
impl<T> Drop for Write<T> {
    fn drop(&mut self) {
        // Release the lock before notifying any dependent computations.
        drop(self.guard.take());
        let _ = self.source.try_revise(|_| {});
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_engine_writes_notify_after_unlock_and_release_with_owner() {
        use std::{cell::Cell, rc::Rc};
        let app = glory::App::memory();
        let state = Signal::new(app.scope(), vec![1]);
        let observed = Rc::new(Cell::new(0));
        let result = observed.clone();
        app.scope().effect(move || result.set(state.read().len()));
        app.pump().unwrap();
        assert_eq!(observed.get(), 1);
        state.write().push(2);
        app.pump().unwrap();
        assert_eq!(observed.get(), 2);
        let source = state.0;
        drop(app);
        assert!(!source.is_alive());
    }
}
