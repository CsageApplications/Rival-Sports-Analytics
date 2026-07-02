//! Rate limiter for API requests

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Rate limiter that tracks API usage from response headers
#[derive(Debug)]
pub struct RateLimiter {
    /// Requests remaining in current period
    remaining: AtomicU32,
    /// Total requests used
    used: AtomicU32,
    /// Minimum delay between requests
    min_delay: Duration,
    /// Last request time
    last_request: Mutex<Option<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(min_delay_ms: u64) -> Self {
        Self {
            remaining: AtomicU32::new(u32::MAX), // Unknown until first request
            used: AtomicU32::new(0),
            min_delay: Duration::from_millis(min_delay_ms),
            last_request: Mutex::new(None),
        }
    }

    /// Update rate limit info from API response headers
    pub fn update_from_headers(&self, headers: &reqwest::header::HeaderMap) {
        if let Some(remaining) = headers.get("x-requests-remaining") {
            if let Some(value) = remaining.to_str().ok().and_then(|s| s.parse().ok()) {
                self.remaining.store(value, Ordering::SeqCst);
                debug!(remaining = value, "Updated rate limit from headers");
            }
        }

        if let Some(used) = headers.get("x-requests-used") {
            if let Some(value) = used.to_str().ok().and_then(|s| s.parse().ok()) {
                self.used.store(value, Ordering::SeqCst);
            }
        }
    }

    /// Get requests remaining
    pub fn remaining(&self) -> u32 {
        self.remaining.load(Ordering::SeqCst)
    }

    /// Get requests used
    pub fn used(&self) -> u32 {
        self.used.load(Ordering::SeqCst)
    }

    /// Wait if necessary before making a request
    pub async fn acquire(&self) {
        // Check remaining requests
        let remaining = self.remaining.load(Ordering::SeqCst);
        if remaining < 10 && remaining != u32::MAX {
            warn!(remaining, "Low on API requests!");
        }

        // Enforce minimum delay
        let mut last = self.last_request.lock().unwrap();
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_delay {
                let wait = self.min_delay - elapsed;
                drop(last); // Release lock while sleeping
                tokio::time::sleep(wait).await;
                let mut last = self.last_request.lock().unwrap();
                *last = Some(Instant::now());
            } else {
                *last = Some(Instant::now());
            }
        } else {
            *last = Some(Instant::now());
        }
    }

    /// Check if we should avoid making requests
    pub fn should_throttle(&self) -> bool {
        let remaining = self.remaining.load(Ordering::SeqCst);
        remaining < 5 && remaining != u32::MAX
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        // Default: 200ms between requests
        Self::new(200)
    }
}
