//! Headless simulation harness for Marengo integration tests.
//!
//! MuJoCo stepping is run via `sim/scripts/smoke_test.py` in CI (`check-sim`),
//! which smokes both the minimal fixture and the production
//! `assets/mjcf/marengo.xml`. This crate holds Rust-side URDF↔MJCF parity
//! checks. It is the reserved home for an in-process plant (L-berthier-20):
//! drive it through `davout::simulation::SimulationBus`, never through a
//! second CAN path.
//!
//! The MJCF helpers below are dependency-free XML attribute scans (not a full
//! MuJoCo parser): they understand `<joint>` / `<body>` / `<inertial>` tags,
//! XML comments, and one level of `<default class="…">` joint-type defaults.
//! Anything fancier belongs behind a real MJCF dependency, not string matching.

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
use armee_kinematics::fixtures;

// Test-only fixture helpers (P-sim-harness-01): no production consumer.
#[cfg(test)]
/// Default MJCF path for CI fixture tests.
fn default_model_path() -> PathBuf {
    fixtures::minimal_mjcf()
}

#[cfg(test)]
/// Production MJCF path (`assets/mjcf/marengo.xml`).
fn production_model_path() -> PathBuf {
    fixtures::production_mjcf()
}

#[cfg(test)]
fn model_exists(path: &Path) -> bool {
    path.is_file()
}

/// Remove `<!-- … -->` comments so commented-out tags are never parsed.
#[cfg(test)]
fn strip_xml_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = &rest[start + "<!--".len()..];
        if let Some(end) = rest.find("-->") {
            rest = &rest[end + "-->".len()..];
        } else {
            // Unterminated comment: drop the tail rather than parsing it.
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Attribute map of a single XML open tag (`<joint …>` / `<body …>` / …).
#[cfg(test)]
fn tag_attrs(tag: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip the tag name and any separators.
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() && bytes[i] != b'_' {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
        {
            i += 1;
        }
        if key_start == i {
            break;
        }
        let key = &tag[key_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
        } else {
            continue;
        }
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
            let q = bytes[i];
            i += 1;
            Some(q)
        } else {
            None
        };
        let val_start = i;
        if let Some(q) = quote {
            while i < bytes.len() && bytes[i] != q {
                i += 1;
            }
            if let Some(value) = tag.get(val_start..i) {
                attrs.insert(key.to_string(), value.to_string());
            }
            i += 1;
        } else {
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            if let Some(value) = tag.get(val_start..i) {
                attrs.insert(key.to_string(), value.to_string());
            }
        }
    }
    attrs
}

/// Find every opening tag named `name` (`<joint …>`, `<body …>`), comment-free.
/// Returns the raw tag text including brackets. `<jointfoo>` does not match.
#[cfg(test)]
fn find_tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut tags = Vec::new();
    let mut rest = xml;
    let open = format!("<{name}");
    while let Some(start) = rest.find(open.as_str()) {
        let after = start + open.len();
        let boundary = rest[after..].chars().next().unwrap_or('>');
        if !(boundary.is_ascii_whitespace() || boundary == '>' || boundary == '/') {
            rest = &rest[after..];
            continue;
        }
        let tag_rest = &rest[start..];
        let Some(end) = tag_rest.find('>') else {
            break;
        };
        tags.push(&tag_rest[..=end]);
        rest = &tag_rest[end + 1..];
    }
    tags
}

#[cfg(test)]
fn parse_f64_list(value: &str, expect: usize) -> Option<Vec<f64>> {
    let nums: Option<Vec<f64>> = value.split_whitespace().map(|s| s.parse().ok()).collect();
    let nums = nums?;
    (nums.len() == expect).then_some(nums)
}

/// One MJCF `<joint>` with its class defaults resolved.
#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
struct MjcfJoint {
    /// Joint name (MuJoCo requires it when the model has more than one joint).
    pub name: String,
    /// Joint type after default resolution (`hinge` when unset — the MuJoCo default).
    pub joint_type: String,
    /// Hinge/slide axis; `[0.0; 3]` when the attribute is absent.
    pub axis: [f64; 3],
    /// `range="lo hi"` in radians, when present.
    pub range: Option<(f64, f64)>,
}

