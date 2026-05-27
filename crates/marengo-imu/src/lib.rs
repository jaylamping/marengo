//! # marengo-imu — BNO085 over I2C (SHTP / SH-2)
//!
//! Linux I2C transport and protocol logic for Hillcrest BNO08x IMUs. Used by
//! [`imu-probe`](../../bins/imu-probe) on the Pi bench and (later) by `marengo-pi`
//! sensor polling.
//!
//! ## Responsibilities
//!
//! - SHTP packet framing, soft reset, product ID check, feature enable.
//! - Parse rotation-vector (and optional accel/gyro) sensor reports.
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

pub use bus::{BusError, I2cBus, MockI2cBus, MockTransaction, TransactionKind};
pub use driver::Bno085;
pub use error::ImuError;
pub use types::{ImuAccuracy, ImuSample, Quaternion, RotationVectorSample};

#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
pub use i2c_linux::LinuxI2cBus;

/// Default 7-bit I2C address when ADR/SA0 is high (commissioned bench board).
pub const DEFAULT_I2C_ADDRESS: u16 = 0x4b;

/// Default report interval for rotation vector (microseconds) — 50 Hz.
pub const DEFAULT_REPORT_INTERVAL_US: u32 = 20_000;
