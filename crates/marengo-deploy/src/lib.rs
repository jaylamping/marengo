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

mod enqueue;
mod error;
mod job;
mod paths;
mod rev;
mod status;
mod upstream;

pub use enqueue::{enqueue_self_update, new_job_id};
pub use error::DeployError;
pub use job::{
    empty_reconciled, job_file_lock_path, load_reconciled_job, read_job_file, read_job_file_strict,
    DeployJob, DeployJobState, DeployPhase, JobFileRead,
};
pub use paths::{resolve_deploy_rev_path, PRIVILEGED_HELPERS_DIR};
pub use rev::{is_full_sha, read_deploy_rev, shas_match, ParsedDeployRev};
pub use status::{current_version_status, web_root_ready, UpdateUiState, VersionStatus};
pub use upstream::{fetch_upstream_sha, init_upstream_cache_from_disk};
