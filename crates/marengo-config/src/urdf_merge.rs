//! Joint-keyed URDF merge preview, resolution, and XML apply (#108).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use armee_kinematics::{actuated_joint_names, load_urdf};
use urdf_rs::JointType;

use crate::ConfigError;

const EPS: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FieldDiff {
    pub joint: String,
    pub field: String,
    pub master_value: String,
    pub contributor_value: String,
    pub kinematics_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MergePreview {
    pub overlapping_joints: Vec<String>,
    pub new_joints: Vec<String>,
    pub field_diffs: Vec<FieldDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionChoice {
    Master,
    Contributor,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FieldResolution {
    pub joint: String,
    pub field: String,
    pub choice: ResolutionChoice,
}

fn parse_error(path: &Path, message: impl Into<String>) -> ConfigError {
    ConfigError::Parse {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

fn format_vec3(values: &[f64; 3]) -> String {
    format!("[{:.6}, {:.6}, {:.6}]", values[0], values[1], values[2])
}

fn format_joint_type(joint_type: &JointType) -> String {
    match joint_type {
        JointType::Revolute => "revolute".to_string(),
        JointType::Continuous => "continuous".to_string(),
        JointType::Prismatic => "prismatic".to_string(),
        JointType::Fixed => "fixed".to_string(),
        JointType::Floating => "floating".to_string(),
        JointType::Planar => "planar".to_string(),
        JointType::Spherical => "spherical".to_string(),
    }
}

#[allow(dead_code)]
fn is_actuated(joint_type: &JointType) -> bool {
    matches!(
        *joint_type,
        JointType::Revolute | JointType::Continuous | JointType::Prismatic
    )
}

/// Compare master and contributor URDF bytes; overlap keyed by actuated joint names.
pub fn merge_preview_from_paths(
    master_path: impl AsRef<Path>,
    contributor_path: impl AsRef<Path>,
) -> Result<MergePreview, ConfigError> {
    let master_path = master_path.as_ref();
    let contributor_path = contributor_path.as_ref();
    let master = load_urdf(master_path).map_err(|e| parse_error(master_path, e.to_string()))?;
    let contributor =
        load_urdf(contributor_path).map_err(|e| parse_error(contributor_path, e.to_string()))?;
    Ok(merge_preview_from_robots(&master, &contributor))
}

pub fn merge_preview_from_robots(
    master: &urdf_rs::Robot,
    contributor: &urdf_rs::Robot,
) -> MergePreview {
    let master_actuated = actuated_joint_names(master);
    let contributor_actuated = actuated_joint_names(contributor);
    let master_set: HashSet<&str> = master_actuated.iter().map(String::as_str).collect();

    let overlapping_joints: Vec<String> = contributor_actuated
        .iter()
        .filter(|j| master_set.contains(j.as_str()))
        .cloned()
        .collect();
    let new_joints: Vec<String> = contributor_actuated
        .iter()
        .filter(|j| !master_set.contains(j.as_str()))
        .cloned()
        .collect();

    let mut field_diffs = Vec::new();
    for joint_name in &overlapping_joints {
        let Some(master_joint) = master.joints.iter().find(|j| j.name == *joint_name) else {
            continue;
        };
        let Some(contributor_joint) = contributor.joints.iter().find(|j| j.name == *joint_name)
        else {
            continue;
        };
        push_joint_field_diffs(&mut field_diffs, master_joint, contributor_joint);
    }

    MergePreview {
        overlapping_joints,
        new_joints,
        field_diffs,
    }
}

fn push_joint_field_diffs(
    out: &mut Vec<FieldDiff>,
    master: &urdf_rs::Joint,
    contributor: &urdf_rs::Joint,
) {
    let joint = master.name.clone();
    if master.parent.link != contributor.parent.link {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "parent".to_string(),
            master_value: master.parent.link.clone(),
            contributor_value: contributor.parent.link.clone(),
            kinematics_critical: true,
        });
    }
    if format_joint_type(&master.joint_type) != format_joint_type(&contributor.joint_type) {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "type".to_string(),
            master_value: format_joint_type(&master.joint_type),
            contributor_value: format_joint_type(&contributor.joint_type),
            kinematics_critical: true,
        });
    }
    if !approx_eq(master.origin.xyz.0[0], contributor.origin.xyz.0[0])
        || !approx_eq(master.origin.xyz.0[1], contributor.origin.xyz.0[1])
        || !approx_eq(master.origin.xyz.0[2], contributor.origin.xyz.0[2])
        || !approx_eq(master.origin.rpy.0[0], contributor.origin.rpy.0[0])
        || !approx_eq(master.origin.rpy.0[1], contributor.origin.rpy.0[1])
        || !approx_eq(master.origin.rpy.0[2], contributor.origin.rpy.0[2])
    {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "origin".to_string(),
            master_value: format!(
                "xyz={} rpy={}",
                format_vec3(&master.origin.xyz.0),
                format_vec3(&master.origin.rpy.0)
            ),
            contributor_value: format!(
                "xyz={} rpy={}",
                format_vec3(&contributor.origin.xyz.0),
                format_vec3(&contributor.origin.rpy.0)
            ),
            kinematics_critical: true,
        });
    }
    if !approx_eq(master.axis.xyz.0[0], contributor.axis.xyz.0[0])
        || !approx_eq(master.axis.xyz.0[1], contributor.axis.xyz.0[1])
        || !approx_eq(master.axis.xyz.0[2], contributor.axis.xyz.0[2])
    {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "axis".to_string(),
            master_value: format_vec3(&master.axis.xyz.0),
            contributor_value: format_vec3(&contributor.axis.xyz.0),
            kinematics_critical: true,
        });
    }
    if !approx_eq(master.limit.lower, contributor.limit.lower) {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "limit_lower".to_string(),
            master_value: format_limit_value(master.limit.lower),
            contributor_value: format_limit_value(contributor.limit.lower),
            kinematics_critical: true,
        });
    }
    if !approx_eq(master.limit.upper, contributor.limit.upper) {
        out.push(FieldDiff {
            joint: joint.clone(),
            field: "limit_upper".to_string(),
            master_value: format_limit_value(master.limit.upper),
            contributor_value: format_limit_value(contributor.limit.upper),
            kinematics_critical: true,
        });
    }
}

