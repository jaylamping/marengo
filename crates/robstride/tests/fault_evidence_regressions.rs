#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::time::Duration;

use marengo_config::MotorType;
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorAddress, MotorBus, MotorState};

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];

fn frame(id: u32, data: [u8; 8]) -> CanFrame {
    CanFrame {
        id,
        data,
        extended: true,
    }
}

fn types() -> HashMap<MotorAddress, MotorType> {
    HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)])
}

// Literal IDs use the documented status bits 16..21; the encoder under test
// does not construct these expectations. Existing public receive APIs compile
// unchanged against the batch03 baseline.
#[test]
fn status_can_id_fault_flags_reach_compatibility_state() {
    for (id, flag) in [
        (0x0201_01fd, 1),
        (0x0202_01fd, 2),
        (0x0204_01fd, 4),
        (0x0208_01fd, 8),
        (0x0210_01fd, 16),
        (0x0220_01fd, 32),
    ] {
        let mut bus = MemoryBus::default();
        bus.rx_queue.push(frame(id, POSE));
        let mut states = HashMap::new();
        bus.recv_all_addressed(&types(), &mut states, Duration::ZERO, Duration::ZERO)
            .expect("status drain");
        assert_eq!(states[&MotorAddress::new("can0", 1)].fault, flag, "{id:#x}");
    }
}

#[test]
fn upper_detailed_fault_bytes_cannot_appear_healthy() {
    for payload in [[0, 0, 1, 0, 0, 0, 0, 0], [0, 0, 0, 0x80, 0, 0, 0, 0]] {
        let mut bus = MemoryBus::default();
        bus.rx_queue.push(frame(0x1500_01fd, payload));
        let mut states = HashMap::new();
        bus.recv_all_addressed(&types(), &mut states, Duration::ZERO, Duration::ZERO)
            .expect("fault drain");
        let state = states[&MotorAddress::new("can0", 1)];
        assert_ne!(state.fault, 0, "nonzero detailed fault payload {payload:?}");
        assert_eq!(state.updated, None, "fault is not pose evidence");
    }
}

#[derive(Default)]
struct PrefixThenFailure {
    delivered: bool,
}

impl CanBus for PrefixThenFailure {
    fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), BusError> {
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<robstride::ReceiveAttempt, BusError> {
        if self.delivered {
            return Err(BusError::Driver("RX failed after delivered frame".into()));
        }
        self.delivered = true;
        Ok(robstride::ReceiveAttempt::Frame(robstride::TimedCanFrame {
            received_at: std::time::Instant::now(),
            received: robstride::ReceivedCanFrame::full_data(
                None,
                frame(0x1500_01fd, [1, 0, 0, 0, 0, 0, 0, 0]),
            ),
        }))
    }
}

impl MotorBus for PrefixThenFailure {}

#[test]
fn delivered_fault_prefix_survives_receive_error_in_compatibility_state() {
    let mut bus = PrefixThenFailure::default();
    let mut states: HashMap<MotorAddress, MotorState> = HashMap::new();
    let error = bus
        .recv_all_addressed(&types(), &mut states, Duration::ZERO, Duration::ZERO)
        .expect_err("terminal receive failure");
    assert!(matches!(error, BusError::Driver(_)));
    let state = states
        .get(&MotorAddress::new("can0", 1))
        .expect("delivered fault must survive terminal receive failure");
    assert_ne!(state.fault, 0);
    assert_eq!(state.updated, None);
}
