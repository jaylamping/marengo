//! Persisted commissioning scope (`commissioning-scope.yaml`).
//!
//! Effective scope = persisted joints ∩ `MARENGO_JOINT_SUBSET` ceiling (when set).
//! Writes use temp + rename. Unknown joint names are rejected before persist.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ConfigError;

/// Document version written by this crate.
pub const COMMISSIONING_SCOPE_VERSION: u32 = 1;

/// On-disk commissioning scope (canonical joint names only — never limb aliases).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommissioningScopeFile {
    pub version: u32,
    pub joints: Vec<String>,
}

impl CommissioningScopeFile {
    /// Normalize: unique, sorted, non-empty names.
    pub fn normalized(joints: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut set: HashSet<String> = HashSet::new();
        for joint in joints {
            let name = joint.into();
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                set.insert(trimmed.to_string());
            }
        }
        let mut joints: Vec<String> = set.into_iter().collect();
        joints.sort();
        Self {
            version: COMMISSIONING_SCOPE_VERSION,
            joints,
        }
    }
}

/// Default Pi path: `$MARENGO_ROOT/var/commissioning-scope.yaml` or `/opt/marengo/...`.
pub fn default_commissioning_scope_path() -> PathBuf {
    let root = std::env::var("MARENGO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/marengo"));
    root.join("var/commissioning-scope.yaml")
}

/// Load scope from disk. Missing file → `Ok(None)`. Corrupt / bad version → error.
pub fn load_commissioning_scope(
    path: impl AsRef<Path>,
) -> Result<Option<CommissioningScopeFile>, ConfigError> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let parsed: CommissioningScopeFile =
        serde_yaml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    if parsed.version != COMMISSIONING_SCOPE_VERSION {
        return Err(ConfigError::Parse {
            path: path.to_path_buf(),
            message: format!(
                "unsupported commissioning-scope version {} (expected {COMMISSIONING_SCOPE_VERSION})",
                parsed.version
            ),
        });
    }
    Ok(Some(CommissioningScopeFile::normalized(parsed.joints)))
}

/// Validate every joint against master inventory names.
pub fn validate_commissioning_scope_joints(
    joints: &[String],
    master_joints: &[String],
) -> Result<(), ConfigError> {
    let known: HashSet<&str> = master_joints.iter().map(String::as_str).collect();
    for joint in joints {
        if !known.contains(joint.as_str()) {
            return Err(ConfigError::UnknownJointSubset {
                joint: joint.clone(),
            });
        }
    }
    Ok(())
}

/// Effective scope = persisted ∩ ceiling. No ceiling → persisted as-is (order preserved sorted).
pub fn effective_commissioning_scope(
    persisted: &[String],
    ceiling: Option<&HashSet<String>>,
) -> Vec<String> {
    let mut out: Vec<String> = match ceiling {
        Some(ceil) => persisted
            .iter()
            .filter(|j| ceil.contains(j.as_str()))
            .cloned()
            .collect(),
        None => persisted.to_vec(),
    };
    out.sort();
    out.dedup();
    out
}

/// True when `next` contains any joint absent from `previous` (energizable set grows).
pub fn scope_widens(previous_effective: &[String], next_effective: &[String]) -> bool {
    let prev: HashSet<&str> = previous_effective.iter().map(String::as_str).collect();
    next_effective.iter().any(|j| !prev.contains(j.as_str()))
}

/// Atomic write: temp + rename. Creates parent dirs.
pub fn save_commissioning_scope(
    path: impl AsRef<Path>,
    scope: &CommissioningScopeFile,
) -> Result<(), ConfigError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
            path: parent.to_path_buf(),
            message: e.to_string(),
        })?;
    }
    let normalized = CommissioningScopeFile::normalized(scope.joints.iter().cloned());
    if normalized.version != COMMISSIONING_SCOPE_VERSION {
        return Err(ConfigError::Parse {
            path: path.to_path_buf(),
            message: format!(
                "unsupported commissioning-scope version {} (expected {COMMISSIONING_SCOPE_VERSION})",
                normalized.version
            ),
        });
    }
    let text = serde_yaml::to_string(&normalized).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let tmp = path.with_extension("yaml.tmp");
    fs::write(&tmp, &text).map_err(|e| ConfigError::Io {
        path: tmp.clone(),
        message: e.to_string(),
    })?;
    fs::rename(&tmp, path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(())
}

