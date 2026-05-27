use std::path::Path;

use i2cdev::core::I2CDevice;
use i2cdev::linux::LinuxI2CDevice;

use crate::bus::{BusError, I2cBus};

fn io_is_no_packet(err: &impl std::fmt::Display) -> bool {
    let msg = err.to_string();
    msg.contains("Remote I/O error")
        || msg.contains("No such device")
        || msg.contains("Resource temporarily unavailable")
}

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
        if let Err(err) = self.device.read(&mut header) {
            if io_is_no_packet(&err) {
                return Err(BusError::NoPacket);
            }
            return Err(BusError::Io {
                message: err.to_string(),
            });
        }
        Ok(header)
    }

    fn read_packet(&mut self, total_len: usize, out: &mut [u8]) -> Result<(), BusError> {
        if out.len() < total_len {
            return Err(BusError::BufferTooSmall {
                need: total_len,
                have: out.len(),
            });
        }
        if total_len == 0 {
            return Ok(());
        }
        if let Err(err) = self.device.read(&mut out[..total_len]) {
            if io_is_no_packet(&err) {
                return Err(BusError::NoPacket);
            }
            return Err(BusError::Io {
                message: err.to_string(),
            });
        }
        Ok(())
    }
}
