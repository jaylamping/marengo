//! Bounded runtime publication classes; command topics never enter this outbox.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, TryLockError};
use std::time::{Duration, Instant};

use crate::topics::{EVENT_TELEMETRY_TOPICS, LATEST_TELEMETRY_TOPICS};

pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub const EVENT_CAPACITY: usize = 128;
pub const EVENT_BYTE_CAPACITY: usize = 512 * 1024;
const LATEST_TOPICS: [&str; 6] = LATEST_TELEMETRY_TOPICS;
pub const QUEUE_ITEM_CAPACITY: usize = LATEST_TOPICS.len() + EVENT_CAPACITY;
pub const QUEUE_BYTE_CAPACITY: usize =
    LATEST_TOPICS.len() * MAX_PAYLOAD_BYTES + EVENT_BYTE_CAPACITY;
const STATE_MAX_AGE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardOutcome {
    Accepted,
    Coalesced,
    Dropped,
}

/// Queue counters describe transport admission, not delivery or physical safety.
///
/// `dropped` counts transport-pressure loss only (lock contention, event
/// overflow, expiry past the age bound). Admissions never queued (unknown
/// topic, oversize payload, closed outbox) count as `rejected`, never as
/// dropped, so Consul command traffic cannot inflate the loss counter.
#[derive(Debug, Clone, Default)]
pub struct IpcQueueStats {
    pub connected: bool,
    pub queued_items: usize,
    pub queued_bytes: usize,
    pub oldest_age_ms: u64,
    pub accepted: u64,
    pub coalesced: u64,
    pub dropped: u64,
    /// State discarded unread past the age bound (a subset of `dropped`).
    pub expired: u64,
    /// Admissions never queued (unknown topic, oversize payload, closed
    /// outbox). Excluded from `dropped`.
    pub rejected: u64,
    pub admitted_disconnected: u64,
    pub write_failures: u64,
}

pub(crate) struct Publication {
    pub topic: &'static str,
    pub payload: Vec<u8>,
    admitted: Instant,
}

struct Queue {
    latest: [Option<Publication>; LATEST_TOPICS.len()],
    events: VecDeque<Publication>,
    event_bytes: usize,
    next_class: usize,
}

pub(crate) struct Outbox {
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    queue: Mutex<Queue>,
    ready: Condvar,
    connected: AtomicBool,
    closed: AtomicBool,
    accepted: AtomicU64,
    coalesced: AtomicU64,
    dropped: AtomicU64,
    expired: AtomicU64,
    rejected: AtomicU64,
    disconnected: AtomicU64,
    write_failures: AtomicU64,
    connection_changes: tokio::sync::broadcast::Sender<bool>,
}

impl Outbox {
    pub fn new() -> Self {
        Self::new_with_clock(Arc::new(Instant::now))
    }

    pub fn new_with_clock(clock: Arc<dyn Fn() -> Instant + Send + Sync>) -> Self {
        Self {
            clock,
            queue: Mutex::new(Queue {
                latest: std::array::from_fn(|_| None),
                events: VecDeque::new(),
                event_bytes: 0,
                next_class: 0,
            }),
            ready: Condvar::new(),
            connected: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            accepted: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            expired: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            disconnected: AtomicU64::new(0),
            write_failures: AtomicU64::new(0),
            connection_changes: tokio::sync::broadcast::channel(16).0,
        }
    }

    pub fn set_connected(&self, connected: bool) {
        let _queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        let connected = connected && !self.closed();
        let previous = self.connected.swap(connected, Ordering::Relaxed);
        if previous != connected {
            let _ = self.connection_changes.send(connected);
        }
        self.ready.notify_all();
    }

