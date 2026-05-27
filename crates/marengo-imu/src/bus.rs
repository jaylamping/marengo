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
    fn read_header(&mut self) -> Result<[u8; 4], BusError>;
    fn read_packet_body(&mut self, total_len: usize, out: &mut [u8]) -> Result<(), BusError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Write,
    ReadHeader,
    ReadBody,
}

#[derive(Debug, Clone)]
pub struct MockTransaction {
    pub kind: TransactionKind,
    pub write_data: Vec<u8>,
    pub header_response: [u8; 4],
    pub body_response: Vec<u8>,
}

/// Scriptable I2C mock for unit tests.
#[derive(Debug, Default)]
pub struct MockI2cBus {
    pub transactions: Vec<MockTransaction>,
    pub index: usize,
}

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
        let body = packet[4..].to_vec();
        for _ in 0..2 {
            self.transactions.push(MockTransaction {
                kind: TransactionKind::ReadHeader,
                write_data: Vec::new(),
                header_response: header,
                body_response: body.clone(),
            });
        }
        if packet.len() > 4 {
            self.transactions.push(MockTransaction {
                kind: TransactionKind::ReadBody,
                write_data: Vec::new(),
                header_response: header,
                body_response: body,
            });
        }
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

    fn read_packet_body(&mut self, total_len: usize, out: &mut [u8]) -> Result<(), BusError> {
        if out.len() < total_len {
            return Err(BusError::BufferTooSmall {
                need: total_len,
                have: out.len(),
            });
        }
        let tx = self.next()?;
        if tx.kind != TransactionKind::ReadBody {
            return Err(BusError::Io {
                message: format!("expected read body, got {:?}", tx.kind),
            });
        }
        out[..4].copy_from_slice(&tx.header_response);
        let body_len = total_len.saturating_sub(4);
        if body_len > 0 {
            out[4..4 + body_len].copy_from_slice(&tx.body_response[..body_len]);
        }
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
        let _ = bus.read_header().expect("peek header");
        let header = bus.read_header().expect("header");
        assert_eq!(header[2], 2);
        let mut buf = [0u8; 8];
        bus.read_packet_body(8, &mut buf).expect("body");
        assert_eq!(buf[4], 0xAA);
    }
}
