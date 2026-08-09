//! Deprecated bringup profile registry — retained for gateway compile until Phase 2 retires `/config/profiles*`.

use std::path::{Path, PathBuf};

use crate::{resolve_config_dir, ConfigError};

/// @deprecated Bringup profiles removed; always empty.
pub const BRINGUP_PRESET_IDS: &[&str] = &[];

/// @deprecated Bringup profiles removed; always empty.
pub const BRINGUP_PROFILE_SLUGS: &[&str] = &[];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetProfileMapping {
    pub preset_id: &'static str,
    pub profile_slug: &'static str,
}

/// @deprecated Bringup profiles removed; always empty.
pub const PRESET_PROFILE_MAP: &[PresetProfileMapping] = &[];

/// @deprecated Bringup profiles removed.
pub fn profile_slug_for_preset(_preset_id: &str) -> Option<&'static str> {
    None
}

/// @deprecated Bringup profiles removed.
pub fn preset_id_for_profile(_profile_slug: &str) -> Option<&'static str> {
    None
}

/// @deprecated Bringup profiles removed.
pub fn is_allowlisted_slug(_slug: &str) -> bool {
    false
}

/// @deprecated Use [`resolve_config_dir`] for master config paths.
pub fn resolve_bringup_dir(
    _repo_root: impl AsRef<Path>,
    slug: &str,
) -> Result<PathBuf, ConfigError> {
    Err(ConfigError::Parse {
        path: PathBuf::from("config/bringup"),
        message: format!("bringup profile retired (requested slug: {slug}); use master config/"),
    })
}

/// @deprecated Use [`resolve_config_dir`].
pub fn bringup_dir_default_repo(slug: &str) -> Result<PathBuf, ConfigError> {
    resolve_bringup_dir(resolve_config_dir(resolve_repo_root()), slug)
}

fn resolve_repo_root() -> PathBuf {
    crate::resolve_repo_root()
}

/// @deprecated Bringup presets removed; returns None.
pub fn derive_preset_label_for_joint(
    _joint: &str,
    _active_slug: &str,
    _membership_slugs: &[String],
) -> Option<&'static str> {
    None
}
