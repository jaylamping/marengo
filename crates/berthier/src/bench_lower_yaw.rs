//! First finite position response at mechanical home; Davout owns admission.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::{
    BenchNeutralFeedback, DavoutError, LowerYawBenchGains, MitJointCommand, PhysicalLowerYawBench,
};
use serde::Serialize;

const JOINT: &str = "right_lower_arm_yaw";
const PERIOD: Duration = Duration::from_millis(5);

#[derive(Debug, Serialize)]
pub struct LowerYawBenchReport {
    pub schema: u32,
    pub audit_path: PathBuf,
    pub joint: &'static str,
    pub requested_peak_rad: f64,
    pub kp: f64,
    pub kd: f64,
    pub elapsed_us: u128,
    pub max_tick_delay_us: u128,
    pub samples: Vec<LowerYawBenchSample>,
    pub stop_writes: usize,
    pub failed_stop_writes: usize,
}

#[derive(Debug, Serialize)]
pub struct LowerYawBenchSample {
    pub elapsed_us: u128,
    pub target_rad: f64,
    pub feedback: Vec<BenchNeutralFeedback>,
}

/// One second total enabled budget: ramp +20 mrad and back, then stop.
pub fn run_bench_lower_yaw(
    root: impl AsRef<Path>,
    operator: &str,
    confirmed_home: bool,
    sign_attested: bool,
    confirmed_motion: bool,
    kp: f64,
    kd: f64,
) -> Result<LowerYawBenchReport, DavoutError> {
    let gains = LowerYawBenchGains::new(kp, kd)?;
    let mut owner = PhysicalLowerYawBench::acquire(
        root,
        operator,
        confirmed_home,
        sign_attested,
        confirmed_motion,
        gains,
    )?;
    let joints = owner.joints().to_vec();
    let audit_path = owner.audit_path().to_owned();
    let result = (|| {
        let started = Instant::now();
        owner.begin(commands(&joints, None, gains))?;
        let end = owner
            .active_deadline()
            .ok_or_else(|| error("missing enabled deadline"))?;
        let motion_started = Instant::now();
        let mut next_tick = motion_started;
        let mut max_delay = Duration::ZERO;
        let mut samples = Vec::with_capacity(200);
        while Instant::now() < end {
            let delay = Instant::now().saturating_duration_since(next_tick);
            if delay > Duration::from_millis(50) {
                return Err(error("lower-yaw tick delayed more than 50 ms"));
            }
            max_delay = max_delay.max(delay);
            let target = target_at(motion_started.elapsed());
            let feedback = owner.tick(commands(&joints, Some(target), gains))?;
            samples.push(LowerYawBenchSample {
                elapsed_us: started.elapsed().as_micros(),
                target_rad: target,
                feedback,
            });
            next_tick += PERIOD;
            if let Some(wait) = next_tick.min(end).checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
        if samples
            .last()
            .is_none_or(|s| s.feedback.len() != joints.len() || s.target_rad != 0.0)
        {
            return Err(error(
                "test ended before return command and complete feedback",
            ));
        }
        Ok((started.elapsed(), max_delay, samples))
    })();
    let stop = owner.finish();
    match (result, stop) {
        (Ok((elapsed, delay, samples)), Ok(stop)) => Ok(LowerYawBenchReport {
            schema: 1,
            audit_path,
            joint: JOINT,
            requested_peak_rad: 0.02,
            kp: gains.kp(),
            kd: gains.kd(),
            elapsed_us: elapsed.as_micros(),
            max_tick_delay_us: delay.as_micros(),
            samples,
            stop_writes: stop.attempts.len(),
            failed_stop_writes: stop.failed_writes(),
        }),
        (Err(error), Ok(_)) => Err(error),
        (result, Err(stop)) => Err(error(&format!(
            "lower-yaw {:?}; final stop failed: {stop}",
            result.err()
        ))),
    }
}

fn target_at(elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.4 {
        (0.05 * seconds).min(0.02)
    } else {
        (0.04 - 0.05 * seconds).max(0.0)
    }
}

fn commands(
    joints: &[String],
    target: Option<f64>,
    gains: LowerYawBenchGains,
) -> Vec<MitJointCommand> {
    joints
        .iter()
        .map(|joint| {
            let position = (joint == JOINT).then_some(target).flatten();
            MitJointCommand {
                joint: joint.clone(),
                kp: position.map_or(0.0, |_| gains.kp()),
                kd: position.map_or(0.0, |_| gains.kd()),
                position_rad: position.unwrap_or(0.0),
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.0,
            }
        })
        .collect()
}

fn error(message: &str) -> DavoutError {
    DavoutError::Homing {
        message: message.into(),
    }
}
