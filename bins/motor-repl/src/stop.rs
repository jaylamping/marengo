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
//! reporting.

use std::collections::HashMap;
use std::fmt;

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

/// Addresses to stop. With an interface override every distinct device id is
/// addressed on that interface (the override replaces `motors.yaml` routing,
/// as it does for every other motor-repl command).
pub fn stop_addresses(
    targets: &[MotorStopTarget],
    interface_override: Option<&str>,
) -> Vec<MotorAddress> {
    let mut addresses: Vec<MotorAddress> = Vec::with_capacity(targets.len());
    for target in targets {
        let interface = interface_override.unwrap_or(&target.can_interface);
        let address = MotorAddress::new(interface, target.device_id);
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses
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
                bus.disable_drive_at(address)
                    .map_err(|error| error.to_string()),
                bus.disable_active_reporting_at(address)
                    .map_err(|error| error.to_string()),
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

/// [`disable_drives`] over the real SocketCAN interfaces.
pub fn disable_drives_socketcan(addresses: &[MotorAddress]) -> StopReport {
    disable_drives(addresses, robstride::RuntimeBus::socketcan)
}

/// `set-zero` can touch a drive and therefore arms an independent exit stop
/// before its reference transaction begins.
pub fn arms_exit_stop(command: &str) -> bool {
    command == "set-zero"
}

/// Whether a failed `set-zero` needs an additional stop. Davout's qualified
/// transaction performs its own stop before successful return.
pub fn exit_stop_required(command: &str, exit_code: i32) -> bool {
    arms_exit_stop(command) && !(command == "set-zero" && exit_code == 0)
}

/// Process exit code for a fatal signal, as a shell reports it.
pub fn signal_exit_code(signal: i32) -> i32 {
    128 + signal
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
                let report = disable_drives_socketcan(&addresses);
                eprint!("{report}");
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
                });
            }
            self.0.send_frame(frame)
        }
        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            Ok(ReceiveAttempt::Idle)
        }
    }
    impl MotorBus for NoReportingBus {}

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

    #[test]
    fn interface_override_reroutes_and_deduplicates_by_device_id() {
        let addresses = stop_addresses(
            &[target("can0", 1), target("can1", 1), target("can1", 2)],
            Some("vcan0"),
        );
        assert_eq!(
            addresses,
            vec![MotorAddress::new("vcan0", 1), MotorAddress::new("vcan0", 2)]
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
            exit_stop_required("set-zero", 1),
            "failed set-zero must stop"
        );
        assert!(
            !exit_stop_required("set-zero", 0),
            "Davout already stopped and verified"
        );
        for command in ["home", "enable", "jog", "speed", "speed-stop"] {
            assert!(!exit_stop_required(command, 1), "{command}");
        }
    }

    #[test]
    fn signal_exit_codes_follow_the_shell_convention() {
        assert_eq!(signal_exit_code(15), 143);
        assert_eq!(signal_exit_code(2), 130);
    }
}