pub fn unresolved_critical_fields(
    preview: &MergePreview,
    resolutions: &[FieldResolution],
) -> Vec<(String, String)> {
    let resolved: HashSet<(String, String)> = resolutions
        .iter()
        .map(|r| (r.joint.clone(), r.field.clone()))
        .collect();
    preview
        .field_diffs
        .iter()
        .filter(|diff| diff.kinematics_critical)
        .filter(|diff| !resolved.contains(&(diff.joint.clone(), diff.field.clone())))
        .map(|diff| (diff.joint.clone(), diff.field.clone()))
        .collect()
}

/// Simulate merged URDF XML without durable write.
pub fn simulate_merge_xml(
    master_xml: &str,
    contributor_xml: &str,
    resolutions: &[FieldResolution],
) -> Result<String, ConfigError> {
    let path = Path::new("marengo.urdf");
    let master = load_urdf_from_str(master_xml, path)?;
    let contributor = load_urdf_from_str(contributor_xml, path)?;
    let preview = merge_preview_from_robots(&master, &contributor);
    let unresolved = unresolved_critical_fields(&preview, resolutions);
    if !unresolved.is_empty() {
        let detail = unresolved
            .iter()
            .map(|(joint, field)| format!("{joint}.{field}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(parse_error(
            path,
            format!("kinematics-critical fields unresolved: {detail}"),
        ));
    }
    let merged = apply_merge_xml(master_xml, contributor_xml, &preview, resolutions)?;
    validate_merged_urdf_xml(&merged)?;
    Ok(merged)
}

