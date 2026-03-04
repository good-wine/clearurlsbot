//! Security middleware and helpers for ClearURLs Bot

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
        let mut users = match self.users.lock() {
            Ok(u) => u,
            Err(e) => {
                log::error!("Errore nel lock users: {}", e);
                return true;
            }
        };
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

mod input_sanitizer {
    /// Sanitizza l'input utente (base, estendibile)
    use crate::sanitizer::validation::is_valid_url;

    pub fn sanitize(input: &str) -> String {
        let mut s = input.trim().replace(|c: char| c.is_control(), "");
        if s.len() > 4000 {
            s.truncate(4000);
        }
        if !is_valid_url(&s) {
            log::error!("Input non valido: {}", s);
            return String::new();
        }
        s
    }

    /// Sanitizza callback data senza validare come URL
    pub fn sanitize_callback(input: &str) -> String {
        let mut s = input.trim().replace(|c: char| c.is_control(), "");
        if s.len() > 4000 {
            s.truncate(4000);
        }
        s
    }
}

pub use input_sanitizer::{sanitize as sanitize_input, sanitize_callback};

/// Checks if a user is admin
pub fn is_admin(user_id: i64, admin_id: i64) -> bool {
    user_id == admin_id
}
