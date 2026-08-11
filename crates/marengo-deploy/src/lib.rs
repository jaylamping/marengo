//! # marengo-deploy
//!
//! Domain logic for the Pi self-update lifecycle and deployed-version status.
//! This crate owns deploy-job persistence and reconciliation, revision parsing,
//! path resolution, upstream SHA caching, self-update enqueueing, and the
//! typed status snapshot consumed by operator clients.
//!
//! It does not own HTTP routing, Axum request/response handling, gateway
//! authentication, SharedState safety gates, or Consul UI behavior. The
//! on-disk job JSON schema is owned here; `scripts/pi-*-self-update.sh` must
//! stay aligned (see `tests/job_script_contract.rs`).

pub mod enqueue;
mod error;
pub mod job;
pub mod paths;
pub mod rev;
pub mod status;
pub mod upstream;

pub use enqueue::{enqueue_self_update, new_job_id};
pub use error::{DeployError, Result};
pub use job::{
    load_reconciled_job, read_job_file, read_job_file_strict, reconcile_job, write_job_file,
    DeployJob, DeployJobState, DeployPhase, JobFileRead, DEPLOY_JOB_MAX_AGE_SECS,
};
pub use paths::{
    resolve_deploy_rev_path, resolve_enqueue_script, resolve_job_file_path,
    resolve_self_update_log_path, resolve_upstream_cache_path,
};
pub use rev::{parse_deploy_rev, read_deploy_rev, shas_match, ParsedDeployRev};
pub use status::{
    assemble_version_status, current_version_status, derive_ui_state, ready_for_target,
    web_root_ready, UpdateUiState, VersionStatus,
};
pub use upstream::{fetch_upstream_sha, init_upstream_cache_from_disk, UPSTREAM_CACHE_TTL_SECS};
