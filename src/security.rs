//! Security middleware and helpers for ClearURLs Bot

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

/// Simple per-user rate limiter (in-memory, not persistent)
pub struct RateLimiter {
    users: Mutex<HashMap<i64, Instant>>,
    min_interval: Duration,
}

impl RateLimiter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
            min_interval,
        }
    }

    /// Returns true if allowed, false if rate-limited
    pub fn check(&self, user_id: i64) -> bool {
        let mut users = self.users.lock().unwrap();
        let now = Instant::now();
        match users.get(&user_id) {
            Some(last) if now.duration_since(*last) < self.min_interval => false,
            _ => {
                users.insert(user_id, now);
                true
            }
        }
    }
}

/// Global rate limiter instance (1 request/sec per user)
pub static RATE_LIMITER: Lazy<RateLimiter> = Lazy::new(|| RateLimiter::new(Duration::from_secs(1)));

/// Sanitizes user input (basic, extend as needed)
pub fn sanitize_input(input: &str) -> String {
    // Remove control chars, trim, limit length
    let mut s = input.trim().replace(|c: char| c.is_control(), "");
    if s.len() > 4000 {
        s.truncate(4000);
    }
    s
}

/// Checks if a user is admin
pub fn is_admin(user_id: i64, admin_id: i64) -> bool {
    user_id == admin_id
}
