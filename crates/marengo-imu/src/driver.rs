use std::time::{Duration, Instant};

use tracing::debug;

use crate::bus::{BusError, I2cBus};
use crate::error::ImuError;
use crate::shtp::{
    build_outgoing_packet, build_product_id_request, build_set_feature_report, parse_accel,
    parse_gyro, parse_rotation_vector, split_batch_reports, PacketHeader, CHANNEL_CONTROL,
    CHANNEL_EXE, CHANNEL_INPUT_SENSOR_REPORTS, DATA_BUFFER_SIZE, GET_FEATURE_RESPONSE,
    REPORT_ROTATION_VECTOR, SHTP_REPORT_PRODUCT_ID_RESPONSE,
};
use crate::types::{ImuAccuracy, Quaternion, RotationVectorSample};

const SOFT_RESET_PAYLOAD: [u8; 1] = [1];
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// BNO085 driver over SHTP/I2C.
pub struct Bno085<B: I2cBus> {
    bus: B,
    sequence: [u8; 6],
    enabled_features: Vec<u8>,
    last_rotation: Option<RotationVectorSample>,
    last_accel: Option<[f64; 3]>,
    last_gyro: Option<[f64; 3]>,
    id_verified: bool,
}

impl<B: I2cBus> Bno085<B> {
    pub fn new(bus: B) -> Self {
        Self {
            bus,
            sequence: [0; 6],
            enabled_features: Vec::new(),
            last_rotation: None,
            last_accel: None,
            last_gyro: None,
            id_verified: false,
        }
    }

    pub fn initialize(&mut self) -> Result<(), ImuError> {
        for attempt in 0..3 {
            self.soft_reset()?;
            match self.check_product_id() {
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

    pub fn poll(&mut self) -> Result<Option<RotationVectorSample>, ImuError> {
        self.process_available_packets(None)?;
        Ok(self.last_rotation)
    }

    pub fn wait_rotation_vector(
        &mut self,
        timeout: Duration,
    ) -> Result<RotationVectorSample, ImuError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.process_available_packets(None)?;
            if let Some(sample) = self.last_rotation {
                return Ok(sample);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(ImuError::Timeout {
            what: "rotation vector report".to_string(),
        })
    }

    pub fn last_rotation(&self) -> Option<RotationVectorSample> {
        self.last_rotation
    }

    fn soft_reset(&mut self) -> Result<(), ImuError> {
        self.sequence = [0; 6];
        self.enabled_features.clear();
        self.id_verified = false;
        self.send_packet(CHANNEL_EXE, &SOFT_RESET_PAYLOAD)?;
        std::thread::sleep(Duration::from_millis(500));
        self.send_packet(CHANNEL_EXE, &SOFT_RESET_PAYLOAD)?;
        std::thread::sleep(Duration::from_millis(500));
        for _ in 0..3 {
            let _ = self.try_read_packet();
        }
        Ok(())
    }

    fn check_product_id(&mut self) -> Result<(), ImuError> {
        if self.id_verified {
            return Ok(());
        }
        let req = build_product_id_request();
        self.send_packet(CHANNEL_CONTROL, &req)?;

        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(packet) = self.try_read_packet()? {
                if packet.channel != CHANNEL_CONTROL {
                    self.handle_packet(packet)?;
                    continue;
                }
                for report in packet.reports()? {
                    if report.first() == Some(&SHTP_REPORT_PRODUCT_ID_RESPONSE) {
                        self.id_verified = true;
                        debug!("BNO085 product id response received");
                        return Ok(());
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(ImuError::Timeout {
            what: "product id response".to_string(),
        })
    }

    fn process_available_packets(&mut self, max: Option<usize>) -> Result<(), ImuError> {
        let mut processed = 0usize;
        while self.data_ready()? {
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
        if packet.channel == CHANNEL_CONTROL {
            for report in packet.reports()? {
                if report.first() == Some(&GET_FEATURE_RESPONSE) && report.len() >= 6 {
                    let feature_id = report[5];
                    if !self.enabled_features.contains(&feature_id) {
                        self.enabled_features.push(feature_id);
                    }
                }
            }
        }

        if packet.channel != CHANNEL_INPUT_SENSOR_REPORTS {
            return Ok(());
        }

        for report in packet.reports()? {
            if let Some((i, j, k, real, accuracy)) = parse_rotation_vector(report) {
                self.last_rotation = Some(RotationVectorSample {
                    quaternion: Quaternion { i, j, k, real }.normalize(),
                    accuracy: ImuAccuracy::from(accuracy),
                });
            }
            if let Some(accel) = parse_accel(report) {
                self.last_accel = Some(accel);
            }
            if let Some(gyro) = parse_gyro(report) {
                self.last_gyro = Some(gyro);
            }
        }
        Ok(())
    }

    fn data_ready(&mut self) -> Result<bool, ImuError> {
        match self.bus.read_header() {
            Ok(header) => Ok(PacketHeader::parse(&header).is_some()),
            Err(BusError::NoPacket) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn try_read_packet(&mut self) -> Result<Option<ShtpPacket>, ImuError> {
        let header_bytes = match self.bus.read_header() {
            Ok(bytes) => bytes,
            Err(BusError::NoPacket) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let header = match PacketHeader::parse(&header_bytes) {
            Some(header) => header,
            None => return Ok(None),
        };

        let total = header.packet_byte_count as usize;
        if total > DATA_BUFFER_SIZE {
            return Err(ImuError::Protocol(format!(
                "packet too large: {total} bytes"
            )));
        }

        let mut buffer = vec![0u8; total];
        self.bus
            .read_packet_body(total, &mut buffer)
            .map_err(ImuError::from)?;
        Ok(Some(ShtpPacket {
            channel: header.channel,
            data: buffer[4..4 + header.data_length].to_vec(),
        }))
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
    data: Vec<u8>,
}

impl ShtpPacket {
    fn reports(&self) -> Result<Vec<&[u8]>, ImuError> {
        split_batch_reports(&self.data).map_err(ImuError::Protocol)
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
    fn marks_feature_enabled_from_control_response() {
        let mut bus = MockI2cBus::default();
        let mut report = [0u8; 17];
        report[0] = GET_FEATURE_RESPONSE;
        report[5] = REPORT_ROTATION_VECTOR;
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
