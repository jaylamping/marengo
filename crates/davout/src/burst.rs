//! Spacing for host write bursts during reference work and the enable bootstrap.
//!
//! Every host frame solicits a drive reply and the bench mcp251x holds only two
//! received frames. The Robstride stop sequence (speed zero, neutral MIT,
//! Disable) answers three frames per address, and `perform_stop` used to write
//! all five addresses back to back: 15 frames and 15 replies in about 4.5 ms
//! (3.3 received frames/ms), which overran the controller at the stop that
//! finishes a reference (2026-10-03 17:09:07, `rx_over_errors` 5 to 6, latched
//! Transport). Enable (see `issue_due_enable_writes`) and type-24 writes
//! (`ACTIVE_REPORTING_WRITE_SPACING`) already take one slot per interface per
//! control period; this spaces the groups of the stop, type-24 Off, type-0
//! identity and status-solicit bursts, and the bootstrap's MIT solicits while a
//! target may still stream type-24 (`Supervisor::write_mit_wires`).

use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

/// Minimum time between the first frames of two groups on one interface. A
/// group is one address's stop frames or one single-frame write, so at most
/// three replies arrive per spacing (1.5 received frames/ms), against the
/// 2.9 frames/ms that overran the controller.
pub const BURST_GROUP_SPACING: Duration = Duration::from_millis(2);

/// Per-interface group starts of one burst.
#[derive(Default)]
pub(crate) struct BurstPacer {
    last_group: FxHashMap<String, Instant>,
}

impl BurstPacer {
    /// Block until `interface`'s previous group is [`BURST_GROUP_SPACING`] old,
    /// then start a new group there.
    pub(crate) fn begin_group(&mut self, interface: &str) {
        if let Some(last) = self.last_group.get(interface) {
            let wait = BURST_GROUP_SPACING.saturating_sub(last.elapsed());
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }
        self.record_group(interface);
    }

    /// A group starts on `interface` now, written by a caller with its own
    /// slot rule (the Enable stagger, type-24 slots); the next
    /// [`Self::begin_group`] there keeps the spacing from it.
    pub(crate) fn record_group(&mut self, interface: &str) {
        let now = Instant::now();
        match self.last_group.get_mut(interface) {
            Some(last) => *last = now,
            None => {
                self.last_group.insert(interface.to_owned(), now);
            }
        }
    }
}
