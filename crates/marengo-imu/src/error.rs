use thiserror::Error;

use crate::bus::BusError;

#[derive(Debug, Error)]
pub enum ImuError {
    #[error("i2c bus: {0}")]
    Bus(#[from] BusError),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("timeout waiting for {what}")]
    Timeout { what: String },
    #[error("feature {feature_id:#04x} was not enabled")]
    FeatureNotEnabled { feature_id: u8 },
}
