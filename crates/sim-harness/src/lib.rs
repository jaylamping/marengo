//! Headless simulation harness for Marengo integration tests.
//!
//! MuJoCo stepping is run via `sim/scripts/smoke_test.py` in CI (`check-sim`).
//! This crate holds Rust-side fixture validation and future in-process bindings.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Default MJCF path for tests and compose.
pub fn default_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.xml")
}

/// Returns true if the model file exists (for skip logic in tests).
pub fn model_exists(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fixture_exists() {
        let p = default_model_path();
        assert!(model_exists(&p), "missing {:?} — run from repo root", p);
    }

    #[test]
    #[cfg(feature = "sim")]
    fn sim_feature_links_kinematics_fixture() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../sim/fixtures/minimal.urdf");
        let robot = armee_kinematics::load_urdf(&path).expect("urdf");
        assert_eq!(armee_kinematics::joint_count(&robot), 2);
    }
}
