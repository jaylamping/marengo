//! Validation shared by motor command and firmware numeric encoders.

use thiserror::Error;

use crate::params::{ParameterId, ParameterKind};

/// Numeric input identified in a rejected driver command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandField {
    Position,
    Velocity,
    ProportionalGain,
    DampingGain,
    TorqueFeedforward,
    FirmwareParameter(ParameterId),
}

/// Invalid numeric data must be rejected before quantization or transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommandError {
    #[error("nonfinite {field:?} for motor {device_id}")]
    NonFinite { device_id: u8, field: CommandField },
    #[error("negative {field:?} for motor {device_id}")]
    NegativeGain { device_id: u8, field: CommandField },
    #[error("negative {field:?} for motor {device_id}")]
    NegativeValue { device_id: u8, field: CommandField },
    #[error("parameter {parameter:?} for motor {device_id} requires {expected:?}, got {actual:?}")]
    ParameterType {
        device_id: u8,
        parameter: ParameterId,
        expected: ParameterKind,
        actual: ParameterKind,
    },
    #[error("unsupported run mode {value} for motor {device_id}")]
    UnsupportedRunMode { device_id: u8, value: u8 },
    #[error("parameter {parameter:?} for motor {device_id} is read-only")]
    ReadOnlyParameter {
        device_id: u8,
        parameter: ParameterId,
    },
    #[error("{field:?} is outside the supported range for motor {device_id}")]
    OutOfRange { device_id: u8, field: CommandField },
    #[error("firmware parameter {parameter:?} for motor {device_id} is forbidden to write")]
    ForbiddenParameterWrite {
        device_id: u8,
        parameter: ParameterId,
    },
    #[error("communication type {comm_type} is forbidden to send")]
    ForbiddenCommunicationType { comm_type: u8 },
}

pub(crate) fn bounded(
    device_id: u8,
    field: CommandField,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), CommandError> {
    finite(device_id, field, value)?;
    if !(minimum..=maximum).contains(&value) {
        return Err(CommandError::OutOfRange { device_id, field });
    }
    Ok(())
}

pub(crate) fn finite(device_id: u8, field: CommandField, value: f32) -> Result<(), CommandError> {
    if !value.is_finite() {
        return Err(CommandError::NonFinite { device_id, field });
    }
    Ok(())
}

pub(crate) fn nonnegative_gain(
    device_id: u8,
    field: CommandField,
    value: f32,
) -> Result<(), CommandError> {
    finite(device_id, field, value)?;
    if value < 0.0 {
        return Err(CommandError::NegativeGain { device_id, field });
    }
    Ok(())
}

pub(crate) fn nonnegative(
    device_id: u8,
    field: CommandField,
    value: f32,
) -> Result<(), CommandError> {
    finite(device_id, field, value)?;
    if value < 0.0 {
        return Err(CommandError::NegativeValue { device_id, field });
    }
    Ok(())
}
