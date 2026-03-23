//! Rate Limiter for Synapsis MCP/TCP Server
//! Implements: SYNAPSIS-2026-006 mitigation
//! Algorithm: Token Bucket with per-session tracking

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    tokens_per_second: u32,
    max_tokens: u32,
}

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(tokens_per_second: u32, max_tokens: u32) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            tokens_per_second,
            max_tokens,
        }
    }
    
    pub fn check(&self, session_id: &str) -> Result<(), RateLimitError> {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        
        let bucket = buckets.entry(session_id.to_string()).or_insert_with(|| TokenBucket {
            tokens: self.max_tokens as f64,
            last_refill: now,
        });
        
        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.tokens_per_second as f64)
            .min(self.max_tokens as f64);
        bucket.last_refill = now;
        
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(RateLimitError::TooManyRequests)
        }
    }
    
    pub fn cleanup_old_buckets(&self, max_age: Duration) {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.last_refill) < max_age);
    }
}

#[derive(Debug)]
pub enum RateLimitError {
    TooManyRequests,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RateLimitError::TooManyRequests => write!(f, "Rate limit exceeded"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(10, 100);
        for _ in 0..50 {
            assert!(limiter.check("test_session").is_ok());
        }
    }
}
