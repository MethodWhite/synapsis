//! Advanced Concurrency Primitives

use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

pub enum LockResult {
    Acquired,
    Timeout,
    Deadlock,
    Cancelled,
}
impl LockResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Acquired)
    }
}

pub struct TimedSpinLock {
    lock: Mutex<()>,
}

impl TimedSpinLock {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }
    pub fn try_lock(&self, _timeout: Duration) -> Option<LockGuard<'_>> {
        self.lock.try_lock().ok().map(|g| LockGuard(g))
    }
    pub fn lock(&self) -> LockGuard<'_> {
        LockGuard(self.lock.lock().unwrap())
    }
    pub fn unlock(&self) {}
}

impl Default for TimedSpinLock {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LockGuard<'a>(std::sync::MutexGuard<'a, ()>);
impl<'a> Drop for LockGuard<'a> {
    fn drop(&mut self) {}
}
impl<'a> std::ops::Deref for LockGuard<'a> {
    type Target = ();
    fn deref(&self) -> &() {
        &self.0
    }
}

pub struct FairMutex<T> {
    data: RwLock<T>,
}
impl<T> FairMutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: RwLock::new(data),
        }
    }
    pub fn read(&self) -> Option<FairReadGuard<'_, T>> {
        self.data.read().ok().map(|g| FairReadGuard(g))
    }
    pub fn write(&self) -> Option<FairWriteGuard<'_, T>> {
        self.data.write().ok().map(|g| FairWriteGuard(g))
    }
}

pub struct FairReadGuard<'a, T: 'a>(RwLockReadGuard<'a, T>);
impl<'a, T: 'a> std::ops::Deref for FairReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

pub struct FairWriteGuard<'a, T: 'a>(RwLockWriteGuard<'a, T>);
impl<'a, T: 'a> std::ops::Deref for FairWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<'a, T: 'a> std::ops::DerefMut for FairWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
