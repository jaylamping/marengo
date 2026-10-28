//! Independent drive stop for motor-repl.
//!
//! A stop must not depend on anything that can be broken when it is needed:
//! no Supervisor, no `control.yaml`, no URDF, no calibration history, and no
//! CAN interface other than the one a given drive sits on. The only inputs are
//! the `(interface, device_id)` rows of `motors.yaml`, one Robstride type-4
//! Disable per drive and, right after it, one type-24 Off. Robstride drives
//! keep type-24 reporting across host processes and a Disable does not end it:
//! on 2026-10-04 five drives left streaming by a faulted session (about 500
//! frames/s) made every later `marengo-pi` start latch Transport on its first
//! bounded drain (`docs/safety.md`, *Reporting Off at exit*). Every drive gets
//! its own attempts and its own outcome; a failure on one drive or interface
//! never skips another. Stops are never paced.
//!
//! The outcome is honest about what it proves: `sent` means the kernel accepted
//! the frame for transmission, not that the drive left Run mode or stopped
//! reporting. A refused write (`ENOBUFS`/`EAGAIN` while the TX queue is full on
//! a congested or error-passive bus) is retried a bounded number of times
//! before that frame is reported failed.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use marengo_config::MotorStopTarget;
use robstride::{BusError, MotorAddress, MotorBus};

/// Result of the stop writes for one drive. `Err` carries the interface-open
/// or write failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveStop {
    pub address: MotorAddress,
    /// Type-4 Disable.
    pub disable: Result<(), String>,
    /// Type-24 Off, written after the Disable whatever its outcome.
    pub reporting_off: Result<(), String>,
}

/// Per-drive outcomes of one stop, in the order the addresses were given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopReport {
    pub drives: Vec<DriveStop>,
}

impl StopReport {
    /// Drives whose Disable was not sent.
    pub fn failed(&self) -> usize {
        self.drives.iter().filter(|d| d.disable.is_err()).count()
    }

    /// Drives whose type-24 Off was not sent (they may keep streaming).
    pub fn reporting_off_failed(&self) -> usize {
        self.drives
            .iter()
            .filter(|d| d.reporting_off.is_err())
            .count()
    }

    /// Every drive's Disable and type-24 Off were sent.
    pub fn all_sent(&self) -> bool {
        !self.drives.is_empty() && self.failed() == 0 && self.reporting_off_failed() == 0
    }
}

impl fmt::Display for StopReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for drive in &self.drives {
            let interface = &drive.address.interface;
            let device_id = drive.address.device_id;
            for (frame, result) in [
                ("disable", &drive.disable),
                ("reporting-off", &drive.reporting_off),
            ] {
                match result {
                    Ok(()) => writeln!(f, "{frame} {interface}:{device_id} sent")?,
                    Err(error) => writeln!(f, "{frame} {interface}:{device_id} FAILED: {error}")?,
                }
            }
        }
        Ok(())
    }
}

