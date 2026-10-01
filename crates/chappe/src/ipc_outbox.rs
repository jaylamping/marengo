//! Bounded runtime publication classes; command topics never enter this outbox.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, TryLockError};
use std::time::{Duration, Instant};

pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub const EVENT_CAPACITY: usize = 128;
pub const EVENT_BYTE_CAPACITY: usize = 512 * 1024;
const LATEST_TOPICS: [&str; 7] = [
    "robot/safety",
    "robot/heartbeat",
    "robot/state",
    "sensors/imu/torso",
    "host/metrics/pi",
    "host/metrics/jetson",
    "robot/actuator/limits",
];
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
#[derive(Debug, Clone, Default)]
pub struct IpcQueueStats {
    pub connected: bool,
    pub queued_items: usize,
    pub queued_bytes: usize,
    pub oldest_age_ms: u64,
    pub accepted: u64,
    pub coalesced: u64,
    pub dropped: u64,
    pub admitted_disconnected: u64,
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
    queue: Mutex<Queue>,
    ready: Condvar,
    connected: AtomicBool,
    closed: AtomicBool,
    accepted: AtomicU64,
    coalesced: AtomicU64,
    dropped: AtomicU64,
    disconnected: AtomicU64,
    connection_changes: tokio::sync::broadcast::Sender<bool>,
}

impl Outbox {
    pub fn new() -> Self {
        Self {
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
            disconnected: AtomicU64::new(0),
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
        self.closed.store(true, Ordering::Relaxed);
        self.set_connected(false);
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn subscribe_connection(&self) -> tokio::sync::broadcast::Receiver<bool> {
        self.connection_changes.subscribe()
    }

    pub fn admit(&self, topic: &str, payload: &[u8]) -> ForwardOutcome {
        let latest_index = LATEST_TOPICS
            .iter()
            .position(|candidate| *candidate == topic);
        let event_topic = match topic {
            "logs/structured" => Some("logs/structured"),
            "robot/audit/action" => Some("robot/audit/action"),
            "robot/audit/tuning" => Some("robot/audit/tuning"),
            _ => None,
        };
        if self.closed()
            || payload.len() > MAX_PAYLOAD_BYTES
            || (latest_index.is_none() && event_topic.is_none())
        {
            return self.drop_publication();
        }
        let mut queue = match self.queue.try_lock() {
            Ok(queue) => queue,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return self.drop_publication(),
        };
        let outcome = if let Some(index) = latest_index {
            let replaced = queue.latest[index].is_some();
            queue.latest[index] = Some(Publication {
                topic: LATEST_TOPICS[index],
                payload: payload.to_vec(),
                admitted: Instant::now(),
            });
            if replaced {
                ForwardOutcome::Coalesced
            } else {
                ForwardOutcome::Accepted
            }
        } else if let Some(topic) = event_topic {
            if queue.events.len() >= EVENT_CAPACITY
                || queue.event_bytes + payload.len() > EVENT_BYTE_CAPACITY
            {
                return self.drop_publication();
            }
            queue.events.push_back(Publication {
                topic,
                payload: payload.to_vec(),
                admitted: Instant::now(),
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
                    if state.admitted.elapsed() <= STATE_MAX_AGE {
                        return Some(state);
                    }
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
                age.max(p.admitted.elapsed().as_millis() as u64),
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
            admitted_disconnected: self.disconnected.load(Ordering::Relaxed),
        }
    }
}