pub fn apply_merge_xml(
    master_xml: &str,
    contributor_xml: &str,
    preview: &MergePreview,
    resolutions: &[FieldResolution],
) -> Result<String, ConfigError> {
    let path = Path::new("marengo.urdf");
    let resolution_map: HashMap<(String, String), ResolutionChoice> = resolutions
        .iter()
        .map(|r| ((r.joint.clone(), r.field.clone()), r.choice))
        .collect();

    let mut merged = master_xml.to_string();
    for diff in &preview.field_diffs {
        let choice = resolution_map
            .get(&(diff.joint.clone(), diff.field.clone()))
            .copied()
            .unwrap_or(ResolutionChoice::Master);
        if choice != ResolutionChoice::Contributor {
            continue;
        }
        merged = rewrite_joint_field(&merged, &diff.joint, &diff.field, &diff.contributor_value)
            .map_err(|message| parse_error(path, message))?;
    }

    if preview.new_joints.is_empty() {
        return Ok(merged);
    }

    let master = load_urdf_from_str(master_xml, path)?;
    let master_links: HashSet<&str> = master.links.iter().map(|l| l.name.as_str()).collect();
    let contributor = load_urdf_from_str(contributor_xml, path)?;
    let links_to_copy = collect_links_for_joints(&contributor, &preview.new_joints);
    let mut blocks = Vec::new();
    for link_name in links_to_copy {
        if master_links.contains(link_name.as_str()) {
            continue;
        }
        let block = extract_xml_block(contributor_xml, "link", &link_name)
            .map_err(|message| parse_error(path, message))?;
        blocks.push(block);
    }
    for joint_name in &preview.new_joints {
        let block = extract_xml_block(contributor_xml, "joint", joint_name)
            .map_err(|message| parse_error(path, message))?;
        blocks.push(block);
    }
    insert_before_robot_close(&mut merged, &blocks)
        .map_err(|message| parse_error(path, message))?;
    Ok(merged)
}

