//! Reference admission through real public Supervisor entry points.
//! Virtual INITIAL fixtures cover admission only, never reference acquisition.
#![allow(clippy::expect_used)]

use davout::simulation::{InitialVirtualReference, SimulationBus, TxMatcher, TxOccurrence, TxRule};
use davout::{
    DavoutError, JointHomingState, MemoryBus, MitJointCommand, OperationalMode, Supervisor,
};
use robstride::CanFrame;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn virtual_supervisor(initial: InitialVirtualReference) -> Supervisor<SimulationBus> {
    Supervisor::from_simulation(root(), SimulationBus::default(), initial)
        .expect("INITIAL virtual reference")
}

fn command(joint: &str) -> MitJointCommand {
    MitJointCommand {
        joint: joint.into(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.1,
    }
}

fn pose(device: u8) -> CanFrame {
    CanFrame {
        id: 0x0280_00fd | (u32::from(device) << 8),
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        extended: true,
    }
}

#[test]
fn direct_scoped_enable_on_fresh_unhomed_refuses_before_arming() {
    let mut supervisor = Supervisor::from_repo(root(), MemoryBus::default())
        .expect("ordinary unqualified Supervisor");
    let joint = supervisor.motors.motors[0].joint.clone();
    let before = supervisor.bus().tx.clone();
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Unhomed
    );
    assert!(supervisor.enable_targets(&[joint]).is_err());
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(
        supervisor.bus().tx,
        before,
        "reference refusal transmitted frames"
    );
}

#[test]
fn ordinary_constructor_does_not_grant_reference_even_for_concrete_simulation_bus() {
    let mut supervisor =
        Supervisor::from_repo(root(), SimulationBus::default()).expect("ordinary constructor");
    let joint = supervisor.motors.motors[0].joint.clone();
    let joints: Vec<String> = supervisor
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    let before = supervisor.bus().frames().len();
    assert!(supervisor.set_homing_complete().is_err());
    assert!(supervisor.enable_targets(&joints).is_err());
    assert!(supervisor.enable_targets(&[joint]).is_err());
    assert_eq!(supervisor.bus().frames().len(), before);
    assert!(
        !supervisor.has_latched_fault(),
        "unavailable reference is a rejected request"
    );
}

#[test]
fn unqualified_calibration_refuses_before_tx_or_history_write() {
    let mut supervisor =
        Supervisor::from_repo(root(), MemoryBus::default()).expect("ordinary constructor");
    let joint = supervisor.motors.motors[0].joint.clone();
    let before = supervisor.bus().tx.clone();
    assert!(matches!(
        supervisor.calibrate_joint_zero(&joint, "operator", true),
        Err(DavoutError::ReferenceUnsupported { .. })
    ));
    assert_eq!(supervisor.bus().tx, before);
    assert!(!supervisor.has_latched_fault());
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Unhomed
    );
}

#[test]
fn legacy_sign_and_target_preflight_remain_specific_without_arming_peers() {
    let mut supervisor = virtual_supervisor(InitialVirtualReference::Unreferenced);
    let joint = supervisor.motors.motors[0].joint.clone();
    supervisor.bus_mut().clear_trace();
    assert!(matches!(
        supervisor.calibrate_joint_zero("missing", "operator", true),
        Err(DavoutError::UnknownJoint { .. })
    ));
    assert!(matches!(
        supervisor.calibrate_joint_zero(&joint, "operator", false),
        Err(DavoutError::HomingVerify { .. })
    ));
    assert!(supervisor.bus().frames().is_empty());
    assert!(!supervisor.has_latched_fault());
}

#[test]
fn initial_virtual_reference_is_owner_local_and_independent_of_history_state() {
    let mut granted = virtual_supervisor(InitialVirtualReference::AllConfigured);
    let mut unreferenced = virtual_supervisor(InitialVirtualReference::Unreferenced);
    let joint = granted.motors.motors[0].joint.clone();
    // No legacy registry remains: only the private authority projects Verified.
    assert_eq!(
        granted.joint_homing_state(&joint),
        JointHomingState::Verified
    );
    assert_eq!(
        unreferenced.joint_homing_state(&joint),
        JointHomingState::Unhomed
    );
    granted
        .enable_targets(std::slice::from_ref(&joint))
        .expect("declared virtual reference");
    let device = unreferenced.motors.motors[0].device_id;
    unreferenced
        .bus_mut()
        .queue_frame(pose(device))
        .expect("ordinary valid data");
    unreferenced.drain_feedback().expect("diagnostic data");
    unreferenced.bus_mut().clear_trace();
    assert!(unreferenced.enable_targets(&[joint]).is_err());
    assert!(
        unreferenced.bus().frames().is_empty(),
        "pose was mistaken for reference"
    );
}

