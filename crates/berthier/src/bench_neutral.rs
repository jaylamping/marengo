//! Finite physical neutral qualification; command composition stays in Berthier.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::{BenchNeutralFeedback, DavoutError, MitJointCommand, PhysicalNeutralBench};
use serde::Serialize;

const PERIOD: Duration = Duration::from_millis(5);
const TICKS: u32 = 100;
const MAX_TICK_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, Serialize)]
pub struct NeutralBenchReport {
    pub schema: u32,
    pub audit_path: PathBuf,
    pub neutral_only: bool,
    pub ticks: u32,
    pub elapsed_us: u128,
    pub max_tick_delay_us: u128,
    pub samples: Vec<NeutralBenchSample>,
    pub stop_writes: usize,
    pub failed_stop_writes: usize,
}

#[derive(Debug, Serialize)]
pub struct NeutralBenchSample {
    pub elapsed_us: u128,
    pub feedback: Vec<BenchNeutralFeedback>,
}

/// Acquires a new owner-local reference, enables once, runs 500 ms, then stops.
/// Blocking standalone bench API; no persisted audit can re-arm a future owner.
pub fn run_bench_neutral(
    root: impl AsRef<Path>,
    operator: &str,
    confirmed_home: bool,
    sign_attested: bool,
    confirmed_neutral_enable: bool,
) -> Result<NeutralBenchReport, DavoutError> {
    let mut owner = PhysicalNeutralBench::acquire(
        root,
        operator,
        confirmed_home,
        sign_attested,
        confirmed_neutral_enable,
    )?;
    let joints = owner.joints().to_vec();
    let audit_path = owner.audit_path().to_owned();
    let result = (|| {
        let started = Instant::now();
        owner.begin(neutral_commands(&joints))?;
        let end = owner.active_deadline().ok_or_else(|| DavoutError::Homing {
            message: "missing neutral enable deadline".into(),
        })?;
        let mut deadline = Instant::now();
        let mut max_delay = Duration::ZERO;
        let mut samples = Vec::with_capacity(TICKS as usize);
        for _ in 0..TICKS {
            let now = Instant::now();
            if now >= end {
                break;
            }
            let delay = now.saturating_duration_since(deadline);
            if delay > MAX_TICK_DELAY {
                return Err(DavoutError::Homing {
                    message: "neutral bench tick delayed more than 50 ms".into(),
                });
            }
            max_delay = max_delay.max(delay);
            let feedback = owner.tick(neutral_commands(&joints))?;
            samples.push(NeutralBenchSample {
                elapsed_us: started.elapsed().as_micros(),
                feedback,
            });
            deadline += PERIOD;
            if let Some(remaining) = deadline.min(end).checked_duration_since(Instant::now()) {
                std::thread::sleep(remaining);
            }
        }
        if samples
            .last()
            .is_none_or(|sample| sample.feedback.len() != joints.len())
        {
            return Err(DavoutError::Homing {
                message: "neutral bench ended without feedback from every drive".into(),
            });
        }
        Ok((started.elapsed(), max_delay, samples))
    })();
    // Finish before encoding/printing/persistence, including failed begin or tick.
    let stop = owner.finish();
    match (result, stop) {
        (Ok((elapsed, delay, samples)), Ok(stop)) => Ok(NeutralBenchReport {
            schema: 1,
            audit_path,
            neutral_only: true,
            ticks: samples.len() as u32,
            elapsed_us: elapsed.as_micros(),
            max_tick_delay_us: delay.as_micros(),
            samples,
            stop_writes: stop.attempts.len(),
            failed_stop_writes: stop.failed_writes(),
        }),
        (Err(error), Ok(_)) => Err(error),
        (result, Err(stop)) => Err(DavoutError::Homing {
            message: format!(
                "neutral bench {:?}; final stop failed: {stop}",
                result.err()
            ),
        }),
    }
}

fn neutral_commands(joints: &[String]) -> Vec<MitJointCommand> {
    joints
        .iter()
        .map(|joint| MitJointCommand {
            joint: joint.clone(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: 0.0,
        })
        .collect()
}
