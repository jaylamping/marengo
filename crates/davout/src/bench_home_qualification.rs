//! Supported mechanical-home qualification, without enable or reference selection.

use marengo_config::MotorType;
use robstride::protocol::{ProtocolObservation, ProtocolReply};
use robstride::{MotorAddress, MotorBus, ParameterId, ParameterValue};
use serde::Serialize;

use super::protocol_inspection::Query;
use super::{DavoutError, MotorProtocolInspection, ProtocolReadReceipt, Supervisor};

pub const BENCH_TIMEOUT_COUNTS: u32 = 600;

#[derive(Debug, Serialize)]
pub struct BenchHomeQualification {
    pub schema: u32,
    pub operator: String,
    pub mechanical_home_confirmed: bool,
    pub sign_attested: bool,
    pub grants_reference: bool,
    pub timeout_counts: u32,
    pub before: Vec<MotorProtocolInspection>,
    pub reads: Vec<ProtocolReadReceipt>,
}

impl<B: MotorBus> Supervisor<B> {
    /// Guarded physical-protocol qualification only. Every drive remains Disabled.
    ///
    /// Caller confirmation is about supported mechanical home and the installed
    /// sign mapping. Success is audit evidence, never Ready or motion permission.
    /// Blocking standalone CLI operation; do not call from a control tick.
    pub fn qualify_bench_home_disabled(
        &mut self,
        confirmed: bool,
        sign_attested: bool,
        operator: &str,
    ) -> Result<BenchHomeQualification, DavoutError> {
        self.qualify_bench_home_for_profile(confirmed, sign_attested, operator, None)
    }

