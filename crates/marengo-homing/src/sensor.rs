use std::collections::HashMap;

use marengo_config::HomingSensors;
use thiserror::Error;

/// Health of a joint's sensor inputs at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorHealth {
    Unknown,
    Healthy,
    Faulted,
}

/// Instantaneous three-Hall snapshot (logical active states).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorSnapshot {
    pub home: bool,
    pub min_limit: bool,
    pub max_limit: bool,
}

/// Classified sensor pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorPattern {
    MidTravel,
    HomeReference,
    MinLimitEdge,
    MaxLimitEdge,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SensorError {
    #[error("impossible sensor overlap (home={home}, min={min_limit}, max={max_limit})")]
    ImpossibleOverlap {
        home: bool,
        min_limit: bool,
        max_limit: bool,
    },
    #[error("gpio read failed for pin {gpio}: {message}")]
    GpioRead { gpio: u8, message: String },
}

pub trait SensorProvider {
    fn read_gpio(&self, gpio: u8, active_high: bool) -> Result<bool, SensorError>;
}

#[derive(Debug, Clone, Copy)]
pub struct ThreeHallInputs {
    pub home_gpio: u8,
    pub min_gpio: u8,
    pub max_gpio: u8,
    pub active_high: bool,
}

impl ThreeHallInputs {
    pub fn from_config(sensors: &HomingSensors) -> Self {
        Self {
            home_gpio: sensors.home.gpio,
            min_gpio: sensors.min_limit.gpio,
            max_gpio: sensors.max_limit.gpio,
            active_high: sensors.home.active_high,
        }
    }
}

impl SensorProvider for ThreeHallInputs {
    fn read_gpio(&self, gpio: u8, active_high: bool) -> Result<bool, SensorError> {
        let _ = (gpio, active_high);
        Err(SensorError::GpioRead {
            gpio,
            message: "ThreeHallInputs is config-only; use MemorySensorProvider or Pi GPIO".into(),
        })
    }
}

/// In-memory GPIO states for unit tests and simulation.
pub struct MemorySensorProvider {
    states: HashMap<u8, bool>,
}

impl MemorySensorProvider {
    pub fn new(states: HashMap<u8, bool>) -> Self {
        Self { states }
    }
}

impl SensorProvider for MemorySensorProvider {
    fn read_gpio(&self, gpio: u8, active_high: bool) -> Result<bool, SensorError> {
        let raw = self.states.get(&gpio).copied().unwrap_or(false);
        Ok(if active_high { raw } else { !raw })
    }
}

impl MemorySensorProvider {
    pub fn read_three_hall(&self, inputs: &ThreeHallInputs) -> Result<SensorSnapshot, SensorError> {
        read_three_hall(self, inputs)
    }

    pub fn set_gpio(&mut self, gpio: u8, raw_high: bool) {
        self.states.insert(gpio, raw_high);
    }
}

pub fn read_three_hall(
    provider: &impl SensorProvider,
    inputs: &ThreeHallInputs,
) -> Result<SensorSnapshot, SensorError> {
    Ok(SensorSnapshot {
        home: provider.read_gpio(inputs.home_gpio, inputs.active_high)?,
        min_limit: provider.read_gpio(inputs.min_gpio, inputs.active_high)?,
        max_limit: provider.read_gpio(inputs.max_gpio, inputs.active_high)?,
    })
}

pub fn classify_sensor_pattern(
    snap: SensorSnapshot,
    allow_overlap: bool,
) -> Result<SensorPattern, SensorError> {
    let count = u8::from(snap.home) + u8::from(snap.min_limit) + u8::from(snap.max_limit);
    if count > 1 && !allow_overlap {
        return Err(SensorError::ImpossibleOverlap {
            home: snap.home,
            min_limit: snap.min_limit,
            max_limit: snap.max_limit,
        });
    }
    match (snap.home, snap.min_limit, snap.max_limit) {
        (true, false, false) => Ok(SensorPattern::HomeReference),
        (false, true, false) => Ok(SensorPattern::MinLimitEdge),
        (false, false, true) => Ok(SensorPattern::MaxLimitEdge),
        (false, false, false) => Ok(SensorPattern::MidTravel),
        _ if allow_overlap => Ok(SensorPattern::MidTravel),
        (h, m, x) => Err(SensorError::ImpossibleOverlap {
            home: h,
            min_limit: m,
            max_limit: x,
        }),
    }
}

/// Startup sensor health: all configured inputs must be readable; stuck-all-active faults.
pub fn check_sensor_health_at_boot(
    provider: &impl SensorProvider,
    inputs: &ThreeHallInputs,
    allow_overlap: bool,
) -> Result<SensorHealth, SensorError> {
    let snap = read_three_hall(provider, inputs)?;
    classify_sensor_pattern(snap, allow_overlap)?;
    Ok(SensorHealth::Healthy)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn boot_health_ok_when_mid_travel() {
        let provider = MemorySensorProvider::new(HashMap::new());
        let inputs = ThreeHallInputs {
            home_gpio: 1,
            min_gpio: 2,
            max_gpio: 3,
            active_high: true,
        };
        let health = check_sensor_health_at_boot(&provider, &inputs, false).expect("healthy");
        assert_eq!(health, SensorHealth::Healthy);
    }
}