/// Structural checks before promoting a merged URDF to live SoT.
pub fn validate_merged_urdf_xml(xml: &str) -> Result<(), ConfigError> {
    let path = Path::new("marengo.urdf");
    let robot = load_urdf_from_str(xml, path)?;
    let mut link_names = HashSet::new();
    for link in &robot.links {
        if !link_names.insert(link.name.as_str()) {
            return Err(parse_error(
                path,
                format!("duplicate link name {}", link.name),
            ));
        }
    }
    let mut joint_names = HashSet::new();
    for joint in &robot.joints {
        if !joint_names.insert(joint.name.as_str()) {
            return Err(parse_error(
                path,
                format!("duplicate joint name {}", joint.name),
            ));
        }
        if !link_names.contains(joint.parent.link.as_str()) {
            return Err(parse_error(
                path,
                format!(
                    "joint {} parent link {} missing",
                    joint.name, joint.parent.link
                ),
            ));
        }
        if !link_names.contains(joint.child.link.as_str()) {
            return Err(parse_error(
                path,
                format!(
                    "joint {} child link {} missing",
                    joint.name, joint.child.link
                ),
            ));
        }
        let axis = joint.axis.xyz.0;
        let norm = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if !norm.is_finite() || norm < 1e-9 {
            return Err(parse_error(
                path,
                format!("joint {} has zero/non-finite axis", joint.name),
            ));
        }
        if matches!(joint.joint_type, JointType::Revolute | JointType::Prismatic) {
            let lower = joint.limit.lower;
            let upper = joint.limit.upper;
            if lower.is_finite() && upper.is_finite() && lower > upper {
                return Err(parse_error(
                    path,
                    format!(
                        "joint {} has inverted limits lower={lower} > upper={upper}",
                        joint.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn load_urdf_from_str(xml: &str, path: &Path) -> Result<urdf_rs::Robot, ConfigError> {
    // Unique per call so parallel tests do not race on a pid-only temp path.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!(
        "marengo-merge-{}-{}-{}.urdf",
        std::process::id(),
        nanos,
        seq
    ));
    std::fs::write(&tmp, xml).map_err(|e| ConfigError::Io {
        path: tmp.clone(),
        message: e.to_string(),
    })?;
    let robot = load_urdf(&tmp).map_err(|e| parse_error(path, e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(robot)
}

fn collect_links_for_joints(robot: &urdf_rs::Robot, joints: &[String]) -> Vec<String> {
    let mut links = HashSet::new();
    for joint_name in joints {
        if let Some(joint) = robot.joints.iter().find(|j| j.name == *joint_name) {
            links.insert(joint.parent.link.clone());
            links.insert(joint.child.link.clone());
        }
    }
    links.into_iter().collect()
}

fn rewrite_joint_field(
    xml: &str,
    joint: &str,
    field: &str,
    contributor_value: &str,
) -> Result<String, String> {
    match field {
        "parent" => rewrite_parent_link(xml, joint, contributor_value),
        "type" => rewrite_joint_type(xml, joint, contributor_value),
        "origin" => rewrite_origin(xml, joint, contributor_value),
        "axis" => rewrite_axis(xml, joint, contributor_value),
        "limit_lower" => rewrite_limit_attr(xml, joint, "lower", contributor_value),
        "limit_upper" => rewrite_limit_attr(xml, joint, "upper", contributor_value),
        other => Err(format!("unsupported merge field {other}")),
    }
}

fn rewrite_parent_link(xml: &str, joint: &str, parent: &str) -> Result<String, String> {
    let block = extract_joint_block(xml, joint)?;
    let updated = rewrite_tag_string_attr(&block, "<parent", "link", parent)?;
    replace_joint_block(xml, joint, &updated)
}

fn rewrite_joint_type(xml: &str, joint: &str, joint_type: &str) -> Result<String, String> {
    let block = extract_joint_block(xml, joint)?;
    let open = block
        .find("<joint")
        .ok_or_else(|| format!("joint open tag missing for {joint}"))?;
    let rel = &block[open..];
    let end = rel
        .find('>')
        .ok_or_else(|| format!("joint open unclosed for {joint}"))?;
    let mut tag = rel[..=end].to_string();
    if let Some(type_idx) = tag.find("type=\"") {
        let start = type_idx + "type=\"".len();
        let rest = &tag[start..];
        let close = rest
            .find('"')
            .ok_or_else(|| format!("type attr unclosed for {joint}"))?;
        tag = format!("{}{}{}", &tag[..start], joint_type, &rest[close..]);
    } else {
        tag = tag.replace('>', &format!(" type=\"{joint_type}\">"));
    }
    // Preserve the remainder of the joint block after the opening tag (parent/child/…).
    let updated = format!("{}{}{}", &block[..open], tag, &rel[end + 1..]);
    replace_joint_block(xml, joint, &updated)
}

fn rewrite_origin(xml: &str, joint: &str, contributor_value: &str) -> Result<String, String> {
    let (xyz, rpy) = parse_origin_values(contributor_value)?;
    let block = extract_joint_block(xml, joint)?;
    let origin_start = block
        .find("<origin")
        .ok_or_else(|| format!("origin tag missing for {joint}"))?;
    let rel = &block[origin_start..];
    let end = rel
        .find("/>")
        .or_else(|| rel.find('>'))
        .ok_or_else(|| format!("origin tag unclosed for {joint}"))?;
    let mut tag = rel[..end].to_string();
    tag = rewrite_vec3_attr(&tag, "xyz", &xyz)?;
    tag = rewrite_vec3_attr(&tag, "rpy", &rpy)?;
    let updated = format!("{}{}{}", &block[..origin_start], tag, &rel[end..]);
    replace_joint_block(xml, joint, &updated)
}

fn rewrite_axis(xml: &str, joint: &str, contributor_value: &str) -> Result<String, String> {
    let values = parse_vec3_bracket(contributor_value)?;
    let block = extract_joint_block(xml, joint)?;
    let axis_start = block
        .find("<axis")
        .ok_or_else(|| format!("axis tag missing for {joint}"))?;
    let rel = &block[axis_start..];
    let end = rel
        .find("/>")
        .or_else(|| rel.find('>'))
        .ok_or_else(|| format!("axis tag unclosed for {joint}"))?;
    let mut tag = rel[..end].to_string();
    tag = rewrite_vec3_attr(&tag, "xyz", &values)?;
    let updated = format!("{}{}{}", &block[..axis_start], tag, &rel[end..]);
    replace_joint_block(xml, joint, &updated)
}

fn rewrite_limit_attr(
    xml: &str,
    joint: &str,
    attr: &str,
    contributor_value: &str,
) -> Result<String, String> {
    let value = contributor_value
        .parse::<f64>()
        .map_err(|_| format!("invalid limit value for {joint}.{attr}"))?;
    let block = extract_joint_block(xml, joint)?;
    let limit_start = block
        .find("<limit")
        .ok_or_else(|| format!("limit tag missing for {joint}"))?;
    let rel = &block[limit_start..];
    let end = rel
        .find("/>")
        .or_else(|| rel.find('>'))
        .ok_or_else(|| format!("limit tag unclosed for {joint}"))?;
    let mut tag = rel[..end].to_string();
    tag = replace_numeric_attr(&tag, attr, value)?;
    let updated = format!("{}{}{}", &block[..limit_start], tag, &rel[end..]);
    replace_joint_block(xml, joint, &updated)
}

fn parse_origin_values(text: &str) -> Result<(Vec<f64>, Vec<f64>), String> {
    let xyz_part = text
        .split("xyz=")
        .nth(1)
        .ok_or_else(|| "origin value missing xyz".to_string())?;
    let rpy_part = text
        .split("rpy=")
        .nth(1)
        .ok_or_else(|| "origin value missing rpy".to_string())?;
    let xyz = parse_vec3_bracket(xyz_part)?;
    let rpy = parse_vec3_bracket(rpy_part)?;
    Ok((xyz, rpy))
}

fn parse_vec3_bracket(text: &str) -> Result<Vec<f64>, String> {
    let start = text.find('[').ok_or_else(|| "vec3 missing [".to_string())?;
    let end = text.find(']').ok_or_else(|| "vec3 missing ]".to_string())?;
    let inner = &text[start + 1..end];
    let parts: Vec<f64> = inner
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid vec3 {inner}"))?;
    if parts.len() != 3 {
        return Err(format!("vec3 expected 3 values in {inner}"));
    }
    Ok(parts)
}

fn rewrite_vec3_attr(tag: &str, name: &str, values: &[f64]) -> Result<String, String> {
    let key = format!("{name}=\"");
    let idx = tag
        .find(&key)
        .ok_or_else(|| format!("attribute {name} missing"))?;
    let start = idx + key.len();
    let rest = &tag[start..];
    let end = rest
        .find('"')
        .ok_or_else(|| format!("attribute {name} unclosed"))?;
    let formatted = format!("{:.6} {:.6} {:.6}", values[0], values[1], values[2]);
    let mut out = String::with_capacity(tag.len() + 16);
    out.push_str(&tag[..start]);
    out.push_str(&formatted);
    out.push_str(&rest[end..]);
    Ok(out)
}

fn rewrite_tag_string_attr(
    tag: &str,
    tag_open: &str,
    attr: &str,
    value: &str,
) -> Result<String, String> {
    let start = tag
        .find(tag_open)
        .ok_or_else(|| format!("{tag_open} tag missing"))?;
    let rel = &tag[start..];
    let end = rel
        .find("/>")
        .or_else(|| rel.find('>'))
        .ok_or_else(|| format!("{tag_open} tag unclosed"))?;
    let mut fragment = rel[..end].to_string();
    let key = format!("{attr}=\"");
    if let Some(idx) = fragment.find(&key) {
        let value_start = idx + key.len();
        let rest = &fragment[value_start..];
        let close = rest
            .find('"')
            .ok_or_else(|| format!("attribute {attr} unclosed"))?;
        fragment = format!("{}{}{}", &fragment[..value_start], value, &rest[close..]);
    } else {
        fragment.push_str(&format!(" {attr}=\"{value}\""));
    }
    Ok(format!("{}{}{}", &tag[..start], fragment, &rel[end..]))
}

fn replace_numeric_attr(tag: &str, name: &str, value: f64) -> Result<String, String> {
    let key = format!("{name}=\"");
    let idx = tag
        .find(&key)
        .ok_or_else(|| format!("attribute {name} missing"))?;
    let value_start = idx + key.len();
    let rest = &tag[value_start..];
    let end = rest
        .find('"')
        .ok_or_else(|| format!("attribute {name} unclosed"))?;
    let mut out = String::with_capacity(tag.len() + 8);
    out.push_str(&tag[..value_start]);
    out.push_str(&format_limit_value(value));
    out.push_str(&rest[end..]);
    Ok(out)
}

fn format_limit_value(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn extract_joint_block(xml: &str, joint: &str) -> Result<String, String> {
    extract_xml_block(xml, "joint", joint)
}

fn extract_xml_block(xml: &str, kind: &str, name: &str) -> Result<String, String> {
    let open = format!("<{kind}");
    let marker = format!("name=\"{name}\"");
    let name_idx = xml
        .find(&marker)
        .ok_or_else(|| format!("{kind} {name} not found"))?;
    let start = xml[..name_idx]
        .rfind(&open)
        .ok_or_else(|| format!("{kind} open tag missing for {name}"))?;
    let after = &xml[name_idx..];
    let close_tag = format!("</{kind}>");
    let rel_end = after
        .find(&close_tag)
        .ok_or_else(|| format!("{kind} close tag missing for {name}"))?;
    let end = name_idx + rel_end + close_tag.len();
    Ok(xml[start..end].to_string())
}

fn replace_joint_block(xml: &str, joint: &str, replacement: &str) -> Result<String, String> {
    let block = extract_joint_block(xml, joint)?;
    let start = xml
        .find(&block)
        .ok_or_else(|| format!("joint block missing for {joint}"))?;
    let mut out = String::with_capacity(xml.len() + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[start + block.len()..]);
    Ok(out)
}

fn insert_before_robot_close(xml: &mut String, blocks: &[String]) -> Result<(), String> {
    let close = "</robot>";
    let idx = xml
        .rfind(close)
        .ok_or_else(|| "robot close tag missing".to_string())?;
    let mut insert = String::new();
    for block in blocks {
        insert.push_str(block);
        if !block.ends_with('\n') {
            insert.push('\n');
        }
    }
    xml.insert_str(idx, &insert);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::fs;

    use super::*;
    use crate::resolve_repo_root;

    fn axis_conflict_fixture() -> (String, String) {
        let root = resolve_repo_root();
        let master = fs::read_to_string(root.join("assets/urdf/marengo.urdf")).expect("master");
        let block = extract_joint_block(&master, "right_shoulder_pitch").expect("pitch block");
        let new_block = block.replace("xyz=\"0 1 0\"", "xyz=\"0 0 1\"");
        let contributor =
            replace_joint_block(&master, "right_shoulder_pitch", &new_block).expect("contributor");
        (master, contributor)
    }

    #[test]
    fn merge_preview_detects_axis_conflict() {
        let (master, contributor) = axis_conflict_fixture();
        let master_robot = load_urdf_from_str(&master, Path::new("master.urdf")).expect("master");
        let contributor_robot =
            load_urdf_from_str(&contributor, Path::new("contributor.urdf")).expect("contributor");
        let preview = merge_preview_from_robots(&master_robot, &contributor_robot);
        assert!(
            preview
                .field_diffs
                .iter()
                .any(|d| d.joint == "right_shoulder_pitch"
                    && d.field == "axis"
                    && d.kinematics_critical),
            "expected axis diff: {:?}",
            preview.field_diffs
        );
    }

    #[test]
    fn simulate_merge_requires_critical_resolution() {
        let (master, contributor) = axis_conflict_fixture();
        let err = simulate_merge_xml(&master, &contributor, &[]).expect_err("unresolved");
        assert!(
            err.to_string().contains("kinematics-critical"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn simulate_merge_applies_contributor_axis_pick() {
        let (master, contributor) = axis_conflict_fixture();
        let preview = merge_preview_from_robots(
            &load_urdf_from_str(&master, Path::new("m.urdf")).expect("m"),
            &load_urdf_from_str(&contributor, Path::new("c.urdf")).expect("c"),
        );
        let axis_diff = preview
            .field_diffs
            .iter()
            .find(|d| d.joint == "right_shoulder_pitch" && d.field == "axis")
            .expect("axis diff");
        let resolutions = vec![FieldResolution {
            joint: axis_diff.joint.clone(),
            field: axis_diff.field.clone(),
            choice: ResolutionChoice::Contributor,
        }];
        let merged = simulate_merge_xml(&master, &contributor, &resolutions).expect("merged");
        let merged_robot = load_urdf_from_str(&merged, Path::new("merged.urdf")).expect("parse");
        let pitch = merged_robot
            .joints
            .iter()
            .find(|j| j.name == "right_shoulder_pitch")
            .expect("pitch");
        assert!((pitch.axis.xyz.0[2] - 1.0).abs() < EPS);
    }

    #[test]
    fn merge_preview_lists_new_joints_from_contributor() {
        let root = resolve_repo_root();
        let master = fs::read_to_string(root.join("assets/urdf/marengo.urdf")).expect("master");
        let contributor = fs::read_to_string(
            root.join("assets/urdf/archive/seed-arm_3dof_right/contributor.urdf"),
        )
        .expect("3dof");
        let preview = merge_preview_from_paths(
            root.join("assets/urdf/marengo.urdf"),
            root.join("assets/urdf/archive/seed-arm_3dof_right/contributor.urdf"),
        )
        .expect("preview");
        assert!(preview
            .overlapping_joints
            .contains(&"right_shoulder_pitch".to_string()));
        assert!(preview.new_joints.is_empty());
        let master_robot = load_urdf_from_str(&master, Path::new("m.urdf")).expect("m");
        let contributor_robot = load_urdf_from_str(&contributor, Path::new("c.urdf")).expect("c");
        let trimmed_preview = merge_preview_from_robots(&master_robot, &contributor_robot);
        assert_eq!(trimmed_preview.new_joints.len(), 0);
    }

    #[test]
    fn rewrite_joint_type_preserves_joint_body() {
        let (master, mut contributor) = axis_conflict_fixture();
        // Force a type conflict by rewriting contributor pitch to continuous.
        contributor = contributor.replace(
            "<joint name=\"right_shoulder_pitch\" type=\"revolute\">",
            "<joint name=\"right_shoulder_pitch\" type=\"continuous\">",
        );
        let preview = merge_preview_from_robots(
            &load_urdf_from_str(&master, Path::new("m.urdf")).expect("m"),
            &load_urdf_from_str(&contributor, Path::new("c.urdf")).expect("c"),
        );
        assert!(
            preview
                .field_diffs
                .iter()
                .any(|d| d.joint == "right_shoulder_pitch" && d.field == "type"),
            "expected type diff: {:?}",
            preview.field_diffs
        );
        let resolutions: Vec<FieldResolution> = preview
            .field_diffs
            .iter()
            .filter(|d| d.kinematics_critical)
            .map(|d| FieldResolution {
                joint: d.joint.clone(),
                field: d.field.clone(),
                choice: if d.field == "type" {
                    ResolutionChoice::Contributor
                } else {
                    ResolutionChoice::Master
                },
            })
            .collect();
        let merged = simulate_merge_xml(&master, &contributor, &resolutions).expect("merged");
        let robot = load_urdf_from_str(&merged, Path::new("merged.urdf")).expect("parse");
        let pitch = robot
            .joints
            .iter()
            .find(|j| j.name == "right_shoulder_pitch")
            .expect("pitch joint present with parent/child/axis");
        assert_eq!(pitch.parent.link, "base_link");
        assert!(!pitch.child.link.is_empty());
        assert!(matches!(pitch.joint_type, JointType::Continuous));
    }

    #[test]
    fn validate_merged_urdf_rejects_duplicate_link() {
        let bad = r#"<?xml version="1.0"?>
<robot name="dup">
  <link name="base_link"/>
  <link name="base_link"/>
  <link name="child"/>
  <joint name="j1" type="revolute">
    <parent link="base_link"/>
    <child link="child"/>
    <origin xyz="0 0 0" rpy="0 0 0"/>
    <axis xyz="0 0 1"/>
    <limit lower="0" upper="1" effort="1" velocity="1"/>
  </joint>
</robot>
"#;
        let err = validate_merged_urdf_xml(bad).expect_err("dup link");
        assert!(err.to_string().contains("duplicate link"), "{err}");
    }
}
