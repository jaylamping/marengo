use std::time::{Duration, Instant};

use tracing::debug;

use crate::bus::{BusError, I2cBus};
use crate::error::ImuError;
use crate::shtp::{
    build_outgoing_packet, build_product_id_request, build_set_feature_report,
    parse_rotation_vector, split_batch_reports, PacketHeader, CHANNEL_CONTROL, CHANNEL_EXE,
    CHANNEL_INPUT_SENSOR_REPORTS, DATA_BUFFER_SIZE, GET_FEATURE_RESPONSE, REPORT_ROTATION_VECTOR,
    SHTP_REPORT_PRODUCT_ID_RESPONSE,
};
use crate::types::{ImuAccuracy, Quaternion, RotationVectorSample};

const SOFT_RESET_PAYLOAD: [u8; 1] = [1];
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
const PRODUCT_ID_TIMEOUT: Duration = Duration::from_secs(5);
const POST_RESET_DELAY: Duration = Duration::from_millis(1000);

/// BNO085 driver over SHTP/I2C.
///
/// `poll` reports only samples that arrived since the previous `poll`: a
/// silent sensor yields `Ok(None)`, never a re-stamped cached sample (CS17).
pub struct Bno085<B: I2cBus> {
    bus: B,
    sequence: [u8; 6],
    enabled_features: Vec<u8>,
    last_rotation: Option<RotationVectorSample>,
    /// A new rotation arrived since the last `poll`.
    pending_rotation: bool,
    /// Monotonic per driver instance, incremented on every new rotation
    /// sample. Publishers expose it so consumers can tell a fresh sample
    /// from a silent sensor.
    sample_seq: u64,
    /// Last inbound SHTP sequence per channel, for gap detection.
    last_rx_seq: [Option<u8>; 6],
    id_verified: bool,
}

impl<B: I2cBus> Bno085<B> {
    pub fn new(bus: B) -> Self {
        Self {
            bus,
            sequence: [0; 6],
            enabled_features: Vec::new(),
            last_rotation: None,
            pending_rotation: false,
            sample_seq: 0,
            last_rx_seq: [None; 6],
            id_verified: false,
        }
    }

    pub fn initialize(&mut self) -> Result<(), ImuError> {
        self.initialize_while(|| true)
    }