#[test]
fn initial_reference_covers_only_declared_joints() {
    let pitch = "right_shoulder_pitch".to_string();
    let roll = "right_shoulder_roll".to_string();
    let mut supervisor = virtual_supervisor(InitialVirtualReference::Joints(vec![pitch.clone()]));
    assert!(supervisor.set_homing_complete().is_err());
    assert_eq!(
        supervisor.joint_homing_state(&roll),
        JointHomingState::Unhomed
    );
    supervisor
        .enable_targets(std::slice::from_ref(&pitch))
        .expect("scoped initial reference");
    supervisor
        .enable_targets(std::slice::from_ref(&pitch))
        .expect("current scoped set remains authorized");
    supervisor.disable_all().expect("ordinary stop");
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.enable_targets(&[roll]).is_err());
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn successful_disable_preserves_reference_but_starts_a_new_motion_session() {
    let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
    let joint = supervisor.motors.motors[0].joint.clone();
    supervisor
        .enable_targets(std::slice::from_ref(&joint))
        .expect("enable");
    let reference_generation = supervisor.reference_generation();
    let stop_generation = supervisor.stop_generation();
    supervisor.disable_all().expect("successful ordinary stop");
    assert!(supervisor.stop_generation() > stop_generation);
    assert_eq!(supervisor.reference_generation(), reference_generation);
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Verified
    );
    supervisor
        .enable_targets(std::slice::from_ref(&joint))
        .expect("new explicit motion session");
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&joint)]).is_err(),
        "old session pose cannot authorize new output"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn reference_policy_mutation_permanently_revokes_even_if_old_fields_are_restored() {
    for change in 0..12 {
        let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
        let joint = supervisor.motors.motors[0].joint.clone();
        let motors_before = supervisor.motors.clone();
        let homing_before = supervisor.homing_config.clone();
        let control_before = supervisor.control.clone();
        match change {
            0 => supervisor.motors.motors[0].direction *= -1,
            1 => supervisor.motors.motors[0].gear_ratio *= 2.0,
            2 => supervisor.motors.motors[0].can_interface = "changed".into(),
            3 => supervisor.motors.motors[0].device_id = 77,
            4 => supervisor.motors.motors[0].bench.position_lower_rad += 0.01,
            5 => {
                supervisor
                    .homing_config
                    .homing
                    .joints
                    .get_mut(&joint)
                    .expect("homing")
                    .overrides
                    .home_offset_rad = Some(0.01)
            }
            6 => {
                supervisor
                    .homing_config
                    .homing
                    .joints
                    .get_mut(&joint)
                    .expect("homing")
                    .overrides
                    .sign_test_required = Some(false)
            }
            7 => {
                supervisor
                    .control
                    .control
                    .joints
                    .get_mut(&joint)
                    .expect("control")
                    .position_hold_trim_rad = 0.01
            }
            8 => {
                supervisor
                    .control
                    .control
                    .joints
                    .get_mut(&joint)
                    .expect("control")
                    .position_limit_margin_min_rad += 0.001
            }
            9 => {
                supervisor
                    .control
                    .control
                    .joints
                    .get_mut(&joint)
                    .expect("control")
                    .velocity_max_rad_s = Some(1.5)
            }
            10 => {
                supervisor
                    .control
                    .control
                    .joints
                    .get_mut(&joint)
                    .expect("control")
                    .position_trajectory_accel_rad_s2 *= 0.5
            }
            _ => supervisor.motors.motors[0].bench.velocity_limit_rad_s *= 0.9,
        }
        assert_eq!(
            supervisor.joint_homing_state(&joint),
            JointHomingState::Unhomed,
            "change {change}"
        );
        supervisor.motors = motors_before;
        supervisor.homing_config = homing_before;
        supervisor.control = control_before;
        supervisor.bus_mut().clear_trace();
        assert!(
            supervisor
                .enable_targets(std::slice::from_ref(&joint))
                .is_err(),
            "restored fields revived change {change}"
        );
        assert!(supervisor.bus().frames().is_empty());
    }
}

