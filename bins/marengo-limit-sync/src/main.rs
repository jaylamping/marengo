//! Apply a Set Limits patch to the local git checkout (motors + control soft + expand-only URDF).
//!
//! Invoked by Consul after the Pi reports Durable persist — never on Pending alone.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use marengo_config::{
    apply_local_limit_patch, soft_limits_with_inset, LimitPatch, DEFAULT_SOFT_INSET_RAD,
};
use marengo_support::init_tracing;

#[derive(Debug, Parser)]
#[command(name = "marengo-limit-sync")]
#[command(about = "Sync taught joint limits into the local Marengo checkout")]
struct Args {
    /// Repository root (contains config/ and assets/urdf/).
    #[arg(long)]
    repo_root: PathBuf,
    /// Joint name.
    #[arg(long)]
    joint: String,
    /// Hard lower (rad).
    #[arg(long, allow_negative_numbers = true)]
    lower: f64,
    /// Hard upper (rad).
    #[arg(long, allow_negative_numbers = true)]
    upper: f64,
    /// Soft inset from hard (rad). Used when soft bounds are omitted.
    #[arg(long, default_value_t = DEFAULT_SOFT_INSET_RAD)]
    soft_inset: f64,
    /// Soft lower (rad). When set with `--soft-upper`, overrides inset defaults.
    #[arg(long, allow_negative_numbers = true)]
    soft_lower: Option<f64>,
    /// Soft upper (rad). When set with `--soft-lower`, overrides inset defaults.
    #[arg(long, allow_negative_numbers = true)]
    soft_upper: Option<f64>,
}

fn main() -> ExitCode {
    init_tracing();
    let args = Args::parse();
    let (soft_lo, soft_hi) = match (args.soft_lower, args.soft_upper) {
        (Some(lo), Some(hi)) => (lo, hi),
        (None, None) => soft_limits_with_inset(args.lower, args.upper, args.soft_inset),
        _ => {
            tracing::error!("--soft-lower and --soft-upper must be supplied together");
            return ExitCode::FAILURE;
        }
    };
    let patch = LimitPatch {
        joint: args.joint,
        position_lower_rad: args.lower,
        position_upper_rad: args.upper,
        torque_limit_nm: None,
        position_soft_lower_rad: Some(soft_lo),
        position_soft_upper_rad: Some(soft_hi),
        velocity_max_rad_s: None,
    };

    match apply_local_limit_patch(&args.repo_root, &patch) {
        Ok(()) => {
            tracing::info!(
                joint = patch.joint,
                hard = ?(patch.position_lower_rad, patch.position_upper_rad),
                soft = ?(soft_lo, soft_hi),
                "local limit sync ok",
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "local limit sync failed");
            ExitCode::FAILURE
        }
    }
}
