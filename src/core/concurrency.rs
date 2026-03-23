//! Concurrency Primitives

use std::sync::{Mutex, RwLock};

pub type SpinLock<T> = Mutex<T>;
pub type SharedMutex = RwLock<()>;