/// Joint-type defaults from `<default>` blocks: `(main_default, class_defaults)`.
#[cfg(test)]
fn joint_type_defaults(xml: &str) -> (Option<String>, HashMap<String, String>) {
    let mut main: Option<String> = None;
    let mut classes: HashMap<String, String> = HashMap::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<default") {
        let after = start + "<default".len();
        let boundary = rest[after..].chars().next().unwrap_or('>');
        if !(boundary.is_ascii_whitespace() || boundary == '>') {
            rest = &rest[after..];
            continue;
        }
        let block_open = &rest[start..];
        let Some(open_end) = block_open.find('>') else {
            break;
        };
        let attrs = tag_attrs(&block_open[..=open_end]);
        let after_open = &block_open[open_end + 1..];
        // One level only: the first nested <joint> sets this block's default,
        // the first </default> ends it.
        let joint_type = find_tags(after_open, "joint")
            .into_iter()
            .next()
            .and_then(|t| tag_attrs(t).remove("type"));
        match (attrs.get("class"), joint_type) {
            (Some(class), Some(joint_type)) => {
                classes.insert(class.clone(), joint_type);
            }
            (None, Some(joint_type)) => main = Some(joint_type),
            _ => {}
        }
        let Some(close) = after_open.find("</default>") else {
            break;
        };
        rest = &after_open[close + "</default>".len()..];
    }
    (main, classes)
}

/// Parse every MJCF `<joint>` (comments stripped, class defaults resolved).
#[cfg(test)]
fn mjcf_joints(xml: &str) -> Vec<MjcfJoint> {
    let clean = strip_xml_comments(xml);
    let (main_default, class_defaults) = joint_type_defaults(&clean);
    find_tags(&clean, "joint")
        .into_iter()
        .filter_map(|tag| {
            let attrs = tag_attrs(tag);
            let name = attrs.get("name")?.clone();
            let joint_type = attrs
                .get("type")
                .cloned()
                .or_else(|| {
                    attrs
                        .get("class")
                        .and_then(|class| class_defaults.get(class).cloned())
                })
                .or_else(|| main_default.clone())
                .unwrap_or_else(|| "hinge".to_string());
            let axis = attrs
                .get("axis")
                .and_then(|a| parse_f64_list(a, 3))
                .map(|v| [v[0], v[1], v[2]])
                .unwrap_or([0.0; 3]);
            let range = attrs
                .get("range")
                .and_then(|r| parse_f64_list(r, 2))
                .map(|v| (v[0], v[1]));
            Some(MjcfJoint {
                name,
                joint_type,
                axis,
                range,
            })
        })
        .collect()
}

/// Count actuated hinge joints: explicit `type="hinge"` plus joints that fall
/// back to the MuJoCo hinge default (directly or through a `<default>` class).
#[cfg(test)]
fn count_mjcf_hinge_joints(xml: &str) -> usize {
    mjcf_joints(xml)
        .iter()
        .filter(|j| j.joint_type == "hinge")
        .count()
}

