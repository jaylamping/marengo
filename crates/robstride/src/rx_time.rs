//! Kernel receive times on the host monotonic clock.
//!
//! SocketCAN stamps every received skb, drive frames and own-TX echoes alike,
//! with `CLOCK_REALTIME` when the driver hands it to the network stack. A host
//! that stalls before reading would otherwise stamp a whole backlog with the
//! time it finally read it. [`map_kernel_rx_time`] carries the kernel time onto
//! [`Instant`] as `read − (wall_at_read − kernel)`, bounded to `[floor, read]`:
//!
//! - never later than the read, which is the time every bus stamped before;
//! - never earlier than `floor`, a host instant the frame provably postdates (a
//!   read that began then found the queue empty, or the previous frame on the
//!   same FIFO socket). This bounds what a forward realtime step can backdate.
//!
//! A missing stamp or a stamp after the wall clock (realtime stepped backward)
//! falls back to the read time. A stamp older than [`MAX_KERNEL_RX_AGE`] is
//! counted stale and stays as old as the evidence allows: its mapped time, or
//! the floor if that is later. Every
//! non-kernel outcome is counted per socket and logged at power-of-two counts,
//! so a broken timestamp path is visible rather than silently degrading to
//! read times.
//!
//! Only the SocketCAN backend consumes this; other buses supply their own
//! instants.
#![cfg_attr(
    not(all(feature = "socketcan", target_os = "linux")),
    allow(
        dead_code,
        reason = "consumed only by the Linux SocketCAN backend; tested everywhere"
    )
)]

use std::time::{Duration, Instant, SystemTime};

/// A kernel stamp older than this at read is implausible for a bus polled at
/// the control rate; such frames are counted stale.
pub const MAX_KERNEL_RX_AGE: Duration = Duration::from_secs(1);

/// How a socket's received frames were stamped since it opened.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RxTimestampCounts {
    /// Kernel time mapped onto the monotonic clock.
    pub kernel: u64,
    /// Kernel time earlier than a provable floor; raised to the floor.
    pub floored: u64,
    /// No kernel time delivered; read time used.
    pub missing: u64,
    /// Kernel time later than the wall clock at read; read time used.
    pub clock_backward: u64,
    /// Kernel time older than [`MAX_KERNEL_RX_AGE`]; kept old.
    pub stale: u64,
}

