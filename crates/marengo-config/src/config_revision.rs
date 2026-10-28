//! Content revision hashing for master config CAS (YAML write-behind).

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::ConfigError;

/// Stable SHA-256 content hash of robot/motors/control/homing YAML plus the URDF bytes
/// `robot.yaml` names, for CAS.
pub fn profile_content_revision(config_dir: impl AsRef<Path>) -> Result<String, ConfigError> {
    let dir = config_dir.as_ref();
    let _lock = crate::ProfileWriteLock::acquire(dir)?;
    profile_content_revision_unlocked(dir)
}

pub(crate) fn profile_content_revision_unlocked(
    config_dir: impl AsRef<Path>,
) -> Result<String, ConfigError> {
    let dir = config_dir.as_ref();
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
    // activation replaces it, so a client's CAS must not pass across either. It sits at
    // `robot.urdf` under the profile root (the config dir's parent, as on the Pi). A missing
    // file hashes as absent (a YAML-only tree still has a revision; boot refuses it anyway),
    // so adding or removing it changes the revision too.
    if let Some(urdf) = urdf {
        let path = dir.parent().unwrap_or(dir).join(&urdf);
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
        let a = profile_content_revision(temp.path().join("config")).unwrap();
        let b = profile_content_revision(temp.path().join("config")).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn revision_changes_when_the_urdf_changes() {
        let temp = master_profile();
        let config = temp.path().join("config");
        let before = profile_content_revision(&config).unwrap();
        let urdf = temp.path().join("assets/urdf/marengo.urdf");
        let text = std::fs::read_to_string(&urdf).unwrap();
        std::fs::write(&urdf, text.replacen("upper=\"3.25\"", "upper=\"3.3\"", 1)).unwrap();
        assert_ne!(profile_content_revision(&config).unwrap(), before);
    }

    #[test]
    fn revision_is_stable_across_serde_mapping_reorder() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::write(temp.path().join(name), "first: 1\nsecond: 2\n").unwrap();
        }
        let first = profile_content_revision(temp.path()).unwrap();
        std::fs::write(temp.path().join("robot.yaml"), "second: 2\nfirst: 1\n").unwrap();

        assert_eq!(profile_content_revision(temp.path()).unwrap(), first);
    }
}
