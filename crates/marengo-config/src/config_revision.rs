//! Content revision hashing for master config CAS (YAML write-behind).

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::ConfigError;

/// Stable SHA-256 content hash of robot/motors/control/homing YAML in `config_dir` plus the
/// bytes of the URDF `robot.yaml` names, for CAS.
///
/// `repo_root` is the root the URDF writers resolve against ([`crate::resolve_urdf_path`]).
/// Pass the same root the caller's writer uses, never one derived from `config_dir`, so the
/// revision covers the file a Set Limits expand or a URDF activation actually replaces.
pub fn profile_content_revision(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
) -> Result<String, ConfigError> {
    let dir = config_dir.as_ref();
    let _lock = crate::ProfileWriteLock::acquire(dir)?;
    profile_content_revision_unlocked(repo_root.as_ref(), dir)
}

pub(crate) fn profile_content_revision_unlocked(
    repo_root: &Path,
    dir: &Path,
) -> Result<String, ConfigError> {
    let mut hasher = Sha256::new();
    let mut urdf = None;
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        let path = dir.join(name);
        let raw = std::fs::read(&path).map_err(|e| ConfigError::Io {
            path: path.clone(),
            message: e.to_string(),
        })?;
        let mut value: serde_yaml::Value =
            serde_yaml::from_slice(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?;
        canonicalize_yaml_value(&mut value, &path)?;
        let canonical = serde_yaml::to_string(&value).map_err(|e| ConfigError::Parse {
            path: path.clone(),
            message: e.to_string(),
        })?;
        hasher.update((name.len() as u64).to_be_bytes());
        hasher.update(name.as_bytes());
        hasher.update((canonical.len() as u64).to_be_bytes());
        hasher.update(canonical.as_bytes());
        if name == "robot.yaml" {
            urdf = value
                .get("robot")
                .and_then(|robot| robot.get("urdf"))
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_owned);
        }
    }
    // The URDF is durable source of truth too (ADR 0012): Set Limits expands it and a URDF
    // activation replaces it, so a client's CAS must not pass across either. It is resolved
    // under `repo_root` exactly as the writers resolve it, so a config dir that is not
    // `<repo_root>/config` still hashes the file they write. A missing file hashes as absent
    // (a YAML-only tree still has a revision; boot refuses it anyway), so adding or removing
    // it changes the revision too.
    if let Some(urdf) = urdf {
        let path = crate::urdf_path_under(repo_root, &urdf);
        let raw = match std::fs::read(&path) {
            Ok(raw) => Some(raw),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(ConfigError::Io {
                    path,
                    message: e.to_string(),
                })
            }
        };
        hasher.update((urdf.len() as u64).to_be_bytes());
        hasher.update(urdf.as_bytes());
        match raw {
            Some(raw) => {
                hasher.update([1]);
                hasher.update((raw.len() as u64).to_be_bytes());
                hasher.update(&raw);
            }
            None => hasher.update([0]),
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn canonicalize_yaml_value(value: &mut serde_yaml::Value, path: &Path) -> Result<(), ConfigError> {
    match value {
        serde_yaml::Value::Sequence(sequence) => {
            for item in sequence {
                canonicalize_yaml_value(item, path)?;
            }
        }
        serde_yaml::Value::Mapping(mapping) => {
            let mut entries = mapping
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            for (key, value) in &mut entries {
                canonicalize_yaml_value(key, path)?;
                canonicalize_yaml_value(value, path)?;
            }
            let mut encoded_keys = entries
                .iter()
                .map(|(key, _)| {
                    serde_yaml::to_string(key)
                        .map(|encoded| (encoded, key.clone()))
                        .map_err(|error| ConfigError::Parse {
                            path: path.to_path_buf(),
                            message: error.to_string(),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            encoded_keys.sort_by(|left, right| left.0.cmp(&right.0));
            let mut ordered = serde_yaml::Mapping::new();
            for (_, key) in encoded_keys {
                if let Some((_, value)) = entries.iter().find(|(candidate, _)| *candidate == key) {
                    ordered.insert(key, value.clone());
                }
            }
            *mapping = ordered;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::{resolve_config_dir, resolve_repo_root};

    /// A copy of the master profile: `<temp>/config/*.yaml` and `<temp>/assets/urdf`.
    fn master_profile() -> tempfile::TempDir {
        let root = resolve_repo_root();
        let source = resolve_config_dir(&root);
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::create_dir_all(temp.path().join("assets/urdf")).unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(source.join(name), temp.path().join("config").join(name)).unwrap();
        }
        std::fs::copy(
            root.join("assets/urdf/marengo.urdf"),
            temp.path().join("assets/urdf/marengo.urdf"),
        )
        .unwrap();
        temp
    }

    #[test]
    fn revision_stable_for_master_config_without_touching_repo() {
        let temp = master_profile();
        let a = profile_content_revision(temp.path(), temp.path().join("config")).unwrap();
        let b = profile_content_revision(temp.path(), temp.path().join("config")).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn revision_changes_when_the_urdf_changes() {
        let temp = master_profile();
        let config = temp.path().join("config");
        let before = profile_content_revision(temp.path(), &config).unwrap();
        let urdf = temp.path().join("assets/urdf/marengo.urdf");
        let text = std::fs::read_to_string(&urdf).unwrap();
        std::fs::write(&urdf, text.replacen("upper=\"3.25\"", "upper=\"3.3\"", 1)).unwrap();
        assert_ne!(
            profile_content_revision(temp.path(), &config).unwrap(),
            before
        );
    }

    /// `MARENGO_CONFIG_DIR` need not be `$MARENGO_ROOT/config`. The writers expand or replace
    /// the URDF under the repo root, so the revision must hash that file, not whatever sits
    /// beside the config dir (here a stale copy: the "wrong file" case).
    #[test]
    fn revision_hashes_the_urdf_the_writers_use_when_config_dir_is_elsewhere() {
        let root = master_profile();
        let elsewhere = tempfile::tempdir().unwrap();
        let config = elsewhere.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(root.path().join("config").join(name), config.join(name)).unwrap();
        }
        std::fs::create_dir_all(elsewhere.path().join("assets/urdf")).unwrap();
        std::fs::copy(
            root.path().join("assets/urdf/marengo.urdf"),
            elsewhere.path().join("assets/urdf/marengo.urdf"),
        )
        .unwrap();
        let robot = crate::load_robot_config_from(&config).unwrap();
        let written = crate::resolve_urdf_path(root.path(), &robot).unwrap();

        let before = profile_content_revision(root.path(), &config).unwrap();
        let mut text = std::fs::read_to_string(&written).unwrap();
        text.push_str("<!-- Set Limits expanded this URDF -->\n");
        std::fs::write(&written, text).unwrap();

        assert_ne!(
            profile_content_revision(root.path(), &config).unwrap(),
            before
        );
    }

    /// A bring-up profile (`--config-dir config/bringup/shoulder_pitch_dual`) sits two levels
    /// under the repo root, so `config_dir.parent()` holds no URDF at all. The revision must
    /// still cover the URDF runtime loads from the repo root, not hash it as absent.
    #[test]
    fn revision_covers_the_repo_urdf_for_a_bringup_profile_config_dir() {
        let root = master_profile();
        let bringup = root.path().join("config/bringup/shoulder_pitch_dual");
        std::fs::create_dir_all(&bringup).unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(root.path().join("config").join(name), bringup.join(name)).unwrap();
        }
        let robot = crate::load_robot_config_from(&bringup).unwrap();
        let loaded = crate::resolve_urdf_path(root.path(), &robot).unwrap();
        assert!(!bringup.parent().unwrap().join(&robot.robot.urdf).exists());

        let before = profile_content_revision(root.path(), &bringup).unwrap();
        let text = std::fs::read_to_string(&loaded).unwrap();
        std::fs::write(&loaded, text.replacen("upper=\"3.25\"", "upper=\"3.3\"", 1)).unwrap();
        assert_ne!(
            profile_content_revision(root.path(), &bringup).unwrap(),
            before
        );
        std::fs::remove_file(&loaded).unwrap();
        assert!(matches!(
            crate::resolve_urdf_path(root.path(), &robot),
            Err(ConfigError::UrdfMissing { .. })
        ));
        assert_ne!(
            profile_content_revision(root.path(), &bringup).unwrap(),
            before
        );
    }

    #[test]
    fn revision_is_stable_across_serde_mapping_reorder() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::write(temp.path().join(name), "first: 1\nsecond: 2\n").unwrap();
        }
        let first = profile_content_revision(temp.path(), temp.path()).unwrap();
        std::fs::write(temp.path().join("robot.yaml"), "second: 2\nfirst: 1\n").unwrap();

        assert_eq!(
            profile_content_revision(temp.path(), temp.path()).unwrap(),
            first
        );
    }
}