    pub(super) fn qualify_bench_home_for_profile(
        &mut self,
        confirmed: bool,
        sign_attested: bool,
        operator: &str,
        identities: Option<&[[u8; 8]; 5]>,
    ) -> Result<BenchHomeQualification, DavoutError> {
        self.refuse_reference_interference("disabled bench home qualification")?;
        self.require_fault_clear()?;
        if !confirmed || !sign_attested || operator.trim().is_empty() || operator.len() > 128 {
            return Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: "supported home, sign attestation and operator are required".into(),
            });
        }
        if self.motors.motors.len() != 5 {
            return Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: "qualification requires the installed five-joint arm".into(),
            });
        }
        for motor in &self.motors.motors {
            self.reference_request_preflight(&motor.joint, Some(sign_attested))?;
            let policy = self
                .homing_config
                .homing
                .effective_joint(&motor.joint)
                .ok_or_else(|| DavoutError::Homing {
                    message: "missing home policy".into(),
                })?;
            if policy.home_offset_rad != 0.0 || policy.sensors.is_some() {
                return Err(DavoutError::ProtocolInspection {
                    joint: motor.joint.clone(),
                    message: "only zero-offset manual home is supported".into(),
                });
            }
        }
        let before = self.inspect_drive_protocol()?;
        if identities.is_some_and(|expected| {
            before
                .iter()
                .zip(expected)
                .any(|(observed, expected)| observed.identity_wire_bytes != *expected)
        }) {
            return Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: "MCU identity does not match qualified physical arm".into(),
            });
        }
        // Select only observed installed model/firmware pairs. Configuration metadata
        // alone cannot establish these external facts.
        for (motor, observed) in self.motors.motors.iter().zip(&before) {
            let supported = matches!(
                (motor.motor_type, observed.firmware_version),
                (MotorType::Rs03, [0, 3, 1, 42])
                    | (MotorType::Rs02, [0, 2, 3, 34])
                    | (MotorType::Rs00, [0, 0, 3, 32])
            );
            let mit_mode = observed
                .reads
                .iter()
                .find(|read| read.query == "RunMode")
                .is_some_and(|read| read.response_data[4..8] == [0, 0, 0, 0]);
            if !supported || !mit_mode {
                return Err(DavoutError::ProtocolInspection {
                    joint: motor.joint.clone(),
                    message: "unsupported observed firmware or non-MIT mode".into(),
                });
            }
        }
        // Changing the physical coordinate invalidates every previous current grant
        // and retained stage before any write, including an uncertain Set Zero.
        self.reference_authority
            .invalidate_for_acquisition()
            .ok_or_else(|| DavoutError::Homing {
                message: "reference generation exhausted".into(),
            })?;
        let result = self.qualify_bench_home_inner(before, operator);
        if let Err(error) = &result {
            self.record_runtime_error(error);
        }
        let cleanup = self.inspection_stop();
        match (result, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (Err(error), Ok(())) => Err(error),
            (result, Err(error)) => Err(DavoutError::ProtocolInspection {
                joint: String::new(),
                message: format!(
                    "qualification {:?}; final stop failed: {error}",
                    result.err()
                ),
            }),
        }
    }

    fn qualify_bench_home_inner(
        &mut self,
        before: Vec<MotorProtocolInspection>,
        operator: &str,
    ) -> Result<BenchHomeQualification, DavoutError> {
        let motors = self.motors.motors.clone();
        let mut accepted: Vec<ProtocolObservation> = Vec::with_capacity(20);
        let mut reads = Vec::with_capacity(20);
        for (i, motor) in motors.iter().enumerate() {
            let address = MotorAddress::from(motor);
            self.bus.write_parameter_at(
                &address,
                ParameterId::CanTimeout,
                ParameterValue::U32(BENCH_TIMEOUT_COUNTS),
            )?;
            let (reply, receipt) = self.inspect_protocol_query(
                &motor.joint,
                &address,
                Query::Parameter(ParameterId::CanTimeout),
                0xB0 + i as u8,
                &accepted,
            )?;
            require_timeout(&motor.joint, &reply)?;
            accepted.push(reply);
            reads.push(receipt);
        }
        for (i, motor) in motors.iter().enumerate() {
            let address = MotorAddress::from(motor);
            self.invalidate_reference_target_pose_for_zero_attempt(motor);
            let (reply, receipt) = self.inspect_protocol_query(
                &motor.joint,
                &address,
                Query::SetZero,
                0xD0 + i as u8,
                &accepted,
            )?;
            // The whole report already reached fault authority. This exact fresh
            // reply must itself remain Reset and carry no status flags.
            if reply.reply != (ProtocolReply::StatusHeader { flags: 0, mode: 0 }) {
                return Err(DavoutError::ProtocolInspection {
                    joint: motor.joint.clone(),
                    message: "Set Zero did not confirm fault-free Reset".into(),
                });
            }
            accepted.push(reply);
            reads.push(receipt);
            let (reply, receipt) = self.inspect_protocol_query(
                &motor.joint,
                &address,
                Query::Parameter(ParameterId::MechanicalPosition),
                0xE0 + i as u8,
                &accepted,
            )?;
            let ProtocolReply::Parameter { value, .. } = reply.reply else {
                return Err(DavoutError::ProtocolInspection {
                    joint: motor.joint.clone(),
                    message: "missing mechanical position".into(),
                });
            };
            let position = f64::from(f32::from_le_bytes(value));
            if !position.is_finite()
                || position.abs() > self.homing_config.homing.zero_verify_tolerance_rad
            {
                return Err(DavoutError::ProtocolInspection {
                    joint: motor.joint.clone(),
                    message: format!("post-zero mechanical position {position} exceeds tolerance"),
                });
            }
            accepted.push(reply);
            reads.push(receipt);
        }
        for (i, motor) in motors.iter().enumerate() {
            let (reply, receipt) = self.inspect_protocol_query(
                &motor.joint,
                &MotorAddress::from(motor),
                Query::Parameter(ParameterId::CanTimeout),
                0xC0 + i as u8,
                &accepted,
            )?;
            require_timeout(&motor.joint, &reply)?;
            accepted.push(reply);
            reads.push(receipt);
        }
        self.inspection_receive(&accepted)?;
        Ok(BenchHomeQualification {
            schema: 1,
            operator: operator.trim().into(),
            mechanical_home_confirmed: true,
            sign_attested: true,
            grants_reference: false,
            timeout_counts: BENCH_TIMEOUT_COUNTS,
            before,
            reads,
        })
    }
}

fn require_timeout(joint: &str, reply: &ProtocolObservation) -> Result<(), DavoutError> {
    if reply.reply
        != (ProtocolReply::Parameter {
            index: ParameterId::CanTimeout.as_u16(),
            error: 0,
            value: BENCH_TIMEOUT_COUNTS.to_le_bytes(),
        })
    {
        return Err(DavoutError::ProtocolInspection {
            joint: joint.into(),
            message: "CAN timeout did not read back as 600 raw counts".into(),
        });
    }
    Ok(())
}