impl RxTimestampCounts {
    /// Frames whose kernel time was absent or implausible.
    pub fn anomalies(&self) -> u64 {
        self.missing + self.clock_backward + self.stale
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RxStampSource {
    Kernel,
    Floored,
    Missing,
    ClockBackward,
    Stale,
}

/// `wall_at_read` must be sampled before `read_instant`, both after the read:
/// the sampling gap then moves the result toward the read, never before the
/// kernel time. The result always lies in `[min(floor, read), read]`.
pub(crate) fn map_kernel_rx_time(
    kernel: Option<SystemTime>,
    wall_at_read: SystemTime,
    read_instant: Instant,
    floor: Instant,
) -> (Instant, RxStampSource) {
    let floor = floor.min(read_instant);
    let Some(kernel) = kernel else {
        return (read_instant, RxStampSource::Missing);
    };
    let Ok(age) = wall_at_read.duration_since(kernel) else {
        return (read_instant, RxStampSource::ClockBackward);
    };
    let stale = age > MAX_KERNEL_RX_AGE;
    match read_instant.checked_sub(age) {
        Some(at) if at >= floor => (
            at,
            if stale {
                RxStampSource::Stale
            } else {
                RxStampSource::Kernel
            },
        ),
        _ => (
            floor,
            if stale {
                RxStampSource::Stale
            } else {
                RxStampSource::Floored
            },
        ),
    }
}

/// A kernel `timespec`; `None` for the unset zero stamp or invalid nanoseconds.
pub(crate) fn wall_time_from_timespec(seconds: u64, nanoseconds: u32) -> Option<SystemTime> {
    if nanoseconds >= 1_000_000_000 || (seconds == 0 && nanoseconds == 0) {
        return None;
    }
    SystemTime::UNIX_EPOCH.checked_add(Duration::new(seconds, nanoseconds))
}

/// One socket's stamping state. Frames leave a socket in FIFO order, so each
/// stamp also floors the next.
#[derive(Debug)]
pub(crate) struct RxClock {
    floor: Instant,
    counts: RxTimestampCounts,
}

impl RxClock {
    /// `opened` must precede the socket's bind: no frame can be queued earlier.
    pub(crate) fn new(opened: Instant) -> Self {
        Self {
            floor: opened,
            counts: RxTimestampCounts::default(),
        }
    }

    /// A read that began at `before_read` found the queue empty, so every frame
    /// read later was queued after it.
    pub(crate) fn observe_empty(&mut self, before_read: Instant) {
        self.floor = self.floor.max(before_read);
    }

    pub(crate) fn stamp(
        &mut self,
        interface: &str,
        kernel: Option<SystemTime>,
        wall_at_read: SystemTime,
        read_instant: Instant,
    ) -> Instant {
        let (at, source) = map_kernel_rx_time(kernel, wall_at_read, read_instant, self.floor);
        self.floor = self.floor.max(at);
        let counts = &mut self.counts;
        let anomaly = match source {
            RxStampSource::Kernel => {
                counts.kernel += 1;
                false
            }
            RxStampSource::Floored => {
                counts.floored += 1;
                false
            }
            RxStampSource::Missing => {
                counts.missing += 1;
                true
            }
            RxStampSource::ClockBackward => {
                counts.clock_backward += 1;
                true
            }
            RxStampSource::Stale => {
                counts.stale += 1;
                true
            }
        };
        if anomaly && counts.anomalies().is_power_of_two() {
            tracing::warn!(
                interface,
                ?source,
                missing = counts.missing,
                clock_backward = counts.clock_backward,
                stale = counts.stale,
                kernel = counts.kernel,
                floored = counts.floored,
                "SocketCAN kernel receive timestamp unusable"
            );
        }
        at
    }

    pub(crate) fn counts(&self) -> RxTimestampCounts {
        self.counts
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const MS: Duration = Duration::from_millis(1);

    struct Clocks {
        wall: SystemTime,
        read: Instant,
        floor: Instant,
    }

    /// A read 200 ms after the floor, with the wall clock sampled at the read.
    fn clocks() -> Option<Clocks> {
        let floor = Instant::now();
        Some(Clocks {
            wall: SystemTime::now(),
            read: floor.checked_add(200 * MS)?,
            floor,
        })
    }

    fn stamp(age_ms: i64, at: &Clocks) -> Option<SystemTime> {
        let magnitude = Duration::from_millis(age_ms.unsigned_abs());
        if age_ms >= 0 {
            at.wall.checked_sub(magnitude)
        } else {
            at.wall.checked_add(magnitude)
        }
    }

    #[test]
    fn kernel_stamp_backdates_to_arrival() {
        let at = clocks().expect("clock range");
        let (received, source) = map_kernel_rx_time(stamp(90, &at), at.wall, at.read, at.floor);
        assert_eq!(source, RxStampSource::Kernel);
        assert_eq!(at.read.duration_since(received), 90 * MS);
    }

    #[test]
    fn zero_age_is_the_read_instant() {
        let at = clocks().expect("clock range");
        let (received, source) = map_kernel_rx_time(stamp(0, &at), at.wall, at.read, at.floor);
        assert_eq!((received, source), (at.read, RxStampSource::Kernel));
    }

    #[test]
    fn missing_stamp_falls_back_to_read_instant() {
        let at = clocks().expect("clock range");
        let (received, source) = map_kernel_rx_time(None, at.wall, at.read, at.floor);
        assert_eq!((received, source), (at.read, RxStampSource::Missing));
    }

    #[test]
    fn negative_age_falls_back_to_read_instant() {
        let at = clocks().expect("clock range");
        let (received, source) = map_kernel_rx_time(stamp(-5, &at), at.wall, at.read, at.floor);
        assert_eq!((received, source), (at.read, RxStampSource::ClockBackward));
    }

    #[test]
    fn never_later_than_read_instant() {
        let at = clocks().expect("clock range");
        for age in [-1_000, -1, 0, 1, 150, 199, 200, 201, 5_000] {
            let (received, _) = map_kernel_rx_time(stamp(age, &at), at.wall, at.read, at.floor);
            assert!(received <= at.read, "age {age} ms");
            assert!(received >= at.floor, "age {age} ms");
        }
    }

    #[test]
    fn stamp_before_floor_is_raised_to_floor() {
        // The floor is 200 ms before the read; a 350 ms age predates a read
        // that already found the queue empty (a forward realtime step).
        let at = clocks().expect("clock range");
        let (received, source) = map_kernel_rx_time(stamp(350, &at), at.wall, at.read, at.floor);
        assert_eq!((received, source), (at.floor, RxStampSource::Floored));
    }

    #[test]
    fn stale_age_stays_old() {
        let floor = Instant::now();
        let Some(read) = floor.checked_add(5_000 * MS) else {
            return;
        };
        let wall = SystemTime::now();
        let kernel = wall.checked_sub(1_500 * MS);
        let (received, source) = map_kernel_rx_time(kernel, wall, read, floor);
        assert_eq!(source, RxStampSource::Stale);
        assert_eq!(read.duration_since(received), 1_500 * MS);
        assert!(read.duration_since(received) > MAX_KERNEL_RX_AGE);
    }

    #[test]
    fn stale_age_beyond_floor_is_counted_stale_at_floor() {
        let at = clocks().expect("clock range");
        let (received, source) =
            map_kernel_rx_time(stamp(3_600_000, &at), at.wall, at.read, at.floor);
        assert_eq!((received, source), (at.floor, RxStampSource::Stale));
    }

    #[test]
    fn floor_after_read_cannot_move_stamp_past_read() {
        let at = clocks().expect("clock range");
        let late_floor = at.read.checked_add(10 * MS).expect("clock range");
        let (received, _) = map_kernel_rx_time(stamp(50, &at), at.wall, at.read, late_floor);
        assert_eq!(received, at.read);
    }

    #[test]
    fn timespec_conversion_rejects_unset_and_invalid() {
        assert_eq!(wall_time_from_timespec(0, 0), None);
        assert_eq!(wall_time_from_timespec(1, 1_000_000_000), None);
        assert_eq!(
            wall_time_from_timespec(1_759_000_000, 5),
            SystemTime::UNIX_EPOCH.checked_add(Duration::new(1_759_000_000, 5))
        );
    }

    #[test]
    fn clock_floors_each_frame_on_the_previous_and_counts_fallbacks() {
        let at = clocks().expect("clock range");
        let mut clock = RxClock::new(at.floor);
        // A missing stamp falls back to the read; the FIFO successor read in the
        // same drain cannot then claim an arrival before it.
        let first = clock.stamp("vcan0", None, at.wall, at.read);
        assert_eq!(first, at.read);
        let later = at.read.checked_add(MS).expect("clock range");
        let second = clock.stamp("vcan0", stamp(100, &at), at.wall, later);
        assert_eq!(second, at.read);
        assert_eq!(
            clock.counts(),
            RxTimestampCounts {
                floored: 1,
                missing: 1,
                ..RxTimestampCounts::default()
            }
        );
    }

    #[test]
    fn empty_queue_observation_floors_later_frames() {
        let at = clocks().expect("clock range");
        let mut clock = RxClock::new(at.floor);
        let empty_at = at.floor.checked_add(150 * MS).expect("clock range");
        clock.observe_empty(empty_at);
        // A 120 ms age would place arrival 80 ms after open, before the empty read.
        let received = clock.stamp("vcan0", stamp(120, &at), at.wall, at.read);
        assert_eq!(received, empty_at);
        // An earlier observation never lowers the floor.
        clock.observe_empty(at.floor);
        let received = clock.stamp("vcan0", stamp(190, &at), at.wall, at.read);
        assert_eq!(received, empty_at);
        assert_eq!(clock.counts().floored, 2);
    }
}
