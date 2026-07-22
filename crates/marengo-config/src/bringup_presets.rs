//! Bringup profile allowlist and Consul preset ↔ slug registry.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::{resolve_repo_root, ConfigError};

/// Consul inventory preset ids that map to bringup directories.
pub const BRINGUP_PRESET_IDS: &[&str] = &["bench_3dof", "bench_4dof"];

/// Allowlisted bringup directory names under `config/bringup/`.
pub const BRINGUP_PROFILE_SLUGS: &[&str] = &[
    "arm_3dof_right",
    "arm_4dof_right",
    "arm_4dof_left",
    "shoulder_pitch_dual",
    "shoulder_pitch_left_only",
    "shoulder_pitch_weighted",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetProfileMapping {
    pub preset_id: &'static str,
    pub profile_slug: &'static str,
}

/// Server-owned preset → profile map (single source of truth for Consul).
pub const PRESET_PROFILE_MAP: &[PresetProfileMapping] = &[
    PresetProfileMapping {
        preset_id: "bench_3dof",
        profile_slug: "arm_3dof_right",
    },
    PresetProfileMapping {
        preset_id: "bench_4dof",
        profile_slug: "arm_4dof_right",
    },
];

pub fn profile_slug_for_preset(preset_id: &str) -> Option<&'static str> {
    PRESET_PROFILE_MAP
        .iter()
        .find(|m| m.preset_id == preset_id)
        .map(|m| m.profile_slug)
}

pub fn preset_id_for_profile(profile_slug: &str) -> Option<&'static str> {
    PRESET_PROFILE_MAP
        .iter()
        .find(|m| m.profile_slug == profile_slug)
        .map(|m| m.preset_id)
}

pub fn is_allowlisted_slug(slug: &str) -> bool {
    BRINGUP_PROFILE_SLUGS.contains(&slug)
}

/// Resolve `config/bringup/<slug>` under repo root. Rejects path segments / traversal.
pub fn resolve_bringup_dir(
    repo_root: impl AsRef<Path>,
    slug: &str,
) -> Result<PathBuf, ConfigError> {
    if slug.is_empty()
        || slug.contains('/')
        || slug.contains('\\')
        || slug.contains("..")
        || slug.starts_with('.')
        || !is_allowlisted_slug(slug)
    {
        return Err(ConfigError::Parse {
            path: PathBuf::from("config/bringup"),
            message: format!("bringup profile slug not allowlisted: {slug}"),
        });
    }
    let root = repo_root.as_ref();
    let bringup_root = root.join("config/bringup");
    let candidate = bringup_root.join(slug);
    let canon_root = std::fs::canonicalize(&bringup_root).map_err(|e| ConfigError::Io {
        path: bringup_root.clone(),
        message: e.to_string(),
    })?;
    let canon = std::fs::canonicalize(&candidate).map_err(|e| ConfigError::Io {
        path: candidate.clone(),
        message: e.to_string(),
    })?;
    if !canon.starts_with(&canon_root) || canon.parent() != Some(canon_root.as_path()) {
        return Err(ConfigError::Parse {
            path: candidate,
            message: "bringup path escaped allowlisted root".to_string(),
        });
    }
    Ok(canon)
}

pub fn bringup_dir_default_repo(slug: &str) -> Result<PathBuf, ConfigError> {
    resolve_bringup_dir(resolve_repo_root(), slug)
}

/// Content hash of robot/motors/control/homing YAML for CAS.
pub fn profile_content_revision(config_dir: impl AsRef<Path>) -> Result<String, ConfigError> {
    let dir = config_dir.as_ref();
    let mut hasher = DefaultHasher::new();
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).map_err(|e| ConfigError::Io {
            path: path.clone(),
            message: e.to_string(),
        })?;
        name.hash(&mut hasher);
        bytes.hash(&mut hasher);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Prefer active profile's mapped preset when the joint is a member; else first mapped membership.
pub fn derive_preset_label_for_joint(
    joint: &str,
    active_slug: &str,
    membership_slugs: &[String],
) -> Option<&'static str> {
    let _ = joint;
    if membership_slugs.iter().any(|s| s == active_slug) {
        if let Some(id) = preset_id_for_profile(active_slug) {
            return Some(id);
        }
    }
    for slug in membership_slugs {
        if let Some(id) = preset_id_for_profile(slug) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn allowlist_rejects_traversal() {
        let root = resolve_repo_root();
        assert!(resolve_bringup_dir(&root, "../etc").is_err());
        assert!(resolve_bringup_dir(&root, "arm_3dof_right/../arm_4dof_right").is_err());
        assert!(resolve_bringup_dir(&root, "not_a_real_profile").is_err());
    }

    #[test]
    fn allowlist_accepts_arm_4dof() {
        let root = resolve_repo_root();
        let dir = resolve_bringup_dir(&root, "arm_4dof_right").expect("slug");
        assert!(dir.join("motors.yaml").is_file());
    }

    #[test]
    fn preset_map_roundtrip() {
        assert_eq!(
            profile_slug_for_preset("bench_3dof"),
            Some("arm_3dof_right")
        );
        assert_eq!(preset_id_for_profile("arm_4dof_right"), Some("bench_4dof"));
        assert_eq!(profile_slug_for_preset("golden_pose"), None);
    }

    #[test]
    fn revision_stable_for_same_files() {
        let root = resolve_repo_root();
        let dir = resolve_bringup_dir(&root, "arm_3dof_right").unwrap();
        let a = profile_content_revision(&dir).unwrap();
        let b = profile_content_revision(&dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn derive_prefers_active() {
        let membership = vec!["arm_3dof_right".to_string(), "arm_4dof_right".to_string()];
        assert_eq!(
            derive_preset_label_for_joint("right_shoulder_roll", "arm_4dof_right", &membership),
            Some("bench_4dof")
        );
    }
}
