//! Expand-only URDF hard envelopes on disk (bench Set Limits write-behind).

use std::fs;
use std::path::Path;

use armee_kinematics::{expand_urdf_joint_hard, load_urdf};

use crate::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    load_control_config_from, load_motors_config_from, load_robot_config_from, resolve_config_dir,
    resolve_urdf_path, validate_limit_patch, write_motors_and_control, ConfigError,
    MotorsConfigFile,
};

/// Expand on-disk URDF hard limits so every motor bench envelope is covered.
///
/// Returns `true` if the URDF file was rewritten. Coalesce-safe: derives expands from the
/// full motors snapshot (not a single joint payload).
pub fn expand_urdf_file_to_cover_motors(
    urdf_path: impl AsRef<Path>,
    motors: &MotorsConfigFile,
) -> Result<bool, ConfigError> {
    let urdf_path = urdf_path.as_ref();
    let mut robot = load_urdf(urdf_path).map_err(|error| ConfigError::Parse {
        path: urdf_path.to_path_buf(),
        message: error.to_string(),
    })?;

    let mut expanded_joints = Vec::new();
    for motor in &motors.motors {
        let expanded = expand_urdf_joint_hard(
            &mut robot,
            &motor.joint,
            motor.bench.position_lower_rad,
            motor.bench.position_upper_rad,
        )
        .map_err(|error| ConfigError::Parse {
            path: urdf_path.to_path_buf(),
            message: error.to_string(),
        })?;
        if expanded {
            expanded_joints.push(motor.joint.clone());
        }
    }
    if expanded_joints.is_empty() {
        return Ok(false);
    }

    // Persist attrs from the mutated in-memory model (hard + safety_controller soft).
    let original = fs::read_to_string(urdf_path).map_err(|error| ConfigError::Io {
        path: urdf_path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut updated = original;
    for joint_name in &expanded_joints {
        let joint = robot
            .joints
            .iter()
            .find(|j| j.name == *joint_name)
            .ok_or_else(|| ConfigError::Parse {
                path: urdf_path.to_path_buf(),
                message: format!("joint {joint_name} missing after expand"),
            })?;
        let soft = joint
            .safety_controller
            .as_ref()
            .map(|s| (s.soft_lower_limit, s.soft_upper_limit));
        updated = rewrite_joint_envelope_attrs(
            &updated,
            joint_name,
            joint.limit.lower,
            joint.limit.upper,
            soft,
        )
        .map_err(|message| ConfigError::Parse {
            path: urdf_path.to_path_buf(),
            message,
        })?;
    }

    let temporary = urdf_path.with_extension("urdf.tmp");
    fs::write(&temporary, &updated).map_err(|error| ConfigError::Io {
        path: temporary.clone(),
        message: error.to_string(),
    })?;
    fs::rename(&temporary, urdf_path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        ConfigError::Io {
            path: urdf_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    Ok(true)
}

/// Atomic write-behind: expand URDF first, then motors+control YAML.
///
/// URDF-first avoids restart with wider motors against a narrow URDF (Enable hard-limit trip).
/// If YAML write fails after URDF expand, restore the previous URDF bytes.
pub fn write_motors_control_and_urdf(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    motors: &MotorsConfigFile,
    control: &crate::ControlConfigFile,
) -> Result<(), ConfigError> {
    let repo_root = repo_root.as_ref();
    let config_dir = config_dir.as_ref();
    let robot = load_robot_config_from(config_dir)?;
    let urdf_path = resolve_urdf_path(repo_root, &robot)?;
    let urdf_backup = fs::read(&urdf_path).map_err(|error| ConfigError::Io {
        path: urdf_path.clone(),
        message: error.to_string(),
    })?;

    expand_urdf_file_to_cover_motors(&urdf_path, motors)?;

    if let Err(error) = write_motors_and_control(config_dir, motors, control) {
        if let Err(restore_error) = fs::write(&urdf_path, &urdf_backup) {
            return Err(ConfigError::Io {
                path: urdf_path,
                message: format!(
                    "YAML persist failed ({error}); URDF restore also failed: {restore_error}"
                ),
            });
        }
        return Err(error);
    }
    Ok(())
}

/// Apply local checkout limit sync: motors hard, control soft inset, expand-only URDF.
pub fn apply_local_limit_patch(
    repo_root: impl AsRef<Path>,
    patch: &crate::LimitPatch,
) -> Result<(), ConfigError> {
    let repo_root = repo_root.as_ref();
    let config_dir = resolve_config_dir(repo_root);
    if !config_dir.is_dir() {
        return Err(ConfigError::Io {
            path: config_dir,
            message: "master config directory missing".into(),
        });
    }

    validate_limit_patch(patch)?;
    let mut patch = patch.clone();
    ensure_soft_inset(&mut patch);

    let mut motors = load_motors_config_from(&config_dir)?;
    let mut control = load_control_config_from(&config_dir)?;

    let motor = motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == patch.joint)
        .ok_or_else(|| ConfigError::Parse {
            path: config_dir.clone(),
            message: format!("joint {} not in motors.yaml", patch.joint),
        })?;
    apply_limit_patch_to_motor(motor, &patch)?;
    let control_entry = control
        .control
        .joints
        .get_mut(&patch.joint)
        .ok_or_else(|| ConfigError::Parse {
            path: config_dir.clone(),
            message: format!("joint {} not in control.yaml", patch.joint),
        })?;
    apply_limit_patch_to_control(control_entry, &patch)?;

    write_motors_control_and_urdf(repo_root, &config_dir, &motors, &control)?;
    Ok(())
}

/// Rewrite hard `<limit>` (and optional `safety_controller` soft) from the mutated model.
fn rewrite_joint_envelope_attrs(
    xml: &str,
    joint: &str,
    lower: f64,
    upper: f64,
    soft: Option<(f64, f64)>,
) -> Result<String, String> {
    let open = format!("name=\"{joint}\"");
    let start = xml
        .find(&open)
        .ok_or_else(|| format!("joint {joint} not found in URDF XML"))?;
    let joint_start = xml[..start]
        .rfind("<joint")
        .ok_or_else(|| format!("joint tag start missing for {joint}"))?;
    let after = &xml[start..];
    let rel_end = after
        .find("</joint>")
        .ok_or_else(|| format!("joint end tag missing for {joint}"))?;
    let joint_end = start + rel_end + "</joint>".len();
    let mut block = xml[joint_start..joint_end].to_string();

    block = rewrite_tag_attrs(&block, "<limit", &[("lower", lower), ("upper", upper)])?;
    if let Some((soft_lo, soft_hi)) = soft {
        if block.contains("<safety_controller") {
            block = rewrite_tag_attrs(
                &block,
                "<safety_controller",
                &[("soft_lower_limit", soft_lo), ("soft_upper_limit", soft_hi)],
            )?;
        }
    }

    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..joint_start]);
    out.push_str(&block);
    out.push_str(&xml[joint_end..]);
    Ok(out)
}

fn rewrite_tag_attrs(block: &str, tag_open: &str, attrs: &[(&str, f64)]) -> Result<String, String> {
    let limit_start = block
        .find(tag_open)
        .ok_or_else(|| format!("{tag_open} tag missing"))?;
    let limit_rel = &block[limit_start..];
    let limit_end_rel = limit_rel
        .find("/>")
        .or_else(|| limit_rel.find('>'))
        .ok_or_else(|| format!("{tag_open} tag unclosed"))?;
    let mut tag = limit_rel[..limit_end_rel].to_string();
    for &(name, value) in attrs {
        tag = replace_attr(&tag, name, value)?;
    }
    let mut out = String::with_capacity(block.len() + 16);
    out.push_str(&block[..limit_start]);
    out.push_str(&tag);
    out.push_str(&limit_rel[limit_end_rel..]);
    Ok(out)
}

fn replace_attr(tag: &str, name: &str, value: f64) -> Result<String, String> {
    let key = format!("{name}=\"");
    let Some(idx) = tag.find(&key) else {
        return Err(format!("attribute {name} missing on limit tag"));
    };
    let value_start = idx + key.len();
    let rest = &tag[value_start..];
    let Some(end) = rest.find('"') else {
        return Err(format!("attribute {name} unclosed on limit tag"));
    };
    let mut out = String::with_capacity(tag.len() + 16);
    out.push_str(&tag[..value_start]);
    out.push_str(&format_limit_value(value));
    out.push_str(&rest[end..]);
    Ok(out)
}

fn format_limit_value(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::{
        load_control_config_from, load_motors_config_from, resolve_repo_root, MotorBenchLimits,
    };
    use armee_kinematics::joint_limits;

    #[test]
    fn rewrite_updates_elbow_hard_only() {
        let root = resolve_repo_root();
        let src = root.join("assets/urdf/marengo.urdf");
        let xml = fs::read_to_string(&src).expect("read");
        let out = rewrite_joint_envelope_attrs(&xml, "right_elbow_pitch", -0.5, 3.0, None)
            .expect("rewrite");
        assert!(out.contains("lower=\"-0.5\""));
        assert!(out.contains("upper=\"3\""));
        assert!(out.contains("name=\"right_shoulder_pitch\""));
    }

    #[test]
    fn rewrite_updates_soft_from_mutated_model() {
        let root = resolve_repo_root();
        let src = root.join("assets/urdf/marengo.urdf");
        let xml = fs::read_to_string(&src).expect("read");
        let out =
            rewrite_joint_envelope_attrs(&xml, "right_elbow_pitch", -0.8, 1.2, Some((-0.8, 1.15)))
                .expect("rewrite");
        assert!(out.contains("lower=\"-0.8\""));
        assert!(out.contains("soft_lower_limit=\"-0.8\""));
    }

    #[test]
    fn expand_file_covers_motors_and_round_trips() {
        let root = resolve_repo_root();
        let tmp = tempfile::tempdir().expect("tmp");
        let urdf_path = tmp.path().join("marengo.urdf");
        fs::copy(root.join("assets/urdf/marengo.urdf"), &urdf_path).expect("copy");

        let mut motors = load_motors_config_from(root.join("config")).expect("motors");
        let elbow = motors
            .motors
            .iter_mut()
            .find(|m| m.joint == "right_elbow_pitch")
            .expect("elbow");
        elbow.bench = MotorBenchLimits {
            position_lower_rad: -0.5,
            position_upper_rad: 3.0,
            velocity_limit_rad_s: elbow.bench.velocity_limit_rad_s,
            torque_limit_nm: elbow.bench.torque_limit_nm,
        };

        assert!(expand_urdf_file_to_cover_motors(&urdf_path, &motors).expect("expand"));
        let robot = load_urdf(&urdf_path).expect("reload");
        let lim = joint_limits(&robot, "right_elbow_pitch").expect("limits");
        assert!((lim.lower - (-0.5)).abs() < 1e-9);
        assert!((lim.upper - 3.0).abs() < 1e-9);
        assert!(!expand_urdf_file_to_cover_motors(&urdf_path, &motors).expect("noop"));
    }

    #[test]
    fn write_motors_control_and_urdf_expands_temp_tree() {
        let root = resolve_repo_root();
        let tmp = tempfile::tempdir().expect("tmp");
        let config_dir = tmp.path().join("config");
        let assets = tmp.path().join("assets/urdf");
        fs::create_dir_all(&assets).expect("assets");
        fs::create_dir_all(&config_dir).expect("config");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            fs::copy(root.join("config").join(name), config_dir.join(name)).expect("yaml");
        }
        fs::copy(
            root.join("assets/urdf/marengo.urdf"),
            assets.join("marengo.urdf"),
        )
        .expect("urdf");

        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let control = load_control_config_from(&config_dir).expect("control");
        let elbow = motors
            .motors
            .iter_mut()
            .find(|m| m.joint == "right_elbow_pitch")
            .expect("elbow");
        elbow.bench.position_lower_rad = -0.4;
        elbow.bench.position_upper_rad = 2.8;

        write_motors_control_and_urdf(tmp.path(), &config_dir, &motors, &control).expect("write");
        let robot = load_urdf(assets.join("marengo.urdf")).expect("urdf");
        let lim = joint_limits(&robot, "right_elbow_pitch").expect("limits");
        assert!(lim.lower <= -0.4 + 1e-9);
        assert!(lim.upper >= 2.8 - 1e-9);
    }
}
