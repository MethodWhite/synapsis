use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait MutexSafe<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexSafe<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| {
            eprintln!("[Synapsis] WARNING: Mutex was poisoned, recovering state (data may be inconsistent)");
            e.into_inner()
        })
    }
}

pub trait RwLockSafe<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T>;
    fn write_safe(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockSafe<T> for RwLock<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| {
            eprintln!("[Synapsis] WARNING: RwLock was poisoned, recovering state (data may be inconsistent)");
            e.into_inner()
        })
    }

    fn write_safe(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| {
            eprintln!("[Synapsis] WARNING: RwLock (write) was poisoned, recovering state (data may be inconsistent)");
            e.into_inner()
        })
    }
}