/// Addresses to stop: every configured address, plus, with an explicit
/// `--can-interface`, every distinct device id on that interface too. The
/// override never replaces configured routing for a stop; a configured
/// interface that is absent fails its own drives and the stop exits 1.
pub fn stop_addresses(
    targets: &[MotorStopTarget],
    interface_override: Option<&str>,
) -> Vec<MotorAddress> {
    let mut addresses: Vec<MotorAddress> = Vec::with_capacity(targets.len());
    let routed = targets
        .iter()
        .map(|target| MotorAddress::new(&target.can_interface, target.device_id));
    let overridden = interface_override.into_iter().flat_map(|interface| {
        targets
            .iter()
            .map(move |target| MotorAddress::new(interface, target.device_id))
    });
    for address in routed.chain(overridden) {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses
}

/// Write attempts per stop frame. Repeating a Disable or Off is harmless.
pub const STOP_WRITE_ATTEMPTS: usize = 3;

/// Pause before retrying a refused write, so a full TX queue can drain
/// (about 0.14 ms per frame at 1 Mbit/s).
const STOP_WRITE_RETRY_PAUSE: Duration = Duration::from_millis(5);

/// One stop frame: retry a write the interface refused for now (a full TX
/// queue), up to [`STOP_WRITE_ATTEMPTS`]. Any other error (a down or missing
/// interface) fails at once, so the stop's later frames and interfaces are not
/// delayed by retries that cannot succeed.
fn send_stop_frame(
    mut write: impl FnMut() -> Result<(), BusError>,
    pause: Duration,
) -> Result<(), String> {
    let mut attempt = 1;
    loop {
        match write() {
            Ok(()) => return Ok(()),
            Err(
                error @ BusError::Send {
                    transient: true, ..
                },
            ) if attempt < STOP_WRITE_ATTEMPTS => {
                eprintln!("stop write refused ({error}); retrying");
                attempt += 1;
                std::thread::sleep(pause);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Send one Disable, then one type-24 Off, to every address, back to back.
/// `open` is called once per distinct interface; an interface that cannot be
/// opened fails only its own drives.
pub fn disable_drives<B: MotorBus>(
    addresses: &[MotorAddress],
    mut open: impl FnMut(&str) -> Result<B, BusError>,
) -> StopReport {
    let mut buses: HashMap<&str, Result<B, String>> = HashMap::new();
    let mut drives = Vec::with_capacity(addresses.len());
    for address in addresses {
        let bus = buses
            .entry(address.interface.as_str())
            .or_insert_with(|| open(&address.interface).map_err(|error| format!("open: {error}")));
        let (disable, reporting_off) = match bus {
            Ok(bus) => (
                send_stop_frame(|| bus.disable_drive_at(address), STOP_WRITE_RETRY_PAUSE),
                send_stop_frame(
                    || bus.disable_active_reporting_at(address),
                    STOP_WRITE_RETRY_PAUSE,
                ),
            ),
            Err(error) => (Err(error.clone()), Err(error.clone())),
        };
        drives.push(DriveStop {
            address: address.clone(),
            disable,
            reporting_off,
        });
    }
    StopReport { drives }
}

/// [`disable_drives`] over the real SocketCAN interfaces, each opened
/// transmit-only and unowned: no receive-side option can fail a stop, and a
/// running owner cannot block it.
pub fn disable_drives_socketcan(addresses: &[MotorAddress]) -> StopReport {
    disable_drives(addresses, robstride::RuntimeBus::socketcan_transmit_only)
}

/// `set-zero` can touch a drive and therefore arms an independent exit stop
/// before its reference transaction begins.
pub fn arms_exit_stop(command: &str) -> bool {
    command == "set-zero"
}

/// How a command ended, as the exit stop needs to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    /// The command ran (or failed) with this process exit code.
    Exited(i32),
    /// Another motor owner holds an interface, so the command was refused
    /// before it opened a CAN socket: it sent nothing, and the drives belong
    /// to that owner.
    OwnerRefused,
}

impl CommandOutcome {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Exited(code) => code,
            Self::OwnerRefused => 1,
        }
    }

    /// The outcome of a failed owner open: an ownership refusal is its own
    /// outcome, every other open failure is an ordinary error exit.
    pub fn from_owner_open_error(error: &BusError) -> Self {
        match error {
            BusError::InterfaceOwned { .. } => Self::OwnerRefused,
            _ => Self::Exited(1),
        }
    }
}

/// Whether a `set-zero` that ended this way needs an additional stop. Davout's
/// qualified transaction performs its own stop before successful return. An
/// ownership refusal sent nothing, so there is nothing to stop; the exit stop
/// would disable the live owner's drives and drop its arm.
pub fn exit_stop_required(command: &str, outcome: CommandOutcome) -> bool {
    arms_exit_stop(command)
        && match outcome {
            CommandOutcome::Exited(code) => !(command == "set-zero" && code == 0),
            CommandOutcome::OwnerRefused => false,
        }
}

/// The error-exit stop for a command that ended with `outcome`: every drive
/// once when [`exit_stop_required`], otherwise nothing is opened or sent.
pub fn exit_stop<B: MotorBus>(
    command: &str,
    outcome: CommandOutcome,
    addresses: &[MotorAddress],
    open: impl FnMut(&str) -> Result<B, BusError>,
) -> Option<StopReport> {
    if !exit_stop_required(command, outcome) {
        return None;
    }
    eprintln!("motor-repl: exit stop, disabling every drive");
    Some(disable_drives(addresses, open))
}

/// Process exit code for a fatal signal, as a shell reports it.
pub fn signal_exit_code(signal: i32) -> i32 {
    128 + signal
}

/// Pause before the signal stop's second pass.
const SIGNAL_STOP_SECOND_PASS_AFTER: Duration = Duration::from_millis(20);

/// The signal stop: every drive, a pause, then every drive again. The main
/// thread may be inside Davout's set-zero transaction between Enable writes
/// when the signal arrives; an Enable it writes after the first pass is
/// caught by the second (the owner's socket also reads the first pass as
/// another writer and latches Transport on its next receive).
pub fn signal_stop<B: MotorBus>(
    addresses: &[MotorAddress],
    mut open: impl FnMut(&str) -> Result<B, BusError>,
    pause: Duration,
) -> [StopReport; 2] {
    let first = disable_drives(addresses, &mut open);
    std::thread::sleep(pause);
    [first, disable_drives(addresses, open)]
}

/// Stop all drives from a dedicated thread when SIGTERM/SIGINT/SIGHUP arrives,
/// then exit. It uses its own sockets, so it works while the main thread is
/// blocked inside a Davout transaction.
#[cfg(unix)]
pub fn install_signal_stop(addresses: Vec<MotorAddress>) -> Result<(), String> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])
        .map_err(|error| format!("install signal handler: {error}"))?;
    std::thread::Builder::new()
        .name("motor-repl-signal-stop".into())
        .spawn(move || {
            if let Some(signal) = signals.forever().next() {
                eprintln!("motor-repl: signal {signal}, disabling every drive");
                let reports = signal_stop(
                    &addresses,
                    robstride::RuntimeBus::socketcan_transmit_only,
                    SIGNAL_STOP_SECOND_PASS_AFTER,
                );
                for report in &reports {
                    eprint!("{report}");
                }
                std::process::exit(signal_exit_code(signal));
            }
        })
        .map_err(|error| format!("spawn signal stop thread: {error}"))?;
    Ok(())
}

