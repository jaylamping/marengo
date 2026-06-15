use std::collections::VecDeque;
use std::sync::RwLock;

use crate::model::LogEventInsert;

pub const DEFAULT_RING_CAPACITY: usize = 10_000;

/// In-memory ring of recent log events for gateway snapshot fanout.
#[derive(Debug)]
pub struct LogRingBuffer {
    capacity: usize,
    slots: RwLock<VecDeque<LogEventInsert>>,
}

impl LogRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            slots: RwLock::new(VecDeque::with_capacity(capacity.min(1024))),
        }
    }

    pub fn push(&self, event: LogEventInsert) {
        let mut guard = match self.slots.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back(event);
    }

    pub fn push_batch(&self, events: &[LogEventInsert]) {
        for event in events {
            self.push(event.clone());
        }
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEventInsert> {
        let guard = match self.slots.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let take = limit.min(guard.len());
        guard.iter().rev().take(take).cloned().collect()
    }

    pub fn preload(&self, events: Vec<LogEventInsert>) {
        let mut guard = match self.slots.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.clear();
        let skip = events.len().saturating_sub(self.capacity);
        for event in events.into_iter().skip(skip) {
            guard.push_back(event);
        }
    }

    pub fn len(&self) -> usize {
        self.slots.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_at_capacity() {
        let ring = LogRingBuffer::new(3);
        for i in 0..5 {
            ring.push(LogEventInsert {
                ts_ms: i,
                level: "info".into(),
                target: "t".into(),
                message: format!("m{i}"),
                session_id: None,
            });
        }
        let recent = ring.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].ts_ms, 4);
        assert_eq!(recent[2].ts_ms, 2);
    }
}