    /// `initialize` that consults `keep_going` between attempts and inside
    /// the product-ID wait, so an owner can abort the worst-case ~22 s init
    /// (absent sensor) instead of blocking shutdown on the publisher thread.
    /// A cancelled init reports `Timeout`, like any other init failure, so
    /// the session backoff path handles it.
    pub fn initialize_while(&mut self, keep_going: impl Fn() -> bool) -> Result<(), ImuError> {
        for attempt in 0..3 {
            if !keep_going() {
                return Err(ImuError::Timeout {
                    what: "imu init cancelled".to_string(),
                });
            }
            self.soft_reset()?;
            match self.check_product_id_while(&keep_going) {
                Ok(()) => return Ok(()),
                Err(err) if attempt < 2 => {
                    debug!(error = %err, attempt, "product id check failed, retrying");
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(err) => return Err(err),
            }
        }
        Err(ImuError::Protocol(
            "could not read product id after reset".to_string(),
        ))
    }

    pub fn enable_rotation_vector(&mut self, report_interval_us: u32) -> Result<(), ImuError> {
        self.enable_feature(REPORT_ROTATION_VECTOR, report_interval_us)
    }

    pub fn enable_feature(
        &mut self,
        feature_id: u8,
        report_interval_us: u32,
    ) -> Result<(), ImuError> {
        let payload = build_set_feature_report(feature_id, report_interval_us);
        self.send_packet(CHANNEL_CONTROL, &payload)?;

        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        while Instant::now() < deadline {
            self.process_available_packets(Some(10))?;
            if self.enabled_features.contains(&feature_id) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(ImuError::FeatureNotEnabled { feature_id })
    }

    /// Drain newly arrived packets and report a rotation sample only when one
    /// arrived since the previous `poll`. A silent sensor yields `Ok(None)`;
    /// the cached sample is never re-stamped (CS17). Pair with
    /// [`Self::sample_seq`] to detect silence across polls.
    pub fn poll(&mut self) -> Result<Option<RotationVectorSample>, ImuError> {
        self.process_available_packets(None)?;
        if self.pending_rotation {
            self.pending_rotation = false;
            Ok(self.last_rotation)
        } else {
            Ok(None)
        }
    }

    /// Monotonic count of new rotation samples on this driver instance.
    /// Incremented only when a fresh, valid rotation report arrives.
    pub fn sample_seq(&self) -> u64 {
        self.sample_seq
    }

    pub fn last_rotation(&self) -> Option<RotationVectorSample> {
        self.last_rotation
    }

    fn soft_reset(&mut self) -> Result<(), ImuError> {
        self.sequence = [0; 6];
        self.enabled_features.clear();
        self.last_rotation = None;
        self.pending_rotation = false;
        self.sample_seq = 0;
        self.last_rx_seq = [None; 6];
        self.id_verified = false;
        self.send_packet(CHANNEL_EXE, &SOFT_RESET_PAYLOAD)?;
        std::thread::sleep(POST_RESET_DELAY);
        self.send_packet(CHANNEL_EXE, &SOFT_RESET_PAYLOAD)?;
        std::thread::sleep(POST_RESET_DELAY);
        for _ in 0..10 {
            let _ = self.try_read_packet();
        }
        Ok(())
    }

    fn check_product_id_while(&mut self, keep_going: impl Fn() -> bool) -> Result<(), ImuError> {
        if self.id_verified {
            return Ok(());
        }
        let req = build_product_id_request();
        self.send_packet(CHANNEL_CONTROL, &req)?;

        let deadline = Instant::now() + PRODUCT_ID_TIMEOUT;
        while Instant::now() < deadline {
            if !keep_going() {
                return Err(ImuError::Timeout {
                    what: "imu init cancelled".to_string(),
                });
            }
            self.process_available_packets(Some(20))?;
            if self.id_verified {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(ImuError::Timeout {
            what: "product id response".to_string(),
        })
    }

    fn process_available_packets(&mut self, max: Option<usize>) -> Result<(), ImuError> {
        let mut processed = 0usize;
        loop {
            if let Some(limit) = max {
                if processed >= limit {
                    break;
                }
            }
            match self.try_read_packet()? {
                Some(packet) => {
                    self.handle_packet(packet)?;
                    processed += 1;
                }
                None => break,
            }
        }
        Ok(())
    }

    fn handle_packet(&mut self, packet: ShtpPacket) -> Result<(), ImuError> {
        self.observe_rx_sequence(packet.channel, packet.sequence);
        for report in packet.reports() {
            match report.first().copied() {
                Some(SHTP_REPORT_PRODUCT_ID_RESPONSE) => {
                    self.id_verified = true;
                    debug!("BNO085 product id response received");
                }
                Some(GET_FEATURE_RESPONSE) if report.len() >= 2 => {
                    let feature_id = report[1];
                    if !self.enabled_features.contains(&feature_id) {
                        self.enabled_features.push(feature_id);
                    }
                }
                _ => {}
            }

            if packet.channel != CHANNEL_INPUT_SENSOR_REPORTS {
                continue;
            }
            if report
                .first()
                .is_some_and(|id| crate::shtp::is_meta_report(*id))
            {
                continue;
            }
            if let Some((i, j, k, real, accuracy)) = parse_rotation_vector(report) {
                let Some(quaternion) = Quaternion { i, j, k, real }.normalize_checked() else {
                    debug!("dropping zero-norm rotation report");
                    continue;
                };
                self.last_rotation = Some(RotationVectorSample {
                    quaternion,
                    accuracy: ImuAccuracy::from(accuracy),
                });
                self.pending_rotation = true;
                self.sample_seq = self.sample_seq.wrapping_add(1);
            }
        }
        Ok(())
    }

    fn try_read_packet(&mut self) -> Result<Option<ShtpPacket>, ImuError> {
        let header_bytes = match self.bus.read_header() {
            Ok(bytes) => bytes,
            Err(BusError::NoPacket) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let header = match PacketHeader::parse(&header_bytes) {
            Some(header) if header.data_length > 0 => header,
            _ => return Ok(None),
        };

        let total = header.packet_byte_count as usize;
        if total > DATA_BUFFER_SIZE {
            return Err(ImuError::Protocol(format!(
                "packet too large: {total} bytes"
            )));
        }

        let mut buffer = vec![0u8; total];
        self.bus
            .read_packet(total, &mut buffer)
            .map_err(ImuError::from)?;
        Ok(Some(ShtpPacket {
            channel: buffer[2],
            sequence: buffer[3],
            data: buffer[4..total].to_vec(),
        }))
    }

    /// Track the inbound per-channel SHTP sequence for gap detection.
    /// Gaps and duplicates are diagnostics only (debug log); the driver has
    /// no retransmission path, so it never acts on them.
    fn observe_rx_sequence(&mut self, channel: u8, sequence: u8) {
        let slot = usize::from(channel);
        if slot >= self.last_rx_seq.len() {
            return;
        }
        if let Some(previous) = self.last_rx_seq[slot] {
            let expected = previous.wrapping_add(1);
            if sequence != expected {
                debug!(
                    channel,
                    previous, sequence, "SHTP sequence gap or duplicate on inbound channel"
                );
            }
        }
        self.last_rx_seq[slot] = Some(sequence);
    }

    fn send_packet(&mut self, channel: u8, payload: &[u8]) -> Result<(), ImuError> {
        let channel_index = channel as usize;
        if channel_index >= self.sequence.len() {
            return Err(ImuError::Protocol(format!("invalid channel {channel}")));
        }
        let seq = self.sequence[channel_index];
        let mut out = [0u8; DATA_BUFFER_SIZE];
        let len = build_outgoing_packet(channel, seq, payload, &mut out);
        self.bus.write(&out[..len]).map_err(ImuError::from)?;
        self.sequence[channel_index] = seq.wrapping_add(1);
        Ok(())
    }
}

struct ShtpPacket {
    channel: u8,
    sequence: u8,
    data: Vec<u8>,
}

impl ShtpPacket {
    fn reports(&self) -> Vec<&[u8]> {
        split_batch_reports(&self.data)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::bus::{MockI2cBus, MockTransaction, TransactionKind};
    use crate::shtp::{build_outgoing_packet, GET_FEATURE_RESPONSE, REPORT_ROTATION_VECTOR};

    #[test]
    fn parses_rotation_report_from_input_channel() {
        let mut bus = MockI2cBus::default();
        bus.push_read_packet(&rotation_report_packet(0.5));

        let mut driver = Bno085::new(bus);
        driver.enabled_features.push(REPORT_ROTATION_VECTOR);
        driver.process_available_packets(None).expect("process");
        let sample = driver.last_rotation().expect("rotation");
        assert!((sample.quaternion.i - 1.0).abs() < 1e-3);
        assert_eq!(sample.accuracy, ImuAccuracy::High);
    }

    fn rotation_report_packet(i: f64) -> Vec<u8> {
        let mut report = [0u8; 14];
        report[0] = REPORT_ROTATION_VECTOR;
        report[2] = 3;
        let qi = (i / (1.0 / 16384.0)) as i16;
        report[4..6].copy_from_slice(&qi.to_le_bytes());
        let mut packet = vec![0u8; 4 + 14];
        let len = build_outgoing_packet(CHANNEL_INPUT_SENSOR_REPORTS, 2, &report, &mut packet);
        packet.truncate(len);
        packet
    }

    #[test]
    fn poll_reports_each_sample_once_then_silence() {
        let mut bus = MockI2cBus::default();
        bus.push_read_packet(&rotation_report_packet(0.5));

        let mut driver = Bno085::new(bus);
        driver.enabled_features.push(REPORT_ROTATION_VECTOR);
        let first = driver.poll().expect("poll").expect("fresh sample");
        assert!((first.quaternion.i - 1.0).abs() < 1e-3);
        assert_eq!(driver.sample_seq(), 1);
        // No new packet arrived: silence, not a re-stamped cached sample.
        assert!(driver.poll().expect("poll").is_none());
        assert_eq!(driver.sample_seq(), 1);
    }

    #[test]
    fn zero_norm_rotation_is_dropped_not_published() {
        let mut report = [0u8; 14];
        report[0] = REPORT_ROTATION_VECTOR;
        let mut packet = vec![0u8; 4 + 14];
        let len = build_outgoing_packet(CHANNEL_INPUT_SENSOR_REPORTS, 2, &report, &mut packet);
        packet.truncate(len);

        let mut bus = MockI2cBus::default();
        bus.push_read_packet(&packet);
        let mut driver = Bno085::new(bus);
        driver.enabled_features.push(REPORT_ROTATION_VECTOR);
        assert!(driver.poll().expect("poll").is_none());
        assert!(driver.last_rotation().is_none());
        assert_eq!(driver.sample_seq(), 0);
    }

    #[test]
    fn initialize_while_aborts_when_cancelled() {
        let bus = MockI2cBus::default();
        let mut driver = Bno085::new(bus);
        let err = driver
            .initialize_while(|| false)
            .expect_err("cancelled init must fail");
        assert!(matches!(err, ImuError::Timeout { .. }));
    }

    #[test]
    fn parses_rotation_after_base_timestamp_prefix() {
        use crate::shtp::REPORT_BASE_TIMESTAMP;

        let mut batch = [0u8; 19];
        batch[0] = REPORT_BASE_TIMESTAMP;
        batch[5] = REPORT_ROTATION_VECTOR;
        batch[7] = 3;
        let qi = (0.5 / (1.0 / 16384.0)) as i16;
        batch[9..11].copy_from_slice(&qi.to_le_bytes());
        let mut packet = vec![0u8; 4 + batch.len()];
        let len = build_outgoing_packet(CHANNEL_INPUT_SENSOR_REPORTS, 2, &batch, &mut packet);
        packet.truncate(len);

        let mut bus = MockI2cBus::default();
        bus.push_read_packet(&packet);
        let mut driver = Bno085::new(bus);
        driver.enabled_features.push(REPORT_ROTATION_VECTOR);
        driver.process_available_packets(None).expect("process");
        let sample = driver.last_rotation().expect("rotation");
        assert!((sample.quaternion.i - 1.0).abs() < 1e-3);
    }

    #[test]
    fn marks_feature_enabled_from_control_response() {
        let mut bus = MockI2cBus::default();
        let mut report = [0u8; 17];
        report[0] = GET_FEATURE_RESPONSE;
        report[1] = REPORT_ROTATION_VECTOR;
        let mut packet = vec![0u8; 21];
        let len = build_outgoing_packet(CHANNEL_CONTROL, 1, &report, &mut packet);
        packet.truncate(len);
        bus.push_read_packet(&packet);

        let mut driver = Bno085::new(bus);
        driver.process_available_packets(None).expect("process");
        assert!(driver.enabled_features.contains(&REPORT_ROTATION_VECTOR));
    }

    #[test]
    fn send_packet_increments_sequence() {
        let mut bus = MockI2cBus::default();
        bus.transactions.push(MockTransaction {
            kind: TransactionKind::Write,
            write_data: Vec::new(),
            header_response: [0; 4],
            body_response: Vec::new(),
        });
        let mut driver = Bno085::new(bus);
        driver
            .send_packet(CHANNEL_CONTROL, &[0xFD, REPORT_ROTATION_VECTOR])
            .expect("send");
        assert_eq!(driver.sequence[CHANNEL_CONTROL as usize], 1);
    }
}
