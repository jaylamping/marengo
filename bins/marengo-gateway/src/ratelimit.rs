//! Per-session token-bucket rate limits for actuator commands.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Tuning slider flood cap (spec: 10 msgs/sec per joint).
pub const TUNING_REFILL_PER_SEC: f64 = 10.0;
pub const TUNING_BURST: f64 = 10.0;

/// Motion-class commands use a stricter bucket.
pub const MOTION_REFILL_PER_SEC: f64 = 2.0;
pub const MOTION_BURST: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBucket {
    Tuning,
    Motion,
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_per_sec,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        self.last_refill = now;
    }

    fn try_consume(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn bucket_key(session_id: &str, joint: &str, bucket: CommandBucket) -> String {
        let kind = match bucket {
            CommandBucket::Tuning => "tuning",
            CommandBucket::Motion => "motion",
        };
        format!("{session_id}:{joint}:{kind}")
    }

    fn bucket_config(bucket: CommandBucket) -> (f64, f64) {
        match bucket {
            CommandBucket::Tuning => (TUNING_BURST, TUNING_REFILL_PER_SEC),
            CommandBucket::Motion => (MOTION_BURST, MOTION_REFILL_PER_SEC),
        }
    }

    /// Returns true when the request is allowed; false when rate-limited.
    pub fn allow(&self, session_id: &str, joint: &str, bucket: CommandBucket) -> bool {
        if session_id.trim().is_empty() {
            return false;
        }
        let key = Self::bucket_key(session_id, joint, bucket);
        let now = Instant::now();
        let mut guard = match self.buckets.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let entry = guard.entry(key).or_insert_with(|| {
            let (capacity, refill) = Self::bucket_config(bucket);
            TokenBucket::new(capacity, refill)
        });
        entry.try_consume(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn tuning_bucket_allows_burst_then_rejects() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.allow(
                "sess-a",
                "shoulder_pitch",
                CommandBucket::Tuning
            ));
        }
        assert!(!limiter.allow(
            "sess-a",
            "shoulder_pitch",
            CommandBucket::Tuning
        ));
    }

    #[test]
    fn motion_bucket_is_stricter_than_tuning() {
        let limiter = RateLimiter::new();
        assert!(limiter.allow("sess-a", "elbow", CommandBucket::Motion));
        assert!(limiter.allow("sess-a", "elbow", CommandBucket::Motion));
        assert!(!limiter.allow("sess-a", "elbow", CommandBucket::Motion));
    }

    #[test]
    fn buckets_are_isolated_by_session_and_joint() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.allow(
                "sess-a",
                "shoulder_pitch",
                CommandBucket::Tuning
            ));
        }
        assert!(!limiter.allow(
            "sess-a",
            "shoulder_pitch",
            CommandBucket::Tuning
        ));
        assert!(limiter.allow(
            "sess-b",
            "shoulder_pitch",
            CommandBucket::Tuning
        ));
        assert!(limiter.allow("sess-a", "elbow", CommandBucket::Tuning));
    }

    #[test]
    fn tuning_bucket_refills_over_time() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            let _ = limiter.allow("sess-a", "shoulder_roll", CommandBucket::Tuning);
        }
        assert!(!limiter.allow(
            "sess-a",
            "shoulder_roll",
            CommandBucket::Tuning
        ));
        thread::sleep(Duration::from_millis(120));
        assert!(limiter.allow(
            "sess-a",
            "shoulder_roll",
            CommandBucket::Tuning
        ));
    }

    #[test]
    fn rejects_empty_session_id() {
        let limiter = RateLimiter::new();
        assert!(!limiter.allow("", "elbow", CommandBucket::Tuning));
        assert!(!limiter.allow("   ", "elbow", CommandBucket::Tuning));
    }
}