#[test]
fn stale_reference_cannot_pass_active_shortcuts_or_output() {
    for entry in 0..3 {
        let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
        let joint = supervisor.motors.motors[0].joint.clone();
        supervisor
            .enable_targets(std::slice::from_ref(&joint))
            .expect("enable");
        supervisor.motors.motors[0].direction *= -1;
        let joints: Vec<String> = supervisor
            .motors
            .motors
            .iter()
            .map(|motor| motor.joint.clone())
            .collect();
        let result = match entry {
            0 => supervisor.enable_targets(&joints),
            1 => supervisor.enable_targets(std::slice::from_ref(&joint)),
            _ => supervisor.send_mit_batch(vec![command(&joint)]),
        };
        assert!(
            result.is_err(),
            "active shortcut {entry} accepted stale reference"
        );
        assert_eq!(supervisor.mode(), OperationalMode::Disabled);
        assert_eq!(
            supervisor.joint_homing_state(&joint),
            JointHomingState::Unhomed
        );
        assert_eq!(
            supervisor
                .safety_snapshot()
                .last_stop
                .expect("stop")
                .attempts
                .len(),
            15
        );
    }
}

#[test]
fn valid_output_only_cap_and_watchdog_changes_preserve_reference() {
    let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
    let joint = supervisor.motors.motors[0].joint.clone();
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
        .control
        .control
        .motor_type_defaults
        .get_mut("rs03")
        .expect("type policy")
        // Above master pitch static friction (fs 0.65 Nm), which validation requires.
        .tau_ff_max_nm = 0.7;
    supervisor
        .set_homing_complete()
        .expect("validated compatible output policy");
    let joints: Vec<String> = supervisor
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    supervisor.enable_targets(&joints).expect("enable");
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Verified
    );
}

#[test]
fn fault_and_stop_uncertainty_revoke_initial_reference_without_automatic_recovery() {
    for failed_stop in [false, true] {
        let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
        let joint = supervisor.motors.motors[0].joint.clone();
        supervisor
            .enable_targets(std::slice::from_ref(&joint))
            .expect("enable");
        if failed_stop {
            let rule = supervisor
                .bus_mut()
                .add_tx_rule(TxRule {
                    matcher: TxMatcher {
                        communication_type: Some(4),
                        ..TxMatcher::default()
                    },
                    occurrence: TxOccurrence::Every,
                    receive: vec![],
                    send_error: Some("scripted stop failure".into()),
                })
                .expect("finite stop rule");
            assert!(supervisor.disable_all().is_err());
            assert_eq!(supervisor.bus().rule_trigger_count(rule), 5);
            supervisor.bus_mut().remove_tx_rule(rule);
        } else {
            supervisor.set_hardware_estop(true);
            supervisor.set_hardware_estop(false);
        }
        assert_ne!(
            supervisor.joint_homing_state(&joint),
            JointHomingState::Verified
        );
        supervisor
            .disable_all()
            .expect("later accepted stop writes");
        assert_ne!(
            supervisor.joint_homing_state(&joint),
            JointHomingState::Verified
        );
        supervisor.bus_mut().clear_trace();
        assert!(supervisor.enable_targets(&[joint]).is_err());
        assert!(supervisor.bus().frames().is_empty());
    }
}

#[test]
fn rebuilding_limits_revokes_initial_reference() {
    let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
    let joint = supervisor.motors.motors[0].joint.clone();
    let motors = supervisor.motors.clone();
    let control = supervisor.control.clone();
    let urdf_robot = supervisor.urdf_robot().clone();
    supervisor
        .restore_limit_snapshot(motors, control, urdf_robot)
        .expect("valid rebuilt policy");
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Unhomed
    );
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.enable_targets(&[joint]).is_err());
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn cached_valid_run_pose_cannot_qualify_enable_or_write_history() {
    let mut supervisor = Supervisor::from_simulation(
        root(),
        SimulationBus::default(),
        InitialVirtualReference::Unreferenced,
    )
    .expect("unreferenced virtual owner");
    let motor = supervisor.motors.motors[0].clone();
    supervisor
        .bus_mut()
        .queue_frame(pose(motor.device_id))
        .expect("literal Run pose");
    assert_eq!(supervisor.drain_feedback().expect("real cache receive"), 1);
    assert!(
        supervisor.joint_feedback(&motor.joint).is_some(),
        "valid cache was never reached"
    );
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.enable_targets(&[motor.joint.clone()]).is_err());
    assert!(supervisor.bus().frames().is_empty());
    assert_eq!(
        supervisor.joint_homing_state(&motor.joint),
        JointHomingState::Unhomed
    );
}

