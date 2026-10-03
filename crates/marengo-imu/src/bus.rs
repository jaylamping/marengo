use thiserror::Error;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("i2c io: {message}")]
    Io { message: String },
    #[error("no packet available")]
    NoPacket,
    #[error("buffer too small: need {need}, have {have}")]
    BufferTooSmall { need: usize, have: usize },
}

/// Minimal I2C transaction surface for SHTP (testable without hardware).
pub trait I2cBus {
    fn write(&mut self, data: &[u8]) -> Result<(), BusError>;
    /// Peek the 4-byte SHTP header (`data_length == 0` means no packet).
    fn read_header(&mut self) -> Result<[u8; 4], BusError>;
    /// After `read_header`, read the full packet (`packet_byte_count` bytes) into `out[..total_len]`.
    ///
    /// Matches Adafruit CircuitPython BNO08x I2C: second read is the whole packet, not payload-only.
    fn read_packet(&mut self, total_len: usize, out: &mut [u8]) -> Result<(), BusError>;
}

/// Linux errno values (`errno.h`) seen on BNO085 I2C reads.
const EAGAIN: i32 = 11;
#[cfg(test)]
const ENODEV: i32 = 19;
const EREMOTEIO: i32 = 121;

/// EREMOTEIO (NACK while the hub has nothing queued) and EAGAIN mean "no
/// packet available". Everything else, notably ENODEV ("no such device",
/// sensor unplugged), is a bus error: the session restarts instead of the
/// sensor looking idle-but-present.
#[cfg_attr(not(all(target_os = "linux", feature = "linux-i2c")), allow(dead_code))]
pub(crate) fn errno_is_no_packet(code: i32) -> bool {
    code == EREMOTEIO || code == EAGAIN
}

#[cfg(test)]
mod errno_tests {
    use super::*;

    #[test]
    fn unplugged_sensor_is_an_error_not_idle() {
        assert!(!errno_is_no_packet(ENODEV));
        assert!(errno_is_no_packet(EREMOTEIO));
        assert!(errno_is_no_packet(EAGAIN));
        assert!(!errno_is_no_packet(5)); // EIO
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionKind {
    Write,
    ReadHeader,
    ReadPacket,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct MockTransaction {
    pub kind: TransactionKind,
    pub write_data: Vec<u8>,
    pub header_response: [u8; 4],
    pub body_response: Vec<u8>,
}

/// Scriptable I2C mock for unit tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct MockI2cBus {
    pub transactions: Vec<MockTransaction>,
    pub index: usize,
}

#[cfg(test)]
impl MockI2cBus {
    pub fn push_write_ack(&mut self) {
        self.transactions.push(MockTransaction {
            kind: TransactionKind::Write,
            write_data: Vec::new(),
            header_response: [0; 4],
            body_response: Vec::new(),
        });
    }

    pub fn push_read_packet(&mut self, packet: &[u8]) {
        if packet.len() < 4 {
            return;
        }
        let mut header = [0u8; 4];
        header.copy_from_slice(&packet[..4]);
        self.transactions.push(MockTransaction {
            kind: TransactionKind::ReadHeader,
            write_data: Vec::new(),
            header_response: header,
            body_response: packet.to_vec(),
        });
        self.transactions.push(MockTransaction {
            kind: TransactionKind::ReadPacket,
            write_data: Vec::new(),
            header_response: header,
            body_response: packet.to_vec(),
        });
    }

    fn next(&mut self) -> Result<&MockTransaction, BusError> {
        let tx = self
            .transactions
            .get(self.index)
            .ok_or(BusError::NoPacket)?;
        self.index += 1;
        Ok(tx)
    }
}

#[cfg(test)]
impl I2cBus for MockI2cBus {
    fn write(&mut self, data: &[u8]) -> Result<(), BusError> {
        let tx = self.next()?;
        if tx.kind != TransactionKind::Write {
            return Err(BusError::Io {
                message: format!("expected write, got {:?}", tx.kind),
            });
        }
        if !tx.write_data.is_empty() && tx.write_data != data {
            return Err(BusError::Io {
                message: "mock write payload mismatch".to_string(),
            });
        }
        Ok(())
    }

    fn read_header(&mut self) -> Result<[u8; 4], BusError> {
        let tx = self.next()?;
        if tx.kind != TransactionKind::ReadHeader {
            return Err(BusError::Io {
                message: format!("expected read header, got {:?}", tx.kind),
            });
        }
        Ok(tx.header_response)
    }

    fn read_packet(&mut self, total_len: usize, out: &mut [u8]) -> Result<(), BusError> {
        if out.len() < total_len {
            return Err(BusError::BufferTooSmall {
                need: total_len,
                have: out.len(),
            });
        }
        let tx = self.next()?;
        if tx.kind != TransactionKind::ReadPacket {
            return Err(BusError::Io {
                message: format!("expected read packet, got {:?}", tx.kind),
            });
        }
        if tx.body_response.len() != total_len {
            return Err(BusError::Io {
                message: format!(
                    "mock packet length mismatch: expected {total_len}, got {}",
                    tx.body_response.len()
                ),
            });
        }
        out[..total_len].copy_from_slice(&tx.body_response);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn mock_write_and_read() {
        let mut bus = MockI2cBus::default();
        bus.push_write_ack();
        bus.push_read_packet(&[0x08, 0x00, 2, 1, 0xAA, 0xBB, 0xCC, 0xDD]);

        bus.write(&[1, 2, 3, 4]).expect("write");
        let header = bus.read_header().expect("header");
        assert_eq!(header[2], 2);
        let mut buf = [0u8; 8];
        bus.read_packet(8, &mut buf).expect("packet");
        assert_eq!(buf[4], 0xAA);
    }
}
