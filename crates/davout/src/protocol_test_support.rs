//! Shared raw-response fixture; unavailable in production builds.
#![allow(clippy::expect_used)]
use robstride::{
    CanBus, CanFrame, MotorBus, ParameterId, ReceiveAttempt, ReceivedCanFrame, TimedCanFrame,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub(crate) fn qualified_identity(device: u8) -> [u8; 8] {
    match device {
        1 => [0x45, 0x7B, 0x30, 2, 0x0C, 0x32, 0x38, 0x17],
        2 => [0x78, 0x56, 0x30, 2, 0x0C, 0x34, 0x37, 1],
        3 => [0x9D, 0x6A, 0x3B, 0x85, 0x9C, 2, 0x30, 0x19],
        4 => [0x8A, 6, 0x3B, 0x8F, 0x24, 0x50, 0xB0, 0x0D],
        5 => [0x62, 0x2C, 0x30, 2, 0x0C, 0x34, 0x37, 1],
        _ => [0; 8],
    }
}

#[derive(Default)]
pub(crate) struct ProbeBus {
    pub(crate) tx: Vec<CanFrame>,
    pub(crate) enabled: HashSet<u8>,
    pub(crate) reset_timeout_after_enable: bool,
    pub(crate) drift_after_enable: bool,
    pub(crate) transient_drift: bool,
    pub(crate) transient_enable_drift: bool,
    pub(crate) stationary_roll_velocity_spike: bool,
    pub(crate) lower_yaw_velocity_spike: bool,
    pub(crate) suppress_active_params: bool,
    pub(crate) suppress_active_pose: bool,
    pub(crate) active_param_delay: Duration,
    pub(crate) delayed: VecDeque<(Instant, TimedCanFrame)>,
    pub(crate) wrong_identity: bool,
    pub(crate) rx: VecDeque<TimedCanFrame>,
    pub(crate) inject_peer_fault: bool,
    pub(crate) wrong_host: bool,
    pub(crate) fail_initial_stop: bool,
    pub(crate) fail_query: bool,
    pub(crate) conflicting_mode: bool,
    pub(crate) late_conflict: bool,
    pub(crate) deferred: Option<TimedCanFrame>,
    pub(crate) deferred_armed: bool,
    pub(crate) timeout_by_device: HashMap<u8, u32>,
    pub(crate) ignore_timeout_write: bool,
    pub(crate) bad_zero: bool,
    pub(crate) inject_run_before_zero: bool,
}

impl CanBus for ProbeBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), robstride::BusError> {
        self.tx.push(frame.clone());
        if self.fail_initial_stop && (frame.id >> 24) & 0x1F == 18 {
            self.fail_initial_stop = false;
            return Err(robstride::BusError::Send {
                message: "initial stop fixture".into(),
            });
        }
        if self.fail_query && frame.data[1] == 0xC4 {
            return Err(robstride::BusError::Send {
                message: "query fixture".into(),
            });
        }
        let id = robstride::unpack_ext_id(frame.id).expect("query ID");
        if id.comm_type == 18
            && frame.data[..2] == ParameterId::CanTimeout.as_u16().to_le_bytes()
            && !self.ignore_timeout_write
        {
            self.timeout_by_device.insert(
                id.device_id,
                u32::from_le_bytes([frame.data[4], frame.data[5], frame.data[6], frame.data[7]]),
            );
        }
        if id.comm_type == 3 {
            self.enabled.insert(id.device_id);
            if self.reset_timeout_after_enable {
                self.timeout_by_device.insert(id.device_id, 0);
            }
        }
        if id.comm_type == 4 && frame.data[1] != 0xC4 {
            self.enabled.remove(&id.device_id);
        }
        let host = if self.wrong_host {
            0xFF
        } else {
            id.extra_data as u8
        };
        let response = match id.comm_type {
            1 | 3
                if self.enabled.contains(&id.device_id)
                    && !self.suppress_active_pose
                    && (id.comm_type == 1 || self.transient_enable_drift) =>
            {
                Some(CanFrame {
                    id: robstride::pack_ext_id(
                        2,
                        u16::from(id.device_id)
                            | if self.enabled.contains(&id.device_id) {
                                2 << 14
                            } else {
                                0
                            },
                        0xFD,
                    ),
                    data: [
                        if self.drift_after_enable
                            || self.transient_drift
                            || self.transient_enable_drift
                        {
                            0x81
                        } else {
                            0x7F
                        },
                        0xFF,
                        0x7F,
                        0xFF,
                        0x7F,
                        0xFF,
                        0,
                        0xC8,
                    ],
                    extended: true,
                })
            }
            0 => Some(CanFrame {
                id: robstride::pack_ext_id(0, u16::from(id.device_id), 0xFE),
                data: if self.wrong_identity {
                    [0xFF; 8]
                } else {
                    qualified_identity(id.device_id)
                },
                extended: true,
            }),
            4 if frame.data[1] == 0xC4 => Some(CanFrame {
                id: robstride::pack_ext_id(2, u16::from(id.device_id), host),
                data: match id.device_id {
                    1 | 2 => [0, 0xC4, 0x56, 0, 3, 1, 42, 0],
                    3 | 4 => [0, 0xC4, 0x56, 0, 2, 3, 34, 0],
                    _ => [0, 0xC4, 0x56, 0, 0, 3, 32, 0],
                },
                extended: true,
            }),
            6 => Some(CanFrame {
                id: robstride::pack_ext_id(2, u16::from(id.device_id), host),
                data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                extended: true,
            }),
            17 if !self.suppress_active_params || self.enabled.is_empty() => Some(CanFrame {
                id: robstride::pack_ext_id(17, u16::from(id.device_id), host),
                data: {
                    let index = u16::from_le_bytes([frame.data[0], frame.data[1]]);
                    let value = if index == ParameterId::CanTimeout.as_u16() {
                        self.timeout_by_device
                            .get(&id.device_id)
                            .copied()
                            .unwrap_or(0)
                            .to_le_bytes()
                    } else if index == ParameterId::MechanicalPosition.as_u16() && self.bad_zero {
                        1.0_f32.to_le_bytes()
                    } else {
                        [0; 4]
                    };
                    [
                        frame.data[0],
                        frame.data[1],
                        0,
                        0,
                        value[0],
                        value[1],
                        value[2],
                        value[3],
                    ]
                },
                extended: true,
            }),
            _ => None,
        };
        if let Some(response) = response {
            let mut response = response;
            if id.comm_type == 1
                && ((self.stationary_roll_velocity_spike && id.device_id == 2)
                    || (self.lower_yaw_velocity_spike && id.device_id == 5))
            {
                // Actual fault-free stationary Run frame from the first-motion capture.
                response.data = [0x7F, 0xFF, 0x80, 0xF8, 0x80, 0x1D, 0, 0xF0];
            }
            let is_version = frame.data[1] == 0xC4;
            let mut conflict = response.clone();
            conflict.id |= 2 << 22;
            let received = TimedCanFrame {
                received_at: Instant::now(),
                received: ReceivedCanFrame::full_data(Some("can0".into()), response.clone()),
            };
            if id.comm_type == 17 && !self.enabled.is_empty() && !self.active_param_delay.is_zero()
            {
                self.delayed
                    .push_back((Instant::now() + self.active_param_delay, received));
            } else {
                self.rx.push_back(received);
            }
            if (id.comm_type == 1 && self.transient_drift)
                || (id.comm_type == 3 && self.transient_enable_drift)
            {
                self.transient_drift = false;
                self.transient_enable_drift = false;
                let mut healthy = response;
                healthy.data[0] = 0x7F;
                self.rx.push_back(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(Some("can0".into()), healthy),
                });
            }
            if is_version && self.conflicting_mode {
                self.rx.push_back(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(Some("can0".into()), conflict.clone()),
                });
            }
            if is_version && self.late_conflict {
                self.late_conflict = false;
                self.deferred = Some(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(Some("can0".into()), conflict),
                });
            }
            if self.inject_run_before_zero && id.comm_type == 17 && host == 0xB4 {
                self.inject_run_before_zero = false;
                self.deferred = Some(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(
                        Some("can0".into()),
                        CanFrame {
                            id: 0x0280_01FD,
                            data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                            extended: true,
                        },
                    ),
                });
            }
            if self.inject_peer_fault {
                self.inject_peer_fault = false;
                self.rx.push_back(TimedCanFrame {
                    received_at: Instant::now(),
                    received: ReceivedCanFrame::full_data(
                        Some("can0".into()),
                        CanFrame {
                            id: 0x0201_02FD,
                            data: [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xC8],
                            extended: true,
                        },
                    ),
                });
            }
        }
        Ok(())
    }
    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, robstride::BusError> {
        if self
            .delayed
            .front()
            .is_some_and(|(due, _)| Instant::now() >= *due)
        {
            if let Some((_, mut received)) = self.delayed.pop_front() {
                received.received_at = Instant::now();
                self.rx.push_back(received);
            }
        }
        if self.rx.is_empty() && self.deferred.is_some() {
            if self.deferred_armed {
                return Ok(self
                    .deferred
                    .take()
                    .map(ReceiveAttempt::Frame)
                    .unwrap_or(ReceiveAttempt::Idle));
            }
            self.deferred_armed = true;
            return Ok(ReceiveAttempt::Idle);
        }
        Ok(self
            .rx
            .pop_front()
            .map(ReceiveAttempt::Frame)
            .unwrap_or(ReceiveAttempt::Idle))
    }
}
impl MotorBus for ProbeBus {}
