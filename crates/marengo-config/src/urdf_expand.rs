//! Expand-only URDF hard envelopes on disk (bench Set Limits write-behind).

use std::fs;
use std::path::Path;

use armee_kinematics::{expand_urdf_joint_hard, load_urdf};

use crate::atomic_file::write_atomic;
use crate::profile_txn::write_motors_control_files;
use crate::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    load_control_config_from, load_homing_config_from, load_motors_config_from,
    load_robot_config_from, resolve_urdf_path, validate_limit_patch, validate_motors_config,
    validate_safety_config, ConfigError, MotorsConfigFile, ProfileWriteLock,
};

/// Expand on-disk URDF hard limits so every motor bench envelope is covered.
///
/// Returns `true` if the URDF file was rewritten. Coalesce-safe: derives expands from the
/// full motors snapshot (not a single joint payload).
pub fn expand_urdf_file_to_cover_motors(
    urdf_path: impl AsRef<Path>,
    motors: &MotorsConfigFile,
) -> Result<bool, ConfigError> {
    validate_motors_config(motors)?;
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
    write_atomic(urdf_path, updated.as_bytes())?;
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
    let _lock = ProfileWriteLock::acquire(config_dir)?;
    write_motors_control_and_urdf_locked(repo_root, config_dir, motors, control)
}

pub(crate) fn write_motors_control_and_urdf_locked(
    repo_root: &Path,
    config_dir: &Path,
    motors: &MotorsConfigFile,
    control: &crate::ControlConfigFile,
) -> Result<(), ConfigError> {
    let robot = load_robot_config_from(config_dir)?;
    let homing = load_homing_config_from(config_dir)?;
    validate_safety_config(&robot, motors, control, &homing)?;
    let urdf_path = resolve_urdf_path(repo_root, &robot)?;
    let urdf_backup = std::fs::read(&urdf_path).map_err(|error| ConfigError::Io {
        path: urdf_path.clone(),
        message: error.to_string(),
    })?;
    expand_urdf_file_to_cover_motors(&urdf_path, motors)?;
    let expanded = std::fs::read(&urdf_path).map_err(|error| ConfigError::Io {
        path: urdf_path.clone(),
        message: error.to_string(),
    })?;

    if let Err(error) = write_motors_control_files(config_dir, motors, control) {
        let current = std::fs::read(&urdf_path).map_err(|read_error| ConfigError::Io {
            path: urdf_path.clone(),
            message: format!(
                "YAML persist failed ({error}); URDF verification failed: {read_error}"
            ),
        })?;
        if current == expanded {
            if let Err(restore_error) = write_atomic(&urdf_path, &urdf_backup) {
                return Err(ConfigError::Io {
                    path: urdf_path,
                    message: format!(
                        "YAML persist failed ({error}); URDF restore also failed: {restore_error}"
                    ),
                });
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Apply local checkout limit sync: motors hard, control soft inset, expand-only URDF.
/// The supplied checkout owns its config; runtime overrides cannot redirect this write.
pub fn apply_local_limit_patch(
    repo_root: impl AsRef<Path>,
    patch: &crate::LimitPatch,
) -> Result<(), ConfigError> {
    let repo_root = repo_root.as_ref();
    let config_dir = repo_root.join("config");
    if !config_dir.is_dir() {
        return Err(ConfigError::Io {
            path: config_dir,
            message: "master config directory missing".into(),
        });
    }
    let _lock = ProfileWriteLock::acquire(&config_dir)?;
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

    write_motors_control_and_urdf_locked(repo_root, &config_dir, &motors, &control)?;
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
    let (joint_start, joint_end) = find_joint_range(xml, joint)?;
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

fn find_joint_range(xml: &str, name: &str) -> Result<(usize, usize), String> {
    let mut search_from = 0;
    while search_from < xml.len() {
        let relative_start = xml[search_from..]
            .find('<')
            .ok_or_else(|| format!("joint {name} not found in URDF XML"))?;
        let start = search_from + relative_start;
        if xml[start..].starts_with("<!--") {
            search_from = xml[start + 4..]
                .find("-->")
                .map(|index| start + 4 + index + 3)
                .ok_or_else(|| "unclosed XML comment".to_string())?;
            continue;
        }
        let after_name = start + "<joint".len();
        if xml[start..].starts_with("<joint")
            && xml
                .as_bytes()
                .get(after_name)
                .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/')
        {
            let open_end = xml[after_name..]
                .find('>')
                .map(|index| after_name + index + 1)
                .ok_or_else(|| "joint opening tag unclosed".to_string())?;
            let opening_tag = &xml[start..open_end];
            if attribute_value(opening_tag, "name") == Some(name) {
                let close_end = xml[open_end..]
                    .find("</joint>")
                    .map(|index| open_end + index + "</joint>".len())
                    .ok_or_else(|| format!("joint end tag missing for {name}"))?;
                return Ok((start, close_end));
            }
        }
        search_from = start + 1;
    }
    Err(format!("joint {name} not found in URDF XML"))
}

fn attribute_value<'a>(tag: &'a str, attribute: &str) -> Option<&'a str> {
    let key = format!("{attribute}=\"");
    let mut search_from = 0;
    while let Some(relative) = tag[search_from..].find(&key) {
        let start = search_from + relative;
        if start == 0 || tag.as_bytes()[start - 1].is_ascii_whitespace() {
            let value_start = start + key.len();
            let value = &tag[value_start..];
            return Some(&value[..value.find('"')?]);
        }
        search_from = start + key.len();
    }
    None
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
    value.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::{
        load_control_config_from, load_motors_config_from, resolve_repo_root, MotorBenchLimits,
    };
    use armee_kinematics::joint_limits;

    use std::fs;

    #[test]
    fn rewrite_matches_joint_element_not_an_earlier_name_attribute() {
        let xml = r#"<robot>
  <joint name="other"><limit lower="-1" upper="1"/></joint>
  <link name="wanted"/>
  <transmission name="wanted"/>
  <joint name="wanted"><limit lower="-2" upper="2"/></joint>
  <!-- <joint name="wanted"><limit lower="-7" upper="7"/></joint> -->
</robot>"#;

        let rewritten =
            rewrite_joint_envelope_attrs(xml, "wanted", -3.0, 3.0, None).expect("rewrite");

        assert!(rewritten.contains(r#"<joint name="other"><limit lower="-1" upper="1"/></joint>"#));
        assert!(rewritten.contains(r#"<link name="wanted"/>"#));
        assert!(rewritten.contains(r#"<transmission name="wanted"/>"#));
        assert!(rewritten.contains(r#"<joint name="wanted"><limit lower="-3" upper="3"/></joint>"#));
        assert!(rewritten
            .contains(r#"<!-- <joint name="wanted"><limit lower="-7" upper="7"/></joint> -->"#));
    }

    #[test]
    fn limit_format_round_trips_sub_micro_precision() {
        let value = -1.234_567_8;
        let parsed = format_limit_value(value)
            .parse::<f64>()
            .expect("formatted value");
        assert_eq!(parsed, value);
    }
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
        let before = joint_limits(&load_urdf(&urdf_path).expect("load"), "right_elbow_pitch")
            .expect("limits before");

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
        // Expand-only: the lower bound moves only if -0.5 is past the current URDF hard.
        assert!((lim.lower - before.lower.min(-0.5)).abs() < 1e-9);
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
