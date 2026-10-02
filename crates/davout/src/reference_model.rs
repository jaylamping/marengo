//! Bounded immutable installed software model, separate from reference grants.

use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;

use marengo_config::RobotConfigFile;

use crate::DavoutError;

const MODEL_CAPACITY: usize = 512 * 1024;

/// Typed values preserve the actual installed model. The serialized-size check
/// bounds retention; it is not a journal encoding or physics qualification.
#[derive(Clone)]
pub(super) struct InstalledModelStamp {
    generation: u64,
    descriptor: Arc<(RobotConfigFile, urdf_rs::Robot)>,
}

impl fmt::Debug for InstalledModelStamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledModelStamp")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl PartialEq for InstalledModelStamp {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation && Arc::ptr_eq(&self.descriptor, &other.descriptor)
    }
}
impl Eq for InstalledModelStamp {}

impl InstalledModelStamp {
    pub(super) fn descriptor(&self) -> (&RobotConfigFile, &urdf_rs::Robot) {
        (&self.descriptor.0, &self.descriptor.1)
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
}

pub(super) struct InstalledReferenceModel {
    stamp: InstalledModelStamp,
}

impl InstalledReferenceModel {
    pub(super) fn new(robot: &RobotConfigFile, urdf: &urdf_rs::Robot) -> Result<Self, DavoutError> {
        Self::at_generation(1, robot, urdf)
    }

    pub(super) fn replacement(
        &self,
        robot: &RobotConfigFile,
        urdf: &urdf_rs::Robot,
    ) -> Result<Self, DavoutError> {
        let next = self
            .generation()
            .checked_add(1)
            .ok_or(DavoutError::InstalledGenerationExhausted)?;
        Self::at_generation(next, robot, urdf)
    }

    fn at_generation(
        generation: u64,
        robot: &RobotConfigFile,
        urdf: &urdf_rs::Robot,
    ) -> Result<Self, DavoutError> {
        // Check borrowed values before allocating their retained copies. Stop
        // serialization at the capacity, rather than allocating then truncating.
        let mut budget = ModelSizeBudget::default();
        serde_json::to_writer(&mut budget, robot).map_err(|error| DavoutError::ReferenceModel {
            message: crate::bounded_message(&error.to_string()),
        })?;
        // The pinned URDF model uses YaSerialize, not serde::Serialize. Feed its
        // XML serializer the same total budget without an intermediate String.
        yaserde::ser::serialize_with_writer(
            urdf,
            budget,
            &yaserde::ser::Config {
                perform_indent: false,
                write_document_declaration: false,
                indent_string: None,
            },
        )
        .map_err(|message| DavoutError::ReferenceModel {
            message: crate::bounded_message(&message),
        })?;
        Ok(Self {
            stamp: InstalledModelStamp {
                generation,
                descriptor: Arc::new((robot.clone(), urdf.clone())),
            },
        })
    }

    pub(super) fn generation(&self) -> u64 {
        self.stamp.generation
    }

    pub(super) fn stamp(&self) -> InstalledModelStamp {
        self.stamp.clone()
    }

    pub(super) fn matches(&self, stamp: &InstalledModelStamp) -> bool {
        self.stamp == *stamp
    }
}

#[derive(Default)]
struct ModelSizeBudget {
    written: usize,
}

impl Write for ModelSizeBudget {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buffer.len())
            .filter(|next| *next <= MODEL_CAPACITY)
            .ok_or_else(|| io::Error::other("installed reference model exceeds 512 KiB"))?;
        self.written = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod model_install_boundary_tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::simulation::{InitialVirtualReference, SimulationBus};
    use crate::{JointHomingState, OperationalMode, SafetySnapshot, Supervisor};

