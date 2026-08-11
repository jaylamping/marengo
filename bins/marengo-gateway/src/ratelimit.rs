//! Per-client token-bucket rate limits for actuator commands.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tuning slider flood cap (spec: 10 msgs/sec per joint).
pub const TUNING_REFILL_PER_SEC: f64 = 10.0;
pub const TUNING_BURST: f64 = 10.0;

/// Motion-class commands use a stricter bucket (reserved for PR-5).
pub const MOTION_REFILL_PER_SEC: f64 = 2.0;
pub const MOTION_BURST: f64 = 2.0;

/// Active-reporting lease heartbeats (StrictMode + renewals).
pub const DIAGNOSTICS_REFILL_PER_SEC: f64 = 20.0;
pub const DIAGNOSTICS_BURST: f64 = 20.0;

/// Hardware-page motor status solicit (~1 poll / 2 s globally).
pub const STATUS_POLL_REFILL_PER_SEC: f64 = 0.5;
pub const STATUS_POLL_BURST: f64 = 1.0;

/// Drop idle rate-limit keys after this idle period.
const BUCKET_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBucket {
    Tuning,
    /// Motion-class / calibration commands (set-zero, reserved for PR-5 motion).
    Motion,
    /// Type-24 lease acquire/renew (RELEASE bypasses the limiter).
    Diagnostics,
    /// Light Disable (type-4) status solicit for Hardware-page Online facets.
    StatusPoll,
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_per_sec: f64,
    last_refill: Instant,
    last_touch: Instant,
}

impl TokenBucket {
    fn new(capacity: f64, refill_per_sec: f64, now: Instant) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_per_sec,
            last_refill: now,
            last_touch: now,
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
        self.last_touch = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refund_one(&mut self, now: Instant) {
        self.refill(now);
        self.tokens = (self.tokens + 1.0).min(self.capacity);
        self.last_touch = now;
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

    fn bucket_key(client_id: &str, joint: &str, bucket: CommandBucket) -> String {
        let kind = match bucket {
            CommandBucket::Tuning => "tuning",
            CommandBucket::Motion => "motion",
            CommandBucket::Diagnostics => "diagnostics",
            CommandBucket::StatusPoll => "status_poll",
        };
        match bucket {
            // Tuning stays per UI session so independent tabs don't starve each other.
            CommandBucket::Tuning => format!("{client_id}:{joint}:{kind}"),
            // Motion/calibration must not be keyed by attacker-chosen client_id —
            // rotating the field would otherwise bypass the flood cap on set-zero.
            CommandBucket::Motion | CommandBucket::Diagnostics => {
                format!("__global__:{joint}:{kind}")
            }
            // One global solicit for all motors — ignore joint and client_id rotation.
            CommandBucket::StatusPoll => format!("__global__:{kind}"),
        }
    }

    fn bucket_config(bucket: CommandBucket) -> (f64, f64) {
        match bucket {
            CommandBucket::Tuning => (TUNING_BURST, TUNING_REFILL_PER_SEC),
            CommandBucket::Motion => (MOTION_BURST, MOTION_REFILL_PER_SEC),
            CommandBucket::Diagnostics => (DIAGNOSTICS_BURST, DIAGNOSTICS_REFILL_PER_SEC),
            CommandBucket::StatusPoll => (STATUS_POLL_BURST, STATUS_POLL_REFILL_PER_SEC),
        }
    }

    fn lock_buckets(&self) -> std::sync::MutexGuard<'_, HashMap<String, TokenBucket>> {
        match self.buckets.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn evict_stale(guard: &mut HashMap<String, TokenBucket>, now: Instant) {
        guard.retain(|_, bucket| now.duration_since(bucket.last_touch) < BUCKET_TTL);
    }

    /// Returns true when the request is allowed; false when rate-limited.
    pub fn allow(&self, client_id: &str, joint: &str, bucket: CommandBucket) -> bool {
        if client_id.trim().is_empty() {
            return false;
        }
        let key = Self::bucket_key(client_id, joint, bucket);
        let now = Instant::now();
        let mut guard = self.lock_buckets();
        Self::evict_stale(&mut guard, now);
        let entry = guard.entry(key).or_insert_with(|| {
            let (capacity, refill) = Self::bucket_config(bucket);
            TokenBucket::new(capacity, refill, now)
        });
        entry.try_consume(now)
    }

    /// Refund one token after a failed publish so consume+publish stays atomic.
    pub fn refund(&self, client_id: &str, joint: &str, bucket: CommandBucket) {
        if client_id.trim().is_empty() {
            return;
        }
        let key = Self::bucket_key(client_id, joint, bucket);
        let now = Instant::now();
        let mut guard = self.lock_buckets();
        if let Some(entry) = guard.get_mut(&key) {
            entry.refund_one(now);
        }
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
            assert!(limiter.allow("client-a", "right_shoulder_pitch", CommandBucket::Tuning));
        }
        assert!(!limiter.allow("client-a", "right_shoulder_pitch", CommandBucket::Tuning));
    }

