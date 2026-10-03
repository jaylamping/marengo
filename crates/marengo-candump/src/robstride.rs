use std::collections::HashMap;

use ::robstride::MotorAddress;
use marengo_config::MotorsConfigFile;

use crate::Error;

/// Joint lookup keyed by (can_interface, device_id) for robstride motors.
#[derive(Debug, Clone)]
pub struct MotorCatalog {
    joints: HashMap<MotorAddress, String>,
}

impl MotorCatalog {
    pub(crate) fn lookup(&self, interface: &str, device_id: u8) -> Option<&str> {
        self.joints
            .get(&MotorAddress::new(interface, device_id))
            .map(String::as_str)
    }
}

impl TryFrom<&MotorsConfigFile> for MotorCatalog {
    type Error = Error;

    /// Includes only driver == "robstride" and rejects empty joint/interface
    /// names. Address uniqueness is the caller's contract: `load_motors_config_from`
    /// validates it (`validate_motors_config`), so an unvalidated config with a
    /// duplicate (can_interface, device_id) keeps the last joint.
    fn try_from(config: &MotorsConfigFile) -> Result<Self, Self::Error> {
        let mut joints = HashMap::new();
        for motor in &config.motors {
            if motor.driver != "robstride" {
                continue;
            }
            if motor.joint.trim().is_empty() {
                return Err(Error::InvalidMotorCatalog(
                    "empty joint name in motors.yaml".into(),
                ));
            }
            if motor.can_interface.trim().is_empty() {
                return Err(Error::InvalidMotorCatalog(
                    "empty can_interface in motors.yaml".into(),
                ));
            }
            joints.insert(MotorAddress::from(motor), motor.joint.clone());
        }
        Ok(Self { joints })
    }
}
