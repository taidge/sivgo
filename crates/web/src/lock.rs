//! Lock helpers that survive a poisoned mutex.
//!
//! Every store in this crate keeps its data behind an `RwLock`. Calling
//! `.unwrap()` on the guard turns one panic anywhere in the process into a
//! permanent outage: the lock stays poisoned, so every later request that
//! touches that store panics too and the endpoint returns 500 forever.
//!
//! None of these stores maintain an invariant across a panic that would make
//! the data unsafe to read afterwards — the worst case is a partially applied
//! update to one entry. Recovering the guard and carrying on is strictly
//! better than refusing to serve.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Read access that ignores poisoning.
pub fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Write access that ignores poisoning.
pub fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_poisoned_lock_is_still_usable() {
        let lock = Arc::new(RwLock::new(vec![1, 2, 3]));

        // Poison it the way a panicking handler would.
        let poisoner = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.write().unwrap();
            panic!("simulated handler panic");
        })
        .join();

        assert!(lock.read().is_err(), "the lock should now be poisoned");

        // The helpers keep working, so the endpoint keeps serving.
        assert_eq!(read(&lock).len(), 3);
        write(&lock).push(4);
        assert_eq!(read(&lock).len(), 4);
    }
}
