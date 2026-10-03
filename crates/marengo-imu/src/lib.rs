//! # marengo-imu — BNO085 over I2C (SHTP / SH-2)
//!
//! Linux I2C transport and protocol logic for Hillcrest BNO08x IMUs. Used by
//! [`imu-probe`](../../bins/imu-probe) on the Pi bench and by `marengo-pi`,
//! which publishes `sensors/imu/torso`.
//!
//! ## Responsibilities
//!
//! - SHTP packet framing, soft reset, product ID check, feature enable.
//! - Parse rotation-vector sensor reports; `Bno085::poll` yields only samples
//!   that arrived since the previous poll (a silent sensor yields `None`).
//!
//! ## Does not
//!
//! - Control motors or publish Chappe topics (bins / `marengo-pi`).
//! - Perform gravity compensation or state estimation (Berthier / future estimator).

mod bus;
mod driver;
mod error;
mod shtp;
mod types;

#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
mod i2c_linux;

pub use bus::{BusError, I2cBus};
pub use driver::Bno085;
pub use error::ImuError;
pub use types::{ImuAccuracy, Quaternion, RotationVectorSample};

#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
pub use i2c_linux::LinuxI2cBus;

/// Default 7-bit I2C address when ADR/SA0 is high (commissioned bench board).
pub const DEFAULT_I2C_ADDRESS: u16 = 0x4b;

/// Default report interval for rotation vector (microseconds) — 50 Hz.
pub const DEFAULT_REPORT_INTERVAL_US: u32 = 20_000;

/// Parse a 7-bit I2C address. I2C addresses are hexadecimal by convention
/// (`i2cdetect`, the Pi env `MARENGO_IMU_ADDRESS=4b`): the text is always hex,
/// with or without a `0x` prefix, so `75` means 0x75. Values above 0x7F and
/// non-hex text are refused; callers must report the refusal, never fall
/// back silently to a default address.
pub fn parse_i2c_address(raw: &str) -> Option<u16> {
    let text = raw.trim();
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    let value = u16::from_str_radix(hex, 16).ok()?;
    (value <= 0x7F).then_some(value)
}

#[cfg(test)]
mod address_tests {
    use super::parse_i2c_address;

    #[test]
    fn addresses_are_hex_with_or_without_prefix() {
        assert_eq!(parse_i2c_address("4b"), Some(0x4B));
        assert_eq!(parse_i2c_address("0x4b"), Some(0x4B));
        assert_eq!(parse_i2c_address("0X4B"), Some(0x4B));
        assert_eq!(parse_i2c_address(" 4a "), Some(0x4A));
    }

    #[test]
    fn non_seven_bit_and_garbage_are_refused() {
        assert_eq!(parse_i2c_address("0x80"), None);
        assert_eq!(parse_i2c_address("200"), None);
        assert_eq!(parse_i2c_address("zz"), None);
        assert_eq!(parse_i2c_address(""), None);
        assert_eq!(parse_i2c_address("0x"), None);
    }
}
