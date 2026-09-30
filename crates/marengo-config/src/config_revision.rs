//! Content revision hashing for master config CAS (YAML write-behind).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::ConfigError;

/// Content hash of robot/motors/control/homing YAML for compare-and-swap.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::resolve_config_dir;
    use crate::resolve_repo_root;

    #[test]
    fn revision_stable_for_master_config() {
        let root = resolve_repo_root();
        let dir = resolve_config_dir(&root);
        let a = profile_content_revision(&dir).unwrap();
        let b = profile_content_revision(&dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }
}