#[test]
fn unsupported_reference_methods_refuse_before_target_or_peer_arming() {
    for method in [
        marengo_config::HomingMethod::HallThreeSensor,
        marengo_config::HomingMethod::None,
    ] {
        let mut supervisor = virtual_supervisor(InitialVirtualReference::Unreferenced);
        let joint = supervisor.motors.motors[0].joint.clone();
        supervisor
            .homing_config
            .homing
            .joints
            .get_mut(&joint)
            .expect("homing policy")
            .method = method;
        if method == marengo_config::HomingMethod::HallThreeSensor {
            supervisor
                .homing_config
                .homing
                .joints
                .get_mut(&joint)
                .expect("homing policy")
                .overrides
                .sensors = Some(marengo_config::HomingSensors {
                home: marengo_config::SensorInput {
                    gpio: 10,
                    active_high: true,
                },
                min_limit: marengo_config::SensorInput {
                    gpio: 11,
                    active_high: true,
                },
                max_limit: marengo_config::SensorInput {
                    gpio: 12,
                    active_high: true,
                },
            });
        }
        supervisor.bus_mut().clear_trace();
        assert!(
            matches!(
                supervisor.calibrate_joint_zero(&joint, "operator", true),
                Err(DavoutError::ReferenceUnsupported { .. })
            ),
            "method {method:?}"
        );
        assert!(supervisor.bus().frames().is_empty());
        assert!(!supervisor.has_latched_fault());
    }
}

#[test]
fn temporary_policy_during_receive_revokes_and_preserves_installed_peer_faults() {
    for (active, peer_fault) in [(false, false), (true, false), (true, true)] {
        let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
        let pitch = supervisor.motors.motors[0].clone();
        let peer = supervisor.motors.motors[1].clone();
        let installed_addresses: Vec<_> = supervisor
            .motors
            .motors
            .iter()
            .map(robstride::MotorAddress::from)
            .collect();
        let original = supervisor.motors.clone();
        if active {
            supervisor
                .enable_targets(std::slice::from_ref(&pitch.joint))
                .expect("active initial fixture");
        }
        let before_stop = supervisor.stop_generation();
        supervisor.motors.motors[0].direction *= -1;
        supervisor.motors.motors[0].gear_ratio *= 2.0;
        supervisor.motors.motors[1].device_id = 77;
        supervisor.bus_mut().clear_trace();
        let mut peer_status = pose(peer.device_id);
        if peer_fault {
            peer_status.id |= 1 << 16;
        }
        supervisor
            .bus_mut()
            .queue_frames([pose(pitch.device_id), peer_status])
            .expect("installed-address raw receive");
        let received = supervisor.drain_feedback();
        assert_eq!(received.is_err(), active);
        assert!(
            supervisor.joint_feedback(&pitch.joint).is_some(),
            "raw literal pose did not reach installed projection"
        );
        supervisor.motors = original;
        if active {
            let snapshot = supervisor.safety_snapshot();
            assert_eq!(
                supervisor.stop_generation(),
                before_stop + 1,
                "duplicate stop burst"
            );
            let attempts = snapshot.last_stop.expect("active reference stop").attempts;
            assert_eq!(attempts.len(), 15);
            for address in installed_addresses {
                assert_eq!(
                    attempts
                        .iter()
                        .filter(|attempt| attempt.address == address)
                        .count(),
                    3
                );
            }
            if peer_fault {
                assert!(
                    snapshot.faults.iter().any(|fault| fault.address.as_ref()
                        == Some(&robstride::MotorAddress::from(&peer))
                        && fault.device.status_flags == 1),
                    "installed peer fault disappeared behind changed public ID"
                );
            } else {
                assert!(
                    snapshot.faults.is_empty(),
                    "reference mismatch invented a device fault"
                );
            }
        }
        assert_ne!(
            supervisor.joint_homing_state(&pitch.joint),
            JointHomingState::Verified
        );
        supervisor.bus_mut().clear_trace();
        assert!(supervisor.enable_targets(&[pitch.joint]).is_err());
        assert!(supervisor.bus().frames().is_empty());
    }
}

#[test]
fn lowered_motor_output_cap_preserves_reference_and_bounds_literal_wire_torque() {
    let mut supervisor = virtual_supervisor(InitialVirtualReference::AllConfigured);
    let motor = supervisor.motors.motors[0].clone();
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("enable");
    supervisor
        .bus_mut()
        .queue_frame(pose(motor.device_id))
        .expect("pose");
    supervisor.drain_feedback().expect("post-enable status");
    supervisor.motors.motors[0].bench.torque_limit_nm = 0.05;
    supervisor.bus_mut().clear_trace();
    supervisor
        .send_mit_batch(vec![command(&motor.joint)])
        .expect("compatible reduced output cap");
    assert_eq!(
        supervisor.joint_homing_state(&motor.joint),
        JointHomingState::Verified
    );
    assert_eq!(supervisor.bus().frames().len(), 1);
    let raw_torque = (supervisor.bus().frames()[0].id >> 8) & 0xffff;
    let wire_torque = (f64::from(raw_torque) / 32767.0 - 1.0) * 60.0;
    assert!(
        wire_torque.abs() <= 0.052,
        "RS03 literal wire torque {wire_torque} exceeded reduced .05Nm cap"
    );
}