    #[test]
    fn motion_bucket_is_stricter_than_tuning() {
        let limiter = RateLimiter::new();
        assert!(limiter.allow("client-a", "elbow", CommandBucket::Motion));
        assert!(limiter.allow("client-a", "elbow", CommandBucket::Motion));
        assert!(!limiter.allow("client-a", "elbow", CommandBucket::Motion));
    }

    #[test]
    fn motion_bucket_ignores_client_id_rotation() {
        let limiter = RateLimiter::new();
        assert!(limiter.allow("client-a", "elbow", CommandBucket::Motion));
        assert!(limiter.allow("client-b", "elbow", CommandBucket::Motion));
        // Third attempt with a fresh client_id must still hit the shared Motion cap.
        assert!(!limiter.allow("client-c", "elbow", CommandBucket::Motion));
    }

    #[test]
    fn buckets_are_isolated_by_client_and_joint() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.allow("client-a", "right_shoulder_pitch", CommandBucket::Tuning));
        }
        assert!(!limiter.allow("client-a", "right_shoulder_pitch", CommandBucket::Tuning));
        assert!(limiter.allow("client-b", "right_shoulder_pitch", CommandBucket::Tuning));
        assert!(limiter.allow("client-a", "right_shoulder_roll", CommandBucket::Tuning));
    }

    #[test]
    fn tuning_bucket_refills_over_time() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            let _ = limiter.allow("client-a", "right_shoulder_roll", CommandBucket::Tuning);
        }
        assert!(!limiter.allow("client-a", "right_shoulder_roll", CommandBucket::Tuning));
        thread::sleep(Duration::from_millis(120));
        assert!(limiter.allow("client-a", "right_shoulder_roll", CommandBucket::Tuning));
    }

    #[test]
    fn rejects_empty_client_id() {
        let limiter = RateLimiter::new();
        assert!(!limiter.allow("", "elbow", CommandBucket::Tuning));
        assert!(!limiter.allow("   ", "elbow", CommandBucket::Tuning));
    }

    #[test]
    fn refund_restores_token_after_failed_publish() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.allow("client-a", "joint", CommandBucket::Tuning));
        }
        assert!(!limiter.allow("client-a", "joint", CommandBucket::Tuning));
        limiter.refund("client-a", "joint", CommandBucket::Tuning);
        assert!(limiter.allow("client-a", "joint", CommandBucket::Tuning));
    }

    #[test]
    fn status_poll_bucket_allows_one_then_rejects_until_refill() {
        let limiter = RateLimiter::new();
        assert!(limiter.allow("client-a", "_", CommandBucket::StatusPoll));
        assert!(!limiter.allow("client-b", "_", CommandBucket::StatusPoll));
    }
}
