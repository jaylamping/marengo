use std::path::Path;

use i2cdev::core::I2CDevice;
use i2cdev::linux::LinuxI2CDevice;

use crate::bus::{BusError, I2cBus};

/// Linux `/dev/i2c-*` backend using `i2cdev`.
pub struct LinuxI2cBus {
    device: LinuxI2CDevice,
}

impl LinuxI2cBus {
    pub fn open(path: impl AsRef<Path>, address: u16) -> Result<Self, BusError> {
        let device = LinuxI2CDevice::new(path.as_ref(), address).map_err(|err| BusError::Io {
            message: err.to_string(),
        })?;
        Ok(Self { device })
    }
}

impl I2cBus for LinuxI2cBus {
    fn write(&mut self, data: &[u8]) -> Result<(), BusError> {
        self.device.write(data).map_err(|err| BusError::Io {
            message: err.to_string(),
        })
    }

    fn read_header(&mut self) -> Result<[u8; 4], BusError> {
        let mut header = [0u8; 4];
        self.device.read(&mut header).map_err(|err| BusError::Io {
            message: err.to_string(),
        })?;
        Ok(header)
    }

    fn read_packet_payload(&mut self, payload_len: usize, out: &mut [u8]) -> Result<(), BusError> {
        if out.len() < 4 + payload_len {
            return Err(BusError::BufferTooSmall {
                need: 4 + payload_len,
                have: out.len(),
            });
        }
        if payload_len == 0 {
            return Ok(());
        }
        self.device
            .read(&mut out[4..4 + payload_len])
            .map_err(|err| BusError::Io {
                message: err.to_string(),
            })
    }
}