/// Remove persisted scope file (no-op if missing).
pub fn clear_commissioning_scope(path: impl AsRef<Path>) -> Result<(), ConfigError> {
    let path = path.as_ref();
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ConfigError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalize_sorts_and_dedups() {
        let scope = CommissioningScopeFile::normalized([
            "right_shoulder_pitch",
            "right_shoulder_roll",
            "right_shoulder_pitch",
            "  ",
        ]);
        assert_eq!(scope.version, COMMISSIONING_SCOPE_VERSION);
        assert_eq!(
            scope.joints,
            vec![
                "right_shoulder_pitch".to_string(),
                "right_shoulder_roll".to_string(),
            ]
        );
    }

    #[test]
    fn effective_intersects_ceiling() {
        let persisted = vec![
            "right_shoulder_roll".to_string(),
            "right_shoulder_pitch".to_string(),
            "right_upper_arm_yaw".to_string(),
            "right_elbow_pitch".to_string(),
        ];
        let ceiling: HashSet<String> = [
            "right_shoulder_roll".to_string(),
            "right_shoulder_pitch".to_string(),
            "right_upper_arm_yaw".to_string(),
        ]
        .into_iter()
        .collect();
        let effective = effective_commissioning_scope(&persisted, Some(&ceiling));
        assert_eq!(
            effective,
            vec![
                "right_shoulder_pitch".to_string(),
                "right_shoulder_roll".to_string(),
                "right_upper_arm_yaw".to_string(),
            ]
        );
    }

    #[test]
    fn effective_without_ceiling_keeps_persisted() {
        let persisted = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            effective_commissioning_scope(&persisted, None),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn widen_detects_growth_not_narrow() {
        let prev = vec!["roll".to_string(), "pitch".to_string()];
        let wider = vec!["roll".to_string(), "pitch".to_string(), "yaw".to_string()];
        let narrower = vec!["roll".to_string()];
        assert!(scope_widens(&prev, &wider));
        assert!(!scope_widens(&prev, &narrower));
        assert!(!scope_widens(&prev, &prev));
    }

    #[test]
    fn validate_rejects_unknown_joint() {
        let master = vec!["right_shoulder_roll".to_string()];
        let err = validate_commissioning_scope_joints(
            &["left_wrist_roll".to_string()],
            &master,
        )
        .expect_err("unknown");
        assert!(matches!(err, ConfigError::UnknownJointSubset { .. }));
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = tempdir().expect("temp");
        let path = dir.path().join("commissioning-scope.yaml");
        assert!(load_commissioning_scope(&path)
            .expect("load")
            .is_none());
    }

    #[test]
    fn save_load_round_trip_atomic() {
        let dir = tempdir().expect("temp");
        let path = dir.path().join("var").join("commissioning-scope.yaml");
        let scope = CommissioningScopeFile::normalized([
            "right_shoulder_pitch",
            "right_shoulder_roll",
        ]);
        save_commissioning_scope(&path, &scope).expect("save");
        assert!(!path.with_extension("yaml.tmp").exists());
        let loaded = load_commissioning_scope(&path)
            .expect("load")
            .expect("present");
        assert_eq!(loaded, scope);
    }

    #[test]
    fn clear_removes_file() {
        let dir = tempdir().expect("temp");
        let path = dir.path().join("commissioning-scope.yaml");
        save_commissioning_scope(
            &path,
            &CommissioningScopeFile::normalized(["right_shoulder_roll"]),
        )
        .expect("save");
        clear_commissioning_scope(&path).expect("clear");
        assert!(load_commissioning_scope(&path)
            .expect("load")
            .is_none());
    }

    #[test]
    fn reject_unsupported_version() {
        let dir = tempdir().expect("temp");
        let path = dir.path().join("commissioning-scope.yaml");
        fs::write(&path, "version: 99\njoints: []\n").expect("write");
        let err = load_commissioning_scope(&path).expect_err("bad version");
        assert!(matches!(err, ConfigError::Parse { .. }));
    }
}
