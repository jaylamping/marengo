use std::collections::HashMap;

use marengo_config::MotorsConfigFile;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MotorAddress {
    interface: String,
    device_id: u8,
}

/// Joint lookup keyed by (can_interface, device_id) for robstride motors.
#[derive(Debug, Clone)]
pub struct MotorCatalog {
    joints: HashMap<MotorAddress, String>,
}

impl MotorCatalog {
    pub(crate) fn lookup(&self, interface: &str, device_id: u8) -> Option<&str> {
        self.joints
            .get(&MotorAddress {
                interface: interface.to_string(),
                device_id,
            })
            .map(String::as_str)
    }
}

impl TryFrom<&MotorsConfigFile> for MotorCatalog {
    type Error = Error;

    /// Includes only driver == "robstride"; rejects duplicate
    /// (can_interface, device_id) addresses and empty joint/interface names.
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
            let address = MotorAddress {
                interface: motor.can_interface.clone(),
                device_id: motor.device_id,
            };
            if joints.contains_key(&address) {
                return Err(Error::InvalidMotorCatalog(format!(
                    "duplicate motor address {}:{}",
                    address.interface, address.device_id
                )));
            }
            joints.insert(address, motor.joint.clone());
        }
        Ok(Self { joints })
    }
}