/// `(body name, inertial mass)` for every `<body>` with an `<inertial mass="…">`.
#[cfg(test)]
fn mjcf_body_masses(xml: &str) -> Vec<(String, f64)> {
    let clean = strip_xml_comments(xml);
    let mut out = Vec::new();
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut rest = clean.as_str();
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt..];
        if rest.starts_with("<!--") {
            // strip_xml_comments already removed these; defensive skip.
            rest = &rest["<!--".len()..];
            continue;
        }
        if rest.starts_with("</") {
            if rest.starts_with("</body") {
                stack.pop();
            }
            rest = match rest.find('>') {
                Some(end) => &rest[end + 1..],
                None => break,
            };
            continue;
        }
        let end = match rest.find('>') {
            Some(end) => end,
            None => break,
        };
        let tag = &rest[..=end];
        let self_closing = tag.ends_with("/>");
        if tag.starts_with("<body") {
            // Self-closing bodies cannot hold an <inertial> block.
            if !self_closing {
                stack.push(tag_attrs(tag).get("name").cloned());
            }
        } else if tag.starts_with("<inertial") {
            let attrs = tag_attrs(tag);
            if let (Some(top), Some(mass)) = (stack.last(), attrs.get("mass")) {
                if let (Some(name), Ok(mass)) = (top.clone(), mass.parse::<f64>()) {
                    out.push((name, mass));
                }
            }
        }
        rest = &rest[end + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]

    use super::*;
    use armee_kinematics::{actuated_joint_count, load_urdf};

    #[test]
    fn default_fixture_exists() {
        let p = default_model_path();
        assert!(model_exists(&p), "missing {p:?} — run from repo root");
    }

    #[test]
    fn urdf_and_mjcf_fixture_dof_match() {
        let robot = load_urdf(fixtures::minimal_urdf()).expect("urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(fixtures::minimal_mjcf()).expect("mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(
            urdf_dof, mjcf_dof,
            "URDF actuated joints ({urdf_dof}) must match MJCF hinges ({mjcf_dof})"
        );
    }

    #[test]
    fn production_urdf_and_mjcf_dof_match() {
        let urdf_path = fixtures::production_urdf();
        let mjcf_path = production_model_path();
        assert!(model_exists(&urdf_path), "missing {urdf_path:?}");
        assert!(model_exists(&mjcf_path), "missing {mjcf_path:?}");
        let robot = load_urdf(&urdf_path).expect("production urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(&mjcf_path).expect("production mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(urdf_dof, mjcf_dof, "production URDF/MJCF DOF mismatch");
    }

    /// Production parity beyond counts (L-sim-harness-03): the runtime keys by
    /// name, so names, hinge axes and ranges must match the URDF joint by
    /// joint. Ranges must sit inside the URDF hard limits — the sim must never
    /// admit a pose the hardware calls out of limits.
    #[test]
    fn production_urdf_and_mjcf_names_axes_and_ranges_match() {
        let robot = load_urdf(fixtures::production_urdf()).expect("production urdf");
        let mjcf = std::fs::read_to_string(production_model_path()).expect("production mjcf");
        let joints = mjcf_joints(&mjcf);
        let hinges: Vec<&MjcfJoint> = joints.iter().filter(|j| j.joint_type == "hinge").collect();
        assert_eq!(
            hinges.len(),
            joints.len(),
            "production MJCF has non-hinge joints: {:?}",
            joints
                .iter()
                .filter(|j| j.joint_type != "hinge")
                .map(|j| (&j.name, &j.joint_type))
                .collect::<Vec<_>>()
        );
        for joint in &robot.joints {
            let Some(m) = hinges.iter().find(|j| j.name == joint.name) else {
                panic!("URDF joint {} missing from production MJCF", joint.name);
            };
            for k in 0..3 {
                let (a, b) = (joint.axis.xyz[k], m.axis[k]);
                assert!(
                    (a - b).abs() < 1e-9,
                    "axis drift on {}: URDF {:?} vs MJCF {:?}",
                    joint.name,
                    joint.axis.xyz,
                    m.axis
                );
            }
            let Some((lo, hi)) = m.range else {
                panic!(
                    "MJCF joint {} has no range (unbounded sim joint)",
                    joint.name
                );
            };
            assert!(
                lo >= joint.limit.lower - 1e-9 && hi <= joint.limit.upper + 1e-9,
                "MJCF range [{lo}, {hi}] on {} escapes URDF hard [{}, {}]",
                joint.name,
                joint.limit.lower,
                joint.limit.upper
            );
        }
        let mut mjcf_names: Vec<&str> = hinges.iter().map(|j| j.name.as_str()).collect();
        mjcf_names.sort_unstable();
        let mut urdf_names: Vec<&str> = robot.joints.iter().map(|j| j.name.as_str()).collect();
        urdf_names.sort_unstable();
        assert_eq!(
            mjcf_names, urdf_names,
            "production MJCF/URDF joint sets differ"
        );
    }

    /// Production mass parity (L-sim-harness-05): every MJCF body inertial mass
    /// must equal the URDF link mass, so sim gravity matches the dynamics model.
    #[test]
    fn production_mjcf_body_masses_match_urdf() {
        let robot = load_urdf(fixtures::production_urdf()).expect("production urdf");
        let mjcf = std::fs::read_to_string(production_model_path()).expect("production mjcf");
        let masses = mjcf_body_masses(&mjcf);
        assert!(
            !masses.is_empty(),
            "production MJCF has no <inertial> masses (sim gravity is MuJoCo-inferred, not CAD)"
        );
        for link in &robot.links {
            let Some((_, mass)) = masses.iter().find(|(name, _)| name == &link.name) else {
                panic!("URDF link {} has no MJCF inertial mass", link.name);
            };
            assert!(
                (mass - link.inertial.mass.value).abs() < 1e-9,
                "mass drift on {}: URDF {} vs MJCF {mass}",
                link.name,
                link.inertial.mass.value
            );
        }
    }

    /// A commented-out joint is not a joint (L-sim-harness-01).
    #[test]
    fn commented_joints_are_not_counted() {
        let xml = "<mujoco><worldbody><!-- <joint name=\"ghost\" type=\"hinge\"/> --></worldbody></mujoco>";
        assert_eq!(count_mjcf_hinge_joints(xml), 0);
        assert!(mjcf_joints(xml).is_empty());
    }

    /// Non-hinge types are excluded from the hinge count (L-sim-harness-01).
    #[test]
    fn slide_joints_are_not_hinges() {
        let xml = "<mujoco><worldbody><body><joint name=\"a\" type=\"hinge\"/><joint name=\"b\" type=\"slide\"/></body></worldbody></mujoco>";
        assert_eq!(count_mjcf_hinge_joints(xml), 1);
        assert_eq!(mjcf_joints(xml).len(), 2);
    }

    /// Class and main `<default>` joint types resolve; the MuJoCo default is
    /// hinge (L-sim-harness-01).
    #[test]
    fn default_class_joint_types_resolve() {
        let xml = "<mujoco><default><joint type=\"slide\"/></default>\
            <default class=\"arm\"><joint type=\"hinge\"/></default><worldbody><body>\
            <joint name=\"a\"/>\
            <joint name=\"b\" class=\"arm\"/>\
            <joint name=\"c\" class=\"arm\" type=\"slide\"/>\
            </body></worldbody></mujoco>";
        let joints = mjcf_joints(xml);
        let joint_type = |name: &str| {
            joints
                .iter()
                .find(|j| j.name == name)
                .expect("joint")
                .joint_type
                .clone()
        };
        assert_eq!(joint_type("a"), "slide");
        assert_eq!(joint_type("b"), "hinge");
        assert_eq!(joint_type("c"), "slide");
    }

    /// The elbow axis is checked on the elbow joint, not file-wide
    /// (L-sim-harness-02): a stray matching axis elsewhere must not satisfy it.
    #[test]
    fn elbow_axis_is_per_joint_not_file_wide() {
        let xml = "<mujoco><worldbody><body>\
            <joint name=\"right_shoulder_pitch\" type=\"hinge\" axis=\"0 1 0\"/>\
            <joint name=\"right_elbow_pitch\" type=\"hinge\" axis=\"0 0 1\"/>\
            </body></worldbody></mujoco>";
        let joints = mjcf_joints(xml);
        let elbow = joints
            .iter()
            .find(|j| j.name == "right_elbow_pitch")
            .expect("elbow");
        assert!(
            (elbow.axis[1] - 1.0).abs() > 1e-9,
            "fixture elbow is off-axis; per-joint check must see it: {elbow:?}"
        );
        assert!(
            xml.contains("axis=\"0 1 0\""),
            "fixture must still contain the axis string elsewhere (the old file-wide match would pass)"
        );
    }

    /// A renamed MJCF joint is a mismatch even when counts agree
    /// (L-sim-harness-03).
    #[test]
    fn renamed_mjcf_joint_breaks_name_parity() {
        let mjcf = std::fs::read_to_string(production_model_path()).expect("production mjcf");
        let renamed = mjcf.replacen("right_elbow_pitch", "elbow_typo", 1);
        let names: Vec<String> = mjcf_joints(&renamed)
            .iter()
            .map(|j| j.name.clone())
            .collect();
        assert!(
            !names.contains(&"right_elbow_pitch".to_string()),
            "renamed fixture must drop the joint name"
        );
        assert_eq!(
            count_mjcf_hinge_joints(&renamed),
            count_mjcf_hinge_joints(&mjcf),
            "counts still agree — a count-only check would miss the rename"
        );
    }
}