    #[derive(Debug, PartialEq)]
    struct Observation {
        stamp: InstalledModelStamp,
        model_xml: String,
        policy: Vec<u8>,
        bounds: Vec<(String, f64, f64)>,
        homing: Vec<(String, JointHomingState)>,
        mode: OperationalMode,
        reference_generation: u64,
        safety: SafetySnapshot,
        trace: Vec<(Option<robstride::MotorAddress>, robstride::CanFrame, bool)>,
        usable_reference: bool,
    }

    fn observe(owner: &Supervisor<SimulationBus>) -> Observation {
        Observation {
            stamp: owner.installed_model.stamp(),
            model_xml: urdf_rs::write_to_string(owner.urdf_robot())
                .expect("actual installed typed model observation"),
            policy: serde_json::to_vec(&(
                &owner.robot,
                &owner.motors,
                &owner.control,
                &owner.homing_config,
            ))
            .expect("actual public installed policy observation"),
            bounds: owner
                .robot
                .robot
                .joints
                .iter()
                .map(|joint| {
                    let policy = owner.joint_limit_policy(joint).expect("installed limit");
                    (joint.clone(), policy.hard_lower(), policy.hard_upper())
                })
                .collect(),
            homing: owner
                .robot
                .robot
                .joints
                .iter()
                .map(|joint| (joint.clone(), owner.joint_homing_state(joint)))
                .collect(),
            mode: owner.mode(),
            reference_generation: owner.reference_authority.generation(),
            safety: owner.safety_snapshot(),
            trace: owner
                .bus()
                .transmissions()
                .iter()
                .map(|write| (write.address.clone(), write.frame.clone(), write.delivered))
                .collect(),
            usable_reference: owner.reference_snapshot().usable_reference,
        }
    }

    fn valid_changed_model(owner: &Supervisor<SimulationBus>, name: &str) -> urdf_rs::Robot {
        let mut model = owner.urdf_robot().clone();
        model.name = name.into();
        model
            .joints
            .iter_mut()
            .find(|joint| joint.name == "right_elbow_pitch")
            .expect("real selected model joint")
            .origin
            .xyz
            .0[0] += 0.01;
        model
    }

    fn install(
        owner: &mut Supervisor<SimulationBus>,
        model: urdf_rs::Robot,
    ) -> Result<(), DavoutError> {
        owner.restore_limit_snapshot(owner.motors.clone(), owner.control.clone(), model)
    }

    struct Case {
        exhausted: bool,
        successful_install: Result<(), DavoutError>,
        successful_expected_xml: String,
        successful_before: Observation,
        successful_after: Observation,
        failed_install: Result<(), DavoutError>,
        failed_before: Observation,
        failed_after: Observation,
        history_absent: bool,
        master_unchanged: bool,
        fixture_removed: bool,
    }

