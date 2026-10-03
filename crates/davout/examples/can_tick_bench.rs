//! Deterministic CPU benchmark of the Marengo CAN path (autoresearch harness).
//!
//! Two workloads, both without hardware:
//! - `router`: robstride `MotorBus` over a SocketCAN-router-shaped in-memory backend
//!   (two interfaces, five motors each, interface-tagged receive, own-write echo).
//!   Per tick: `mit_control_all_at` for ten motors, each echoed and answered by one
//!   status frame, then `recv_feedback_report` decodes the replies.
//! - `davout`: full `Supervisor` tick on the repo master config (five motors, can0):
//!   status replies → `drain_feedback` → `send_mit_batch`.
//!
//! Prints `METRIC` lines (median ns per tick) and a correctness checksum.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::hint::black_box;
use std::time::{Duration, Instant};

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{MitJointCommand, Supervisor};
use marengo_config::MotorType;
use robstride::{
    AddressedMitCommand, BusError, CanBus, CanFrame, FeedbackEvent, MitCommand, MotorAddress,
    MotorBus, ReceiveAttempt, ReceivedCanFrame, TimedCanFrame,
};

const REPEATS: usize = 15;
const ROUTER_TICKS: usize = 4000;
const DAVOUT_TICKS: usize = 2000;

fn status_frame(device_id: u8, tick: usize) -> CanFrame {
    // OperationStatus (type 2), drive mode Motor (2), host 0xfd.
    let id = (2 << 24) | (2 << 22) | (u32::from(device_id) << 8) | 0xfd;
    let wobble = (tick % 7) as u16;
    let position = 0x7fff_u16 + wobble;
    let velocity = 0x7fff_u16 - wobble;
    let torque = 0x7fff_u16 + (u16::from(device_id) & 3);
    let mut data = [0u8; 8];
    data[0..2].copy_from_slice(&position.to_be_bytes());
    data[2..4].copy_from_slice(&velocity.to_be_bytes());
    data[4..6].copy_from_slice(&torque.to_be_bytes());
    data[6..8].copy_from_slice(&250_u16.to_be_bytes());
    CanFrame {
        id,
        data,
        extended: true,
    }
}

/// Mirrors `SocketCanRouter`: per-interface sockets keyed by name, configured
/// address set, round-robin receive cursor, interface name cloned per frame.
struct RouterBench {
    sockets: HashMap<String, VecDeque<CanFrame>>,
    addresses: HashSet<MotorAddress>,
    interfaces: Vec<String>,
    cursor: usize,
    next_start: usize,
    tick: usize,
    tx_checksum: u64,
}

impl RouterBench {
    fn new(addresses: &[MotorAddress]) -> Self {
        let mut interfaces: Vec<String> = addresses.iter().map(|a| a.interface.clone()).collect();
        interfaces.sort();
        interfaces.dedup();
        Self {
            sockets: interfaces
                .iter()
                .map(|i| (i.clone(), VecDeque::with_capacity(16)))
                .collect(),
            addresses: addresses.iter().cloned().collect(),
            interfaces,
            cursor: 0,
            next_start: 0,
            tick: 0,
            tx_checksum: 0,
        }
    }
}

impl CanBus for RouterBench {
    fn validate_address(&self, address: &MotorAddress) -> Result<(), BusError> {
        if !self.addresses.contains(address) || !self.sockets.contains_key(&address.interface) {
            return Err(BusError::UnknownMotorAddress {
                interface: address.interface.clone(),
                device_id: address.device_id,
            });
        }
        Ok(())
    }

    fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), BusError> {
        Err(BusError::Driver("router requires address".into()))
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        if !self.addresses.contains(address) {
            return Err(BusError::UnknownMotorAddress {
                interface: address.interface.clone(),
                device_id: address.device_id,
            });
        }
        let socket = self.sockets.get_mut(&address.interface).ok_or_else(|| {
            BusError::UnknownMotorAddress {
                interface: address.interface.clone(),
                device_id: address.device_id,
            }
        })?;
        self.tx_checksum = self
            .tx_checksum
            .wrapping_mul(31)
            .wrapping_add(u64::from(frame.id) ^ u64::from_le_bytes(frame.data));
        // SocketCAN echoes the write in wire order, then the drive answers each
        // MIT frame with one status frame.
        socket.push_back(frame.clone());
        socket.push_back(status_frame(address.device_id, self.tick));
        Ok(())
    }

    fn receive_source_count(&self) -> usize {
        self.interfaces.len()
    }

    fn begin_receive(&mut self) {
        self.cursor = self.next_start;
        self.next_start = (self.next_start + 1) % self.interfaces.len();
    }

    fn echoes_transmissions(&self) -> bool {
        true
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let interface = &self.interfaces[self.cursor];
        self.cursor = (self.cursor + 1) % self.interfaces.len();
        let socket = self
            .sockets
            .get_mut(interface)
            .ok_or_else(|| BusError::Driver("missing interface".into()))?;
        Ok(match socket.pop_front() {
            Some(frame) => ReceiveAttempt::Frame(TimedCanFrame {
                received_at: Instant::now(),
                received: ReceivedCanFrame::full_data(Some(interface.clone()), frame),
            }),
            None => ReceiveAttempt::Idle,
        })
    }
}