    pub fn close(&self) {
        let _queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        self.closed.store(true, Ordering::Relaxed);
        if self.connected.swap(false, Ordering::Relaxed) {
            let _ = self.connection_changes.send(false);
        }
        self.ready.notify_all();
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn subscribe_connection(&self) -> tokio::sync::broadcast::Receiver<bool> {
        self.connection_changes.subscribe()
    }

    pub fn record_write_failure(&self) {
        self.write_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn admit(&self, topic: &str, payload: &[u8]) -> ForwardOutcome {
        self.admit_before_lock(topic, payload, || {})
    }

    fn admit_before_lock(
        &self,
        topic: &str,
        payload: &[u8],
        before_lock: impl FnOnce(),
    ) -> ForwardOutcome {
        let latest_index = LATEST_TOPICS
            .iter()
            .position(|candidate| *candidate == topic);
        let event_index = EVENT_TELEMETRY_TOPICS
            .iter()
            .position(|candidate| *candidate == topic);
        if self.closed()
            || payload.len() > MAX_PAYLOAD_BYTES
            || (latest_index.is_none() && event_index.is_none())
        {
            return self.reject_publication();
        }
        before_lock();
        let mut queue = match self.queue.try_lock() {
            Ok(queue) => queue,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return self.drop_publication(),
        };
        if self.closed() {
            return self.reject_publication();
        }
        let outcome = if let Some(index) = latest_index {
            let replaced = queue.latest[index].is_some();
            queue.latest[index] = Some(Publication {
                topic: LATEST_TOPICS[index],
                payload: payload.to_vec(),
                admitted: (self.clock)(),
            });
            if replaced {
                ForwardOutcome::Coalesced
            } else {
                ForwardOutcome::Accepted
            }
        } else if let Some(index) = event_index {
            if queue.events.len() >= EVENT_CAPACITY
                || queue.event_bytes + payload.len() > EVENT_BYTE_CAPACITY
            {
                return self.drop_publication();
            }
            queue.events.push_back(Publication {
                topic: EVENT_TELEMETRY_TOPICS[index],
                payload: payload.to_vec(),
                admitted: (self.clock)(),
            });
            queue.event_bytes += payload.len();
            ForwardOutcome::Accepted
        } else {
            return self.drop_publication();
        };
        match outcome {
            ForwardOutcome::Accepted => &self.accepted,
            ForwardOutcome::Coalesced => &self.coalesced,
            ForwardOutcome::Dropped => &self.dropped,
        }
        .fetch_add(1, Ordering::Relaxed);
        if !self.connected.load(Ordering::Relaxed) {
            self.disconnected.fetch_add(1, Ordering::Relaxed);
        }
        drop(queue);
        self.ready.notify_one();
        outcome
    }

    fn drop_publication(&self) -> ForwardOutcome {
        self.dropped.fetch_add(1, Ordering::Relaxed);
        ForwardOutcome::Dropped
    }

    /// Admissions never queued (unknown topic, oversize payload, closed
    /// outbox): counted separately so transport-pressure loss stays honest.
    fn reject_publication(&self) -> ForwardOutcome {
        self.rejected.fetch_add(1, Ordering::Relaxed);
        ForwardOutcome::Dropped
    }

    pub fn next(&self) -> Option<Publication> {
        let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if self.closed() || !self.connected.load(Ordering::Relaxed) {
                return None;
            }
            for _ in 0..=LATEST_TOPICS.len() {
                let class = queue.next_class;
                queue.next_class = (class + 1) % (LATEST_TOPICS.len() + 1);
                if class == LATEST_TOPICS.len() {
                    if let Some(event) = queue.events.pop_front() {
                        queue.event_bytes -= event.payload.len();
                        return Some(event);
                    }
                } else if let Some(state) = queue.latest[class].take() {
                    if (self.clock)().saturating_duration_since(state.admitted) <= STATE_MAX_AGE {
                        return Some(state);
                    }
                    // Expired state is transport-pressure loss (a subset of
                    // dropped) with its own counter so the gateway can tell
                    // "expired" from "never sent".
                    self.expired.fetch_add(1, Ordering::Relaxed);
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub fn stats(&self) -> IpcQueueStats {
        let queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        let publications = queue.latest.iter().flatten().chain(queue.events.iter());
        let (items, bytes, age) = publications.fold((0, 0, 0), |(items, bytes, age), p| {
            (
                items + 1,
                bytes + p.payload.len(),
                age.max(
                    (self.clock)()
                        .saturating_duration_since(p.admitted)
                        .as_millis() as u64,
                ),
            )
        });
        IpcQueueStats {
            connected: self.connected.load(Ordering::Relaxed),
            queued_items: items,
            queued_bytes: bytes,
            oldest_age_ms: age,
            accepted: self.accepted.load(Ordering::Relaxed),
            coalesced: self.coalesced.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            expired: self.expired.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            admitted_disconnected: self.disconnected.load(Ordering::Relaxed),
            write_failures: self.write_failures.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod shutdown_admission_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::sync::{mpsc, Arc};

    #[test]
    fn shutdown_winning_before_queue_lock_refuses_publication() {
        let outbox = Arc::new(Outbox::new());
        let publisher = Arc::clone(&outbox);
        let (paused_tx, paused_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            publisher.admit_before_lock("robot/state", &[1], || {
                paused_tx.send(()).expect("pause");
                resume_rx.recv().expect("resume");
            })
        });
        paused_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("publisher barrier");
        outbox.close();
        resume_tx.send(()).expect("release publisher");
        assert_eq!(
            worker.join().expect("join publisher"),
            ForwardOutcome::Dropped
        );
        assert_eq!(outbox.stats().queued_items, 0);
        assert_eq!(outbox.stats().accepted, 0);
        // Closed admission never queued: rejected, not dropped.
        assert_eq!(outbox.stats().rejected, 1);
        assert_eq!(outbox.stats().dropped, 0);
    }

    #[test]
    fn unknown_topic_and_oversize_are_rejected_not_dropped() {
        let outbox = Outbox::new();
        assert_eq!(
            outbox.admit("robot/testing/mit_command_batch", &[1]),
            ForwardOutcome::Dropped
        );
        assert_eq!(
            outbox.admit("robot/state", &vec![0u8; MAX_PAYLOAD_BYTES + 1]),
            ForwardOutcome::Dropped
        );
        let stats = outbox.stats();
        assert_eq!(stats.rejected, 2);
        assert_eq!(stats.dropped, 0);
        assert_eq!(stats.accepted, 0);
    }

    #[test]
    fn expired_state_counts_expired_inside_dropped() {
        use std::sync::Mutex;
        let now = Arc::new(Mutex::new(Instant::now()));
        let clock = Arc::clone(&now);
        let outbox = Outbox::new_with_clock(Arc::new(move || *clock.lock().expect("fake clock")));
        outbox.set_connected(true);
        assert_eq!(outbox.admit("robot/state", &[1]), ForwardOutcome::Accepted);
        *now.lock().expect("fake clock") += Duration::from_secs(2);
        assert_eq!(
            outbox.admit("logs/structured", &[2]),
            ForwardOutcome::Accepted
        );
        let publication = outbox.next();
        assert_eq!(publication.map(|p| p.topic), Some("logs/structured"));
        let stats = outbox.stats();
        assert_eq!(stats.expired, 1);
        assert_eq!(stats.dropped, 1);
        assert_eq!(stats.rejected, 0);
    }
}

#[cfg(test)]
mod contention_admission_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn publisher_finishes_while_writer_still_owns_queue_guard() {
        let outbox = Arc::new(Outbox::new());
        let held_by_writer = outbox.queue.lock().expect("writer guard");
        let publisher = Arc::clone(&outbox);
        let (complete_tx, complete_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = complete_tx.send(publisher.admit("robot/state", &[1]));
        });
        let while_guard_held = complete_rx.recv_timeout(Duration::from_secs(5));
        drop(held_by_writer);
        worker.join().expect("join publisher after cleanup release");
        assert_eq!(
            while_guard_held.expect("admission must finish without writer release"),
            ForwardOutcome::Dropped
        );
        assert_eq!(outbox.stats().queued_items, 0);
        assert_eq!(outbox.stats().dropped, 1);
    }
}
