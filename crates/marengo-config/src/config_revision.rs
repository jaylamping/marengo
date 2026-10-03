//! Content revision hashing for master config CAS (YAML write-behind).

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::ConfigError;

/// Stable SHA-256 content hash of robot/motors/control/homing YAML for CAS.
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

    #[test]
    fn revision_stable_for_master_config_without_touching_repo() {
        let root = resolve_repo_root();
        let source = resolve_config_dir(&root);
        let temp = tempfile::tempdir().unwrap();
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(source.join(name), temp.path().join(name)).unwrap();
        }
        let a = profile_content_revision(temp.path()).unwrap();
        let b = profile_content_revision(temp.path()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
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