impl MotorBus for RouterBench {}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn bench_router() -> (f64, u64) {
    let kinds = [
        MotorType::Rs03,
        MotorType::Rs03,
        MotorType::Rs02,
        MotorType::Rs02,
        MotorType::Rs00,
    ];
    let mut addresses = Vec::new();
    let mut types = HashMap::new();
    for interface in ["can0", "can1"] {
        for (index, kind) in kinds.iter().enumerate() {
            let address = MotorAddress::new(interface, index as u8 + 1);
            types.insert(address.clone(), *kind);
            addresses.push(address);
        }
    }
    let mut bus = RouterBench::new(&addresses);
    let mut checksum = 0u64;
    let run = |bus: &mut RouterBench, ticks: usize, checksum: &mut u64| {
        for tick in 0..ticks {
            bus.tick = tick;
            let cmds: Vec<AddressedMitCommand> = addresses
                .iter()
                .zip(kinds.iter().cycle())
                .map(|(address, kind)| AddressedMitCommand {
                    address: address.clone(),
                    command: MitCommand {
                        device_id: address.device_id,
                        motor_type: *kind,
                        position_rad: 0.1 * (tick % 11) as f32,
                        velocity_rad_s: 0.0,
                        kp: 20.0,
                        kd: 1.0,
                        torque_ff_nm: 0.5,
                    },
                })
                .collect();
            bus.mit_control_all_at(black_box(&cmds)).expect("send");
            let report = bus.recv_feedback_report(&types, Duration::ZERO, Duration::ZERO);
            assert!(report.terminal_error.is_none());
            assert_eq!(report.observations.len(), addresses.len());
            for observation in &report.observations {
                if let FeedbackEvent::Status(status) = observation.event {
                    *checksum = checksum
                        .wrapping_mul(131)
                        .wrapping_add(u64::from(status.position_rad.to_bits()))
                        .wrapping_add(u64::from(observation.address.device_id));
                } else {
                    panic!("unexpected event");
                }
            }
        }
    };
    run(&mut bus, 500, &mut checksum); // warm-up
    checksum = 0;
    bus.tx_checksum = 0;
    let mut samples = Vec::with_capacity(REPEATS);
    for _ in 0..REPEATS {
        let started = Instant::now();
        run(&mut bus, ROUTER_TICKS, &mut checksum);
        samples.push(started.elapsed().as_nanos() as f64 / ROUTER_TICKS as f64);
    }
    (median(samples), checksum ^ bus.tx_checksum)
}

fn bench_davout() -> (f64, u64) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
    )
    .expect("repository fixture");
    supervisor.control.control.comm_watchdog_ms = 60_000;
    let motors: Vec<_> = supervisor.motors.motors.clone();
    let joints: Vec<String> = motors.iter().map(|m| m.joint.clone()).collect();
    supervisor.enable_targets(&joints).expect("enable");
    let mut checksum = 0u64;
    let run = |supervisor: &mut Supervisor<SimulationBus>, ticks: usize, checksum: &mut u64| {
        for _ in 0..ticks {
            for motor in &motors {
                // Steady pose: Davout derives velocity from position deltas, and
                // benchmark ticks are far shorter than the 5 ms control period.
                let frame = status_frame(motor.device_id, 0);
                supervisor
                    .bus_mut()
                    .queue_received(ReceivedCanFrame::full_data(
                        Some(motor.can_interface.clone()),
                        frame,
                    ))
                    .expect("queue");
            }
            let frames = supervisor.drain_feedback().expect("drain");
            assert_eq!(frames, motors.len());
            let cmds = motors
                .iter()
                .map(|motor| MitJointCommand {
                    joint: motor.joint.clone(),
                    kp: 0.0,
                    kd: 0.0,
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                })
                .collect();
            supervisor.send_mit_batch(cmds).expect("send");
            for frame in supervisor.bus_mut().frames() {
                *checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(frame.id) ^ u64::from_le_bytes(frame.data));
            }
            supervisor.bus_mut().clear_trace();
        }
    };
    run(&mut supervisor, 200, &mut checksum); // warm-up
    checksum = 0;
    let mut samples = Vec::with_capacity(REPEATS);
    for _ in 0..REPEATS {
        let started = Instant::now();
        run(&mut supervisor, DAVOUT_TICKS, &mut checksum);
        samples.push(started.elapsed().as_nanos() as f64 / DAVOUT_TICKS as f64);
    }
    (median(samples), checksum)
}

fn main() {
    let (router_ns, router_sum) = bench_router();
    let (davout_ns, davout_sum) = bench_davout();
    println!("METRIC can_tick_ns={:.1}", router_ns + davout_ns);
    println!("METRIC router_tick_ns={router_ns:.1}");
    println!("METRIC davout_tick_ns={davout_ns:.1}");
    println!("CHECKSUM router={router_sum:016x} davout={davout_sum:016x}");
}
