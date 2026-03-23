//! Retry Utilities

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    threshold: u32,
    #[allow(dead_code)]
    recovery_timeout_ms: u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, recovery_timeout_secs: u64) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            threshold,
            recovery_timeout_ms: recovery_timeout_secs * 1000,
        }
    }
    pub fn state(&self) -> CircuitState {
        match self.state.load(Ordering::Acquire) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
    pub fn is_closed(&self) -> bool {
        self.state() == CircuitState::Closed
    }
    pub fn check(&self) -> Result<(), &'static str> {
        match self.state() {
            CircuitState::Closed | CircuitState::HalfOpen => Ok(()),
            CircuitState::Open => Err("Circuit breaker open"),
        }
    }
    pub fn success(&self) {
        match self.state() {
            CircuitState::HalfOpen => {
                let s = self.success_count.fetch_add(1, Ordering::AcqRel) + 1;
                if s >= 3 {
                    self.state
                        .store(CircuitState::Closed as u8, Ordering::Release);
                    self.failure_count.store(0, Ordering::Release);
                }
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Release);
            }
            _ => {}
        }
    }
    pub fn failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
        match self.state() {
            CircuitState::HalfOpen => {
                self.state
                    .store(CircuitState::Open as u8, Ordering::Release);
                self.success_count.store(0, Ordering::Release);
            }
            CircuitState::Closed => {
                if failures >= self.threshold {
                    self.state
                        .store(CircuitState::Open as u8, Ordering::Release);
                }
            }
            _ => {}
        }
    }
}

pub struct Retry {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl Retry {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
        }
    }
    pub fn execute<F, T, E>(&self, mut op: F) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        let mut attempt = 0;
        loop {
            match op() {
                Ok(r) => return Ok(r),
                Err(e) => {
                    attempt += 1;
                    if attempt >= self.max_attempts {
                        return Err(e);
                    }
                    let delay = self.base_delay * (2u32.pow(attempt - 1)).min(30) as u32;
                    thread::sleep(delay.min(self.max_delay));
                }
            }
        }
    }
}
