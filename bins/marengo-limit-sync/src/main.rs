//! Apply a Set Limits patch to the local git checkout (motors + control soft + expand-only URDF).
//!
//! Invoked by Consul after the Pi reports Durable persist — never on Pending alone.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use marengo_config::{
    apply_local_limit_patch, ensure_soft_inset, soft_limits_with_inset, LimitPatch,
    DEFAULT_SOFT_INSET_RAD,
};

#[derive(Debug, Parser)]
#[command(name = "marengo-limit-sync")]
#[command(about = "Sync taught joint limits into the local Marengo checkout")]
struct Args {
    /// Repository root (contains config/bringup and assets/urdf).
    #[arg(long)]
    repo_root: PathBuf,
    /// Bringup profile slug (allowlisted).
    #[arg(long)]
    profile: String,
    /// Joint name.
    #[arg(long)]
    joint: String,
    /// Hard lower (rad).
    #[arg(long)]
    lower: f64,
    /// Hard upper (rad).
    #[arg(long)]
    upper: f64,
    /// Soft inset from hard (rad). Used when soft bounds are omitted.
    #[arg(long, default_value_t = DEFAULT_SOFT_INSET_RAD)]
    soft_inset: f64,
    /// Soft lower (rad). When set with `--soft-upper`, overrides inset defaults.
    #[arg(long)]
    soft_lower: Option<f64>,
    /// Soft upper (rad). When set with `--soft-lower`, overrides inset defaults.
    #[arg(long)]
    soft_upper: Option<f64>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let (soft_lo, soft_hi) = match (args.soft_lower, args.soft_upper) {
        (Some(lo), Some(hi)) => (lo, hi),
        _ => soft_limits_with_inset(args.lower, args.upper, args.soft_inset),
    };
    let mut patch = LimitPatch {
        joint: args.joint,
        position_lower_rad: args.lower,
        position_upper_rad: args.upper,
        torque_limit_nm: None,
        position_soft_lower_rad: Some(soft_lo),
        position_soft_upper_rad: Some(soft_hi),
        velocity_max_rad_s: None,
    };
    ensure_soft_inset(&mut patch);

    match apply_local_limit_patch(&args.repo_root, &args.profile, &patch) {
        Ok(()) => {
            eprintln!(
                "local limit sync ok: profile={} joint={} hard=[{}, {}] soft=[{}, {}]",
                args.profile,
                patch.joint,
                patch.position_lower_rad,
                patch.position_upper_rad,
                soft_lo,
                soft_hi
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("local limit sync failed: {error}");
            ExitCode::FAILURE
        }
    }
}
