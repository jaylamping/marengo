//! Warn-only hardware completeness v1 (mass/COM, kinematics map, limits, config coverage).

use std::path::Path;

use armee_kinematics::{actuated_joint_names, joint_limits, load_urdf};

use crate::{
    load_control_config_from, load_motors_config_from, load_robot_config_from, resolve_urdf_path,
    ConfigError,
};

const EPS: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletenessWarning {
    pub code: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletenessReport {
    pub warnings: Vec<CompletenessWarning>,
}

fn warn(
    code: &str,
    joint: Option<&str>,
    link: Option<&str>,
    message: impl Into<String>,
) -> CompletenessWarning {
    CompletenessWarning {
        code: code.to_string(),
        severity: "warn".to_string(),
        joint: joint.map(str::to_string),
        link: link.map(str::to_string),
        message: message.into(),
    }
}

/// Compute advisory completeness for master config + live URDF. Never blocks downstream flows.
pub fn completeness_report(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
) -> Result<CompletenessReport, ConfigError> {
    let repo_root = repo_root.as_ref();
    let config_dir = config_dir.as_ref();
    let robot = load_robot_config_from(config_dir)?;
    let motors = load_motors_config_from(config_dir)?;
    let control = load_control_config_from(config_dir)?;
    let urdf_path = resolve_urdf_path(repo_root, &robot)?;
    let urdf_robot = load_urdf(&urdf_path).map_err(|error| ConfigError::Parse {
        path: urdf_path.clone(),
        message: error.to_string(),
    })?;

    let mut warnings = Vec::new();

    for link in &urdf_robot.links {
        let mass_value = link.inertial.mass.value;
        if !mass_value.is_finite() || mass_value <= 0.0 {
            warnings.push(warn(
                "missing_mass",
                None,
                Some(&link.name),
                format!(
                    "link {} has missing or non-positive mass ({mass_value})",
                    link.name
                ),
            ));
        }
        let com = link.inertial.origin.xyz.0;
        if !com[0].is_finite() || !com[1].is_finite() || !com[2].is_finite() {
            warnings.push(warn(
                "missing_com",
                None,
                Some(&link.name),
                format!("link {} inertial origin/COM is not finite", link.name),
            ));
        }
    }

    let urdf_actuated = actuated_joint_names(&urdf_robot);
    let urdf_set: std::collections::HashSet<&str> =
        urdf_actuated.iter().map(String::as_str).collect();

    for joint in &robot.robot.joints {
        if !control.control.joints.contains_key(joint) {
            warnings.push(warn(
                "missing_control",
                Some(joint),
                None,
                format!("joint {joint} in robot.yaml has no control.yaml entry"),
            ));
        }
        if !motors.motors.iter().any(|m| m.joint == *joint) {
            warnings.push(warn(
                "missing_motor",
                Some(joint),
                None,
                format!("joint {joint} in robot.yaml has no motors.yaml entry"),
            ));
        }
        if !urdf_set.contains(joint.as_str()) {
            warnings.push(warn(
                "urdf_joint_missing",
                Some(joint),
                None,
                format!("joint {joint} in robot.yaml is not an actuated URDF joint"),
            ));
        }
    }

    for motor in &motors.motors {
        if !robot.robot.joints.iter().any(|j| j == &motor.joint) {
            warnings.push(warn(
                "unmapped_motor",
                Some(&motor.joint),
                None,
                format!("motor {} is not listed in robot.yaml", motor.joint),
            ));
        }
        if !urdf_set.contains(motor.joint.as_str()) {
            warnings.push(warn(
                "unmapped_joint",
                Some(&motor.joint),
                None,
                format!("motor {} has no matching actuated URDF joint", motor.joint),
            ));
            continue;
        }
        let urdf_limits = joint_limits(&urdf_robot, &motor.joint).map_err(|error| {
            ConfigError::Parse {
                path: urdf_path.clone(),
                message: error.to_string(),
            }
        })?;
        if motor.bench.position_lower_rad < urdf_limits.lower - EPS {
            warnings.push(warn(
                "limit_gap_lower",
                Some(&motor.joint),
                None,
                format!(
                    "motors bench lower {:.4} is below URDF hard lower {:.4}",
                    motor.bench.position_lower_rad, urdf_limits.lower
                ),
            ));
        }
        if motor.bench.position_upper_rad > urdf_limits.upper + EPS {
            warnings.push(warn(
                "limit_gap_upper",
                Some(&motor.joint),
                None,
                format!(
                    "motors bench upper {:.4} is above URDF hard upper {:.4}",
                    motor.bench.position_upper_rad, urdf_limits.upper
                ),
            ));
        }
    }

    for joint in &urdf_actuated {
        if !robot.robot.joints.iter().any(|j| j == joint) {
            warnings.push(warn(
                "urdf_joint_unwired",
                Some(joint),
                None,
                format!("URDF actuated joint {joint} is not listed in robot.yaml"),
            ));
        }
    }

    Ok(CompletenessReport { warnings })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::fs;

    use super::*;
    use crate::{resolve_config_dir, resolve_repo_root};

    #[test]
    fn completeness_has_no_blocking_flag() {
        let root = resolve_repo_root();
        let config_dir = resolve_config_dir(&root);
        let report = completeness_report(&root, &config_dir).expect("report");
        for warning in &report.warnings {
            assert_eq!(warning.severity, "warn");
        }
    }

    #[test]
    fn completeness_warns_unmapped_motor_joint() {
        let root = resolve_repo_root();
        let tmp = tempfile::tempdir().expect("tmp");
        let config_dir = tmp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            fs::copy(root.join("config").join(name), config_dir.join(name)).expect("copy yaml");
        }
        let assets = tmp.path().join("assets/urdf");
        fs::create_dir_all(&assets).expect("assets");
        fs::copy(root.join("assets/urdf/marengo.urdf"), assets.join("marengo.urdf")).expect("urdf");

        let mut robot = load_robot_config_from(&config_dir).expect("robot");
        robot.robot.joints.push("phantom_joint".to_string());
        fs::write(
            config_dir.join("robot.yaml"),
            serde_yaml::to_string(&robot).expect("robot yaml"),
        )
        .expect("write robot");

        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let template = motors.motors[0].clone();
        motors.motors.push(template);
        let phantom = motors.motors.last_mut().expect("phantom");
        phantom.joint = "phantom_joint".to_string();
        phantom.device_id = 9;
        fs::write(
            config_dir.join("motors.yaml"),
            serde_yaml::to_string(&motors).expect("motors yaml"),
        )
        .expect("write motors");

        let report = completeness_report(tmp.path(), &config_dir).expect("report");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.code == "unmapped_joint" && w.joint.as_deref() == Some("phantom_joint")),
            "expected unmapped_joint: {:?}",
            report.warnings
        );
    }

    #[test]
    fn completeness_warns_missing_mass_on_zero_mass_link() {
        let root = resolve_repo_root();
        let tmp = tempfile::tempdir().expect("tmp");
        let config_dir = tmp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            fs::copy(root.join("config").join(name), config_dir.join(name)).expect("copy yaml");
        }
        let assets = tmp.path().join("assets/urdf");
        fs::create_dir_all(&assets).expect("assets");
        let urdf = fs::read_to_string(root.join("assets/urdf/marengo.urdf")).expect("urdf");
        let urdf = urdf.replace("<mass value=\"0.3\"/>", "<mass value=\"0\"/>");
        fs::write(assets.join("marengo.urdf"), urdf).expect("write urdf");

        let report = completeness_report(tmp.path(), &config_dir).expect("report");
        assert!(
            report.warnings.iter().any(|w| w.code == "missing_mass"),
            "expected missing_mass: {:?}",
            report.warnings
        );
    }

    #[test]
    fn completeness_warns_limit_gap_when_bench_exceeds_urdf() {
        let root = resolve_repo_root();
        let tmp = tempfile::tempdir().expect("tmp");
        let config_dir = tmp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            fs::copy(root.join("config").join(name), config_dir.join(name)).expect("copy yaml");
        }
        let assets = tmp.path().join("assets/urdf");
        fs::create_dir_all(&assets).expect("assets");
        fs::copy(root.join("assets/urdf/marengo.urdf"), assets.join("marengo.urdf")).expect("urdf");

        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let elbow = motors
            .motors
            .iter_mut()
            .find(|m| m.joint == "right_elbow_pitch")
            .expect("elbow");
        elbow.bench.position_upper_rad = 99.0;
        fs::write(
            config_dir.join("motors.yaml"),
            serde_yaml::to_string(&motors).expect("motors yaml"),
        )
        .expect("write motors");

        let report = completeness_report(tmp.path(), &config_dir).expect("report");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.code == "limit_gap_upper" && w.joint.as_deref() == Some("right_elbow_pitch")),
            "expected limit_gap_upper: {:?}",
            report.warnings
        );
    }
}