    #[test]
    fn actual_model_restore_preserves_owner_on_stream_capacity_and_generation_refusal() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let inputs = [
            "config/robot.yaml",
            "config/motors.yaml",
            "config/control.yaml",
            "config/homing.yaml",
            "assets/urdf/marengo.urdf",
        ];
        let master: Vec<_> = inputs
            .iter()
            .map(|relative| {
                let path = source.join(relative);
                let bytes = std::fs::read(&path).expect("immutable master input");
                (path, bytes)
            })
            .collect();
        let mut cases = Vec::new();
        for exhausted in [true, false] {
            let directory = crate::test_directory::TestDirectory::new("model-boundary");
            let fixture = directory.path().to_owned();
            for relative in inputs {
                let destination = fixture.join(relative);
                std::fs::create_dir_all(destination.parent().expect("fixture parent"))
                    .expect("owned fixture directory");
                std::fs::copy(source.join(relative), destination).expect("immutable copied input");
            }
            let control_path = fixture.join("config/control.yaml");
            let control_text = std::fs::read_to_string(&control_path).expect("copied control");
            assert_eq!(
                control_text
                    .matches("active_reporting_diagnostics: true")
                    .count(),
                1
            );
            std::fs::write(
                control_path,
                control_text.replace(
                    "active_reporting_diagnostics: true",
                    "active_reporting_diagnostics: false",
                ),
            )
            .expect("copied diagnostics disabled before construction");
            let history = fixture.join("history.yaml");
            let mut owner = Supervisor::from_simulation_with_calibration_record_path(
                &fixture,
                SimulationBus::default(),
                &history,
                InitialVirtualReference::Unreferenced,
            )
            .expect("actual installed model and closed unreferenced owner");

            let successful_install;
            let successful_expected_xml;
            let successful_before;
            let successful_after;
            let failed_install;
            let failed_before;
            let failed_after;
            if exhausted {
                // Only lifetime metadata changes. A genuine installation, not
                // the injected value alone, must reach the maximum generation.
                owner.installed_model.stamp.generation = u64::MAX - 1;
                successful_before = observe(&owner);
                let model = valid_changed_model(&owner, "real-install-at-max");
                successful_expected_xml =
                    urdf_rs::write_to_string(&model).expect("valid changed model input");
                successful_install = install(&mut owner, model);
                successful_after = observe(&owner);
                failed_before = observe(&owner);
                let second = valid_changed_model(&owner, "refused-install-after-max");
                failed_install = install(&mut owner, second);
                failed_after = observe(&owner);
            } else {
                failed_before = observe(&owner);
                let mut oversized = owner.urdf_robot().clone();
                // Literal ASCII is a valid URDF attribute and leaves all limit
                // inputs unchanged. This reaches the real XML writer after the
                // real robot JSON writer, using their one aggregate size budget.
                oversized.name = "x".repeat(524_289);
                failed_install = install(&mut owner, oversized);
                failed_after = observe(&owner);
                successful_before = observe(&owner);
                let model = valid_changed_model(&owner, "real-retry-after-capacity-error");
                successful_expected_xml =
                    urdf_rs::write_to_string(&model).expect("valid retry model input");
                successful_install = install(&mut owner, model);
                successful_after = observe(&owner);
            }
            let history_absent = !history.try_exists().expect("no history side effect");
            let master_unchanged = master.iter().all(|(path, bytes)| {
                std::fs::read(path).expect("unchanged master input") == *bytes
            });
            drop(owner);
            drop(directory);
            cases.push(Case {
                exhausted,
                successful_install,
                successful_expected_xml,
                successful_before,
                successful_after,
                failed_install,
                failed_before,
                failed_after,
                history_absent,
                master_unchanged,
                fixture_removed: !fixture.try_exists().expect("actual resource cleanup"),
            });
        }

        // Both actual success neighbors and both refusal cases have run and
        // released their exclusively owned resource trees before assertions.
        for case in cases {
            case.successful_install
                .expect("actual healthy public model restore");
            assert_eq!(
                case.successful_after.model_xml,
                case.successful_expected_xml
            );
            assert_ne!(
                case.successful_after.model_xml,
                case.successful_before.model_xml
            );
            assert_ne!(case.successful_after.stamp, case.successful_before.stamp);
            assert_eq!(case.successful_after.policy, case.successful_before.policy);
            assert_eq!(case.successful_after.bounds, case.successful_before.bounds);
            if case.exhausted {
                assert_eq!(case.successful_after.stamp.generation, u64::MAX);
                assert!(matches!(
                    &case.failed_install,
                    Err(DavoutError::InstalledGenerationExhausted)
                ));
            } else {
                assert!(matches!(
                    &case.failed_install,
                    Err(DavoutError::ReferenceModel { .. })
                ));
            }
            assert_eq!(
                case.failed_after, case.failed_before,
                "rejected real model installation must preserve installed descriptor, policy, limits, reference state and wire output"
            );
            assert!(case.successful_before.trace.is_empty());
            assert!(case.successful_after.trace.is_empty());
            assert!(!case.successful_after.usable_reference);
            assert!(case
                .successful_after
                .homing
                .iter()
                .all(|(_, state)| *state == JointHomingState::Unhomed));
            assert!(case.history_absent && case.master_unchanged && case.fixture_removed);
        }
    }
}
