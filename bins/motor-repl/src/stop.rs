//! Independent drive stop for motor-repl.
//!
//! A stop must not depend on anything that can be broken when it is needed:
//! no Supervisor, no `control.yaml`, no URDF, no calibration history, and no
//! CAN interface other than the one a given drive sits on. The only inputs are
//! the `(interface, device_id)` rows of `motors.yaml` and one Robstride type-4
//! Disable per drive. Every drive gets its own attempt and its own outcome; a
//! failure on one drive or interface never skips another.
//!
//! The outcome is honest about what it proves: `sent` means the kernel accepted
//! the frame for transmission, not that the drive left Run mode.

use std::collections::HashMap;
use std::fmt;

use marengo_config::MotorStopTarget;
use robstride::{BusError, MotorAddress, MotorBus};

/// Result of the Disable attempt for one drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveStop {
    pub address: MotorAddress,
    /// `Err` carries the interface-open or write failure.
    pub result: Result<(), String>,
}

/// Per-drive outcomes of one stop, in the order the addresses were given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopReport {
    pub drives: Vec<DriveStop>,
}

impl StopReport {
    pub fn failed(&self) -> usize {
        self.drives.iter().filter(|d| d.result.is_err()).count()
    }

    pub fn all_sent(&self) -> bool {
        !self.drives.is_empty() && self.failed() == 0
    }
}

impl fmt::Display for StopReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for drive in &self.drives {
            match &drive.result {
                Ok(()) => writeln!(
                    f,
                    "disable {}:{} sent",
                    drive.address.interface, drive.address.device_id
                )?,
                Err(error) => writeln!(
                    f,
                    "disable {}:{} FAILED: {error}",
                    drive.address.interface, drive.address.device_id
                )?,
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

/// Send one Disable to every address. `open` is called once per distinct
/// interface; an interface that cannot be opened fails only its own drives.
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
        let result = match bus {
            Ok(bus) => bus
                .disable_drive_at(address)
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.clone()),
        };
        drives.push(DriveStop {
            address: address.clone(),
            result,
        });
    }
    StopReport { drives }
}

/// [`disable_drives`] over the real SocketCAN interfaces.
pub fn disable_drives_socketcan(addresses: &[MotorAddress]) -> StopReport {
    disable_drives(addresses, robstride::RuntimeBus::socketcan)
}

/// Commands that can leave a drive enabled or moving when the process ends, and
/// so arm the exit stop before they run. `home`, `status`, `gravity-*` and
/// `torque-cmd` never address a drive in this one-shot process.
pub fn arms_exit_stop(command: &str) -> bool {
    matches!(
        command,
        "enable" | "jog" | "speed" | "speed-stop" | "set-zero"
    )
}

/// Whether the exit stop must run for `command` ending with `exit_code`.
/// `set-zero` stops the drives itself on success (Davout's transaction
/// cleanup); every other arming command is one-shot, so a drive it left
/// enabled would keep its last MIT frame with no host behind it.
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

    fn assert_is_disable_to(frame: &CanFrame, device_id: u8) {
        let id = unpack_ext_id(frame.id).expect("extended id");
        assert_eq!(id.comm_type, CommunicationType::Disable.as_u8());
        assert_eq!(id.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(id.device_id, device_id);
        assert_eq!(frame.data, [0; 8]);
        assert!(frame.extended);
    }

    #[test]
    fn every_configured_address_gets_exactly_one_type_4_disable() {
        let addresses = stop_addresses(
            &[target("can0", 1), target("can0", 2), target("can0", 5)],
            None,
        );
        let log = Rc::new(RefCell::new(MemoryBus::default()));
        let opened = RefCell::new(Vec::new());
        let report = disable_drives(&addresses, |interface| {
            opened.borrow_mut().push(interface.to_string());
            Ok(SharedBus(Rc::clone(&log)))
        });
        assert!(report.all_sent());
        assert_eq!(report.drives.len(), 3);
        assert_eq!(opened.into_inner(), vec!["can0"], "one open per interface");
        let log = log.borrow();
        assert_eq!(log.tx.len(), 3);
        for (frame, id) in log.tx.iter().zip([1, 2, 5]) {
            assert_is_disable_to(frame, id);
        }
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
        assert!(by_id(1).result.is_ok());
        assert!(by_id(3).result.is_ok());
        let error = by_id(2).result.clone().expect_err("can1 unopened");
        assert!(error.contains("No such device"), "{error}");
        let log = log.borrow();
        assert_eq!(log.tx.len(), 2, "can0 drives still stopped");
        assert_is_disable_to(&log.tx[0], 1);
        assert_is_disable_to(&log.tx[1], 3);
    }

    #[test]
    fn a_failed_write_is_reported_per_drive_and_does_not_stop_the_rest() {
        let addresses = stop_addresses(&[target("can0", 1), target("can0", 2)], None);
        let report = disable_drives(&addresses, |_| Ok(DeadBus));
        assert_eq!(report.failed(), 2);
        let text = report.to_string();
        assert!(
            text.contains("disable can0:1 FAILED: CAN send failed"),
            "{text}"
        );
        assert!(text.contains("disable can0:2 FAILED"), "{text}");
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
            "disable can0:1 sent\ndisable can1:9 FAILED: open: driver error: down\n"
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
    fn exit_stop_arms_for_drive_touching_commands_only() {
        for command in ["enable", "jog", "speed", "speed-stop", "set-zero"] {
            assert!(arms_exit_stop(command), "{command}");
        }
        for command in [
            "status",
            "homing-status",
            "home",
            "disable",
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
        for command in ["enable", "jog", "speed", "speed-stop"] {
            assert!(exit_stop_required(command, 0), "{command} success");
            assert!(exit_stop_required(command, 1), "{command} failure");
        }
        assert!(!exit_stop_required("status", 1));
    }

    #[test]
    fn signal_exit_codes_follow_the_shell_convention() {
        assert_eq!(signal_exit_code(15), 143);
        assert_eq!(signal_exit_code(2), 130);
    }
}
