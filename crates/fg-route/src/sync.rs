//! Locks and threads whose failure is a bug: a poisoned lock or a panicked thread carries the
//! panic on to the thread that meets it, instead of an `unwrap` at every call site.

use std::any::Any;
use std::sync::{LockResult, Mutex, MutexGuard};

pub trait LockExt<T> {
    /// Locks the mutex. A mutex is only poisoned by a thread that panicked while holding it, so
    /// this panics in turn.
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().expect("a thread panicked while holding the lock")
    }
}

pub(crate) trait Unpoisoned<T> {
    /// The value of a lock result (`get_mut`, `into_inner`), panicking on a poisoned lock.
    fn unpoisoned(self) -> T;
}

impl<T> Unpoisoned<T> for LockResult<T> {
    fn unpoisoned(self) -> T {
        self.expect("a thread panicked while holding the lock")
    }
}

pub trait Joined<T> {
    /// The value of a joined thread; when it panicked, the panic goes on in this thread.
    fn joined(self) -> T;
}

impl<T> Joined<T> for Result<T, Box<dyn Any + Send>> {
    fn joined(self) -> T {
        self.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    }
}