#[cfg(not(unix))]
pub fn install_signal_stop(_addresses: Vec<MotorAddress>) -> Result<(), String> {
    Err("signal-driven stop requires a unix target".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use robstride::receive::ReceiveAttempt;
    use robstride::{
        unpack_ext_id, CanBus, CanFrame, CommunicationType, MemoryBus, DEFAULT_HOST_ID,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    fn target(interface: &str, device_id: u8) -> MotorStopTarget {
        MotorStopTarget {
            can_interface: interface.into(),
            device_id,
        }
    }

    /// A bus whose writes all fail.
    struct DeadBus;

    impl CanBus for DeadBus {
        fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), BusError> {
            Err(BusError::Send {
                message: "No buffer space available".into(),
                transient: true,
            })
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for DeadBus {}

    /// Shares its transmit log so the test can read it after `disable_drives`
    /// dropped the bus.
    struct SharedBus(Rc<RefCell<MemoryBus>>);

    impl CanBus for SharedBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            self.0.borrow_mut().send_frame(frame)
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for SharedBus {}

    /// Accepts every write except type-24.
    struct NoReportingBus(MemoryBus);

    impl CanBus for NoReportingBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            let id = unpack_ext_id(frame.id).expect("extended id");
            if id.comm_type == CommunicationType::ActiveReporting.as_u8() {
                return Err(BusError::Send {
                    message: "No buffer space available".into(),
                    transient: true,
                });
            }
            self.0.send_frame(frame)
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for NoReportingBus {}

    /// Refuses its first `refusals` writes as a full TX queue would.
    struct CongestedBus {
        refusals: usize,
        inner: MemoryBus,
    }

    impl CanBus for CongestedBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            if self.refusals > 0 {
                self.refusals -= 1;
                return Err(BusError::Send {
                    message: "No buffer space available (os error 105)".into(),
                    transient: true,
                });
            }
            self.inner.send_frame(frame)
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for CongestedBus {}

    /// DRV-07: one transient ENOBUFS no longer fails a drive's stop.
    #[test]
    fn a_refused_stop_write_is_retried() {
        let addresses = stop_addresses(&[target("can0", 1)], None);
        let report = disable_drives(&addresses, |_| {
            Ok(CongestedBus {
                refusals: STOP_WRITE_ATTEMPTS - 1,
                inner: MemoryBus::default(),
            })
        });
        assert!(report.all_sent(), "{report}");
    }

    #[test]
    fn stop_write_retries_are_bounded_and_skip_permanent_errors() {
        let mut calls = 0;
        let error = send_stop_frame(
            || {
                calls += 1;
                Err(BusError::Send {
                    message: "No buffer space available".into(),
                    transient: true,
                })
            },
            Duration::ZERO,
        )
        .expect_err("queue never drains");
        assert_eq!(calls, STOP_WRITE_ATTEMPTS);
        assert!(error.contains("No buffer space"), "{error}");
        let mut calls = 0;
        send_stop_frame(
            || {
                calls += 1;
                Err(BusError::Driver("no such device".into()))
            },
            Duration::ZERO,
        )
        .expect_err("not transient");
        assert_eq!(calls, 1);
    }

    /// A permanent write error (interface down) is not retried, so it adds
    /// no retry pauses before the stop reaches the next drive and interface.
    #[test]
    fn a_permanent_send_error_is_not_retried_and_later_interfaces_still_stop() {
        let mut calls = 0;
        let error = send_stop_frame(
            || {
                calls += 1;
                Err(BusError::Send {
                    message: "Network is down (os error 100)".into(),
                    transient: false,
                })
            },
            Duration::ZERO,
        )
        .expect_err("interface down");
        assert_eq!(calls, 1, "a down interface is not retried");
        assert!(error.contains("Network is down"), "{error}");

        let down_writes = Rc::new(RefCell::new(0));
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let addresses = stop_addresses(&[target("can0", 1), target("can1", 2)], None);
        let report = disable_drives(&addresses, |interface| {
            Ok(if interface == "can0" {
                DownOrShared::Down(Rc::clone(&down_writes))
            } else {
                DownOrShared::Up(SharedBus(Rc::clone(&log)))
            })
        });
        assert_eq!(
            *down_writes.borrow(),
            2,
            "one Disable and one Off, no retries"
        );
        assert_eq!(report.failed(), 1, "{report}");
        let log = log.borrow();
        assert_eq!(log.tx.len(), 2);
        assert_is_disable_to(&log.tx[0], 2);
        assert_is_reporting_off_to(&log.tx[1], 2);
    }

    /// can0 is down (every write fails permanently); can1 records.
    enum DownOrShared {
        Down(Rc<RefCell<usize>>),
        Up(SharedBus),
    }

    impl CanBus for DownOrShared {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            match self {
                Self::Down(writes) => {
                    *writes.borrow_mut() += 1;
                    Err(BusError::Send {
                        message: "Network is down (os error 100)".into(),
                        transient: false,
                    })
                }
                Self::Up(bus) => bus.send_frame(frame),
            }
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for DownOrShared {}

    fn assert_is_disable_to(frame: &CanFrame, device_id: u8) {
        let id = unpack_ext_id(frame.id).expect("extended id");
        assert_eq!(id.comm_type, CommunicationType::Disable.as_u8());
        assert_eq!(id.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(id.device_id, device_id);
        assert_eq!(frame.data, [0; 8]);
        assert!(frame.extended);
    }

    fn assert_is_reporting_off_to(frame: &CanFrame, device_id: u8) {
        let id = unpack_ext_id(frame.id).expect("extended id");
        assert_eq!(id.comm_type, CommunicationType::ActiveReporting.as_u8());
        assert_eq!(id.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(id.device_id, device_id);
        assert_eq!(frame.data[6], 0x00, "type-24 Off");
        assert!(frame.extended);
    }

    /// 2026-10-04: a Disable does not end type-24 reporting, so a stop that
    /// sent only Disable left inherited streams running for the next owner.
    #[test]
    fn every_address_gets_a_disable_then_a_type_24_off() {
        let addresses = stop_addresses(
            &[
                target("can0", 1),
                target("can0", 2),
                target("can0", 3),
                target("can0", 4),
                target("can0", 5),
            ],
            None,
        );
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let opened = RefCell::new(Vec::new());
        let report = disable_drives(&addresses, |interface| {
            opened.borrow_mut().push(interface.to_string());
            Ok(SharedBus(Rc::clone(&log)))
        });
        assert!(report.all_sent());
        assert_eq!(report.drives.len(), 5);
        assert_eq!(opened.into_inner(), vec!["can0"], "one open per interface");
        let log = log.borrow();
        assert_eq!(log.tx.len(), 10, "one Disable and one Off per address");
        for (pair, id) in log.tx.chunks(2).zip(1..=5) {
            assert_is_disable_to(&pair[0], id);
            assert_is_reporting_off_to(&pair[1], id);
        }
    }

    #[test]
    fn a_failed_off_is_reported_without_hiding_the_sent_disable() {
        let addresses = stop_addresses(&[target("can0", 1), target("can0", 2)], None);
        let report = disable_drives(&addresses, |_| Ok(NoReportingBus(MemoryBus::default())));
        assert_eq!(report.failed(), 0, "every Disable was sent");
        assert_eq!(report.reporting_off_failed(), 2);
        assert!(!report.all_sent());
        let text = report.to_string();
        assert!(text.contains("disable can0:1 sent"), "{text}");
        assert!(
            text.contains("reporting-off can0:1 FAILED: CAN send failed"),
            "{text}"
        );
    }

    #[test]
    fn dead_interface_fails_only_its_own_drives() {
        let addresses = stop_addresses(
            &[target("can0", 1), target("can1", 2), target("can0", 3)],
            None,
        );
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let report = disable_drives(&addresses, |interface| {
            if interface == "can1" {
                Err(BusError::Driver("No such device".into()))
            } else {
                Ok(SharedBus(Rc::clone(&log)))
            }
        });
        assert_eq!(report.failed(), 1);
        assert!(!report.all_sent());
        let by_id = |id: u8| {
            report
                .drives
                .iter()
                .find(|d| d.address.device_id == id)
                .expect("drive outcome")
        };
        assert!(by_id(1).disable.is_ok() && by_id(1).reporting_off.is_ok());
        assert!(by_id(3).disable.is_ok() && by_id(3).reporting_off.is_ok());
        let error = by_id(2).disable.clone().expect_err("can1 unopened");
        assert!(error.contains("No such device"), "{error}");
        assert!(by_id(2).reporting_off.is_err());
        assert_eq!(report.reporting_off_failed(), 1);
        let log = log.borrow();
        assert_eq!(log.tx.len(), 4, "can0 drives still stopped");
        assert_is_disable_to(&log.tx[0], 1);
        assert_is_reporting_off_to(&log.tx[1], 1);
        assert_is_disable_to(&log.tx[2], 3);
        assert_is_reporting_off_to(&log.tx[3], 3);
    }

    #[test]
    fn a_failed_write_is_reported_per_drive_and_does_not_stop_the_rest() {
        let addresses = stop_addresses(&[target("can0", 1), target("can0", 2)], None);
        let report = disable_drives(&addresses, |_| Ok(DeadBus));
        assert_eq!(report.failed(), 2);
        assert_eq!(report.reporting_off_failed(), 2);
        let text = report.to_string();
        assert!(
            text.contains("disable can0:1 FAILED: CAN send failed"),
            "{text}"
        );
        assert!(text.contains("reporting-off can0:1 FAILED"), "{text}");
        assert!(text.contains("disable can0:2 FAILED"), "{text}");
        assert!(text.contains("reporting-off can0:2 FAILED"), "{text}");
    }

    #[test]
    fn report_text_lists_every_drive_in_order() {
        let addresses = stop_addresses(&[target("can0", 1), target("can1", 9)], None);
        let report = disable_drives(&addresses, |interface| {
            if interface == "can1" {
                Err(BusError::Driver("down".into()))
            } else {
                Ok(MemoryBus::default())
            }
        });
        let text = report.to_string();
        assert_eq!(
            text,
            "disable can0:1 sent\nreporting-off can0:1 sent\n\
             disable can1:9 FAILED: open: driver error: down\n\
             reporting-off can1:9 FAILED: open: driver error: down\n"
        );
    }

    /// DRV-01: an override adds its interface; configured drives on the
    /// other buses are still stopped.
    #[test]
    fn interface_override_adds_to_configured_routing_and_deduplicates() {
        let addresses = stop_addresses(
            &[target("can0", 1), target("can1", 1), target("can1", 2)],
            Some("vcan0"),
        );
        assert_eq!(
            addresses,
            vec![
                MotorAddress::new("can0", 1),
                MotorAddress::new("can1", 1),
                MotorAddress::new("can1", 2),
                MotorAddress::new("vcan0", 1),
                MotorAddress::new("vcan0", 2),
            ]
        );
    }

    #[test]
    fn empty_target_list_is_not_a_successful_stop() {
        let report = disable_drives::<MemoryBus>(&[], |_| Ok(MemoryBus::default()));
        assert!(!report.all_sent());
    }

    #[test]
    fn exit_stop_arms_only_for_set_zero() {
        assert!(arms_exit_stop("set-zero"));
        for command in [
            "status",
            "homing-status",
            "home",
            "enable",
            "disable",
            "jog",
            "speed",
            "speed-stop",
            "gravity-on",
            "gravity-off",
            "torque-cmd",
            "gravity-preview",
        ] {
            assert!(!arms_exit_stop(command), "{command}");
        }
    }

    #[test]
    fn exit_stop_runs_on_error_and_on_one_shot_success() {
        assert!(
            exit_stop_required("set-zero", CommandOutcome::Exited(1)),
            "failed set-zero must stop"
        );
        assert!(
            !exit_stop_required("set-zero", CommandOutcome::Exited(0)),
            "Davout already stopped and verified"
        );
        for command in ["home", "enable", "jog", "speed", "speed-stop"] {
            assert!(
                !exit_stop_required(command, CommandOutcome::Exited(1)),
                "{command}"
            );
        }
    }

    /// A set-zero refused because another motor owner holds the bus sends no
    /// frame and opens no socket; any other error exit stops every drive.
    #[test]
    fn an_owner_refused_set_zero_sends_no_frames() {
        let addresses = stop_addresses(&[target("can0", 1), target("can1", 2)], None);
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let mut opens = 0;
        let report = exit_stop("set-zero", CommandOutcome::OwnerRefused, &addresses, |_| {
            opens += 1;
            Ok(SharedBus(Rc::clone(&log)))
        });
        assert!(report.is_none());
        assert_eq!(opens, 0, "no socket opened");
        assert!(log.borrow().tx.is_empty(), "no frame sent");

        let report = exit_stop("set-zero", CommandOutcome::Exited(1), &addresses, |_| {
            Ok(SharedBus(Rc::clone(&log)))
        })
        .expect("a failed set-zero stops every drive");
        assert!(report.all_sent(), "{report}");
        assert_eq!(log.borrow().tx.len(), 4);
    }

    /// A set-zero refused because marengo-pi owns the bus sent nothing; its
    /// exit stop would disable the live owner's drives. Every other open
    /// failure keeps the exit stop, and both still exit 1.
    #[test]
    fn an_ownership_refusal_skips_the_exit_stop_and_other_open_failures_do_not() {
        let refused = CommandOutcome::from_owner_open_error(&BusError::InterfaceOwned {
            interface: "can0".into(),
        });
        assert_eq!(refused.exit_code(), 1);
        assert!(!exit_stop_required("set-zero", refused));
        for error in [
            BusError::Driver("SocketCAN open failed: No such device".into()),
            BusError::DuplicateMotorAddress {
                interface: "can0".into(),
                device_id: 1,
            },
        ] {
            let failed = CommandOutcome::from_owner_open_error(&error);
            assert_eq!(failed.exit_code(), 1, "{error}");
            assert!(exit_stop_required("set-zero", failed), "{error}");
        }
    }

    /// DRV-08: an Enable the main thread writes after the first pass is
    /// still followed by a Disable.
    #[test]
    fn signal_stop_disables_every_drive_twice() {
        let addresses = stop_addresses(&[target("can0", 1), target("can1", 2)], None);
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let mut opens = 0;
        let reports = signal_stop(
            &addresses,
            |_| {
                opens += 1;
                Ok(SharedBus(Rc::clone(&log)))
            },
            Duration::ZERO,
        );
        assert!(reports.iter().all(StopReport::all_sent));
        assert_eq!(opens, 4, "each pass opens its own sockets");
        let log = log.borrow();
        assert_eq!(log.tx.len(), 8);
        for pass in log.tx.chunks(4) {
            assert_is_disable_to(&pass[0], 1);
            assert_is_reporting_off_to(&pass[1], 1);
            assert_is_disable_to(&pass[2], 2);
            assert_is_reporting_off_to(&pass[3], 2);
        }
    }

    #[test]
    fn signal_exit_codes_follow_the_shell_convention() {
        assert_eq!(signal_exit_code(15), 143);
        assert_eq!(signal_exit_code(2), 130);
    }
}
