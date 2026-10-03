// Separate existing-law nominal-period contract. Eventual parent: position_hold.
// This does not treat the implementation's integer 1000/hz as an expected value.
// Both real fixtures are collected/dropped before every assertion.

use super::*;

#[derive(Debug)]
enum PeriodTerminal {
    Survived,
    AscentStall { joint: String, ms: u64, sample: u32 },
    OtherError { message: String, sample: u32 },
}

#[derive(Debug)]
struct PeriodObservation {
    valid_outputs: bool,
    samples: usize,
    recovery_event_samples: usize,
    last_q: Option<f64>,
    last_raw_dq: Option<f64>,
    largest_stall_ms: u64,
    terminal: PeriodTerminal,
}

fn period_parameters() -> HoldJointParams {
    HoldJointParams {
        kp: 8.0,
        kd: 1.25,
        ki: 0.0,
        max_lead: 0.12,
        vel_deadband: 0.02,
        advance_max_lead: 0.12,
        advance_vel_deadband: 0.02,
        slew_rad_s: 0.15,
        trajectory_v_max: 1.25,
        trajectory_threshold_rad: 0.15,
        a_max: 4.5,
        velocity_cap: Some(2.0),
        friction: None,
        limit_policy: None,
        tau_meas: 0.0,
    }
}

fn period_observe(moving: bool) -> PeriodObservation {
    let mut hold = PositionHold::new(1);
    hold.arm(&[0.02], &[0.30], 0);
    let parameters = [period_parameters()];
    let names = [String::from("period_joint")];
    let mut wave = None;
    let mut previous_q = 0.02;
    let mut observation = PeriodObservation {
        valid_outputs: true,
        samples: 0,
        recovery_event_samples: 0,
        last_q: None,
        last_raw_dq: None,
        largest_stall_ms: 0,
        terminal: PeriodTerminal::Survived,
    };
    // 3125 * 0.0008 s = 2.5 s. Coherent positive speed is 0.01 rad/s.
    // This finite nominal-period policy is independent of host elapsed time.
    for step in 1_u32..=3125 {
        let q = if moving {
            0.02 + f64::from(step) * 0.000008
        } else {
            0.02
        };
        let dq = (q - previous_q) / 0.0008;
        previous_q = q;
        match hold.tick(HoldWorld {
            q: &[q],
            dq_meas: &[dq],
            tau_g: &[0.0],
            joints: &parameters,
            joint_names: &names,
            dt: 0.0008,
            hz: 1250,
            tick_count: u64::from(step),
            wave: &mut wave,
        }) {
            Ok(out) => {
                observation.samples += 1;
                let (Some(diag), Some(command)) = (out.diag.first(), out.mit.first()) else {
                    observation.valid_outputs = false;
                    continue;
                };
                observation.valid_outputs &= out.diag.len() == 1
                    && out.mit.len() == 1
                    && diag.q == q
                    && diag.dq_raw == dq
                    && diag.target == 0.30
                    && command.joint == "period_joint"
                    && command.position_rad.is_finite()
                    && command.velocity_rad_s.is_finite()
                    && command.torque_ff_nm.is_finite();
                observation.recovery_event_samples +=
                    usize::from(diag.planner_event.as_str() == "ascent_breakaway");
                observation.largest_stall_ms =
                    observation.largest_stall_ms.max(diag.ascent_stall_ms);
                observation.last_q = Some(diag.q);
                observation.last_raw_dq = Some(diag.dq_raw);
            }
            Err(HoldError::AscentStall { joint, ms, .. }) => {
                observation.terminal = PeriodTerminal::AscentStall {
                    joint,
                    ms,
                    sample: step,
                };
                break;
            }
            Err(error) => {
                observation.terminal = PeriodTerminal::OtherError {
                    message: error.to_string(),
                    sample: step,
                };
                break;
            }
        }
    }
    drop(hold);
    observation
}

#[test]
fn submillisecond_period_accumulates_stall_time_without_faulting_real_motion() {
    // Both helpers run and drop their real law instances before classification.
    let stationary = period_observe(false);
    let moving = period_observe(true);
    let unexpected_errors: Vec<String> = [&stationary, &moving]
        .into_iter()
        .filter_map(|outcome| match &outcome.terminal {
            PeriodTerminal::OtherError { message, sample } => {
                Some(format!("sample {sample}: {message}"))
            }
            _ => None,
        })
        .collect();
    assert!(unexpected_errors.is_empty() && stationary.valid_outputs && moving.valid_outputs
            && stationary.recovery_event_samples > 0
            && stationary.last_q == Some(0.02) && stationary.last_raw_dq == Some(0.0),
        "CS24 period setup must reach actual unresolved stationary recovery and the moving neighbor; unexpected_errors={unexpected_errors:?}; stationary={stationary:#?}, moving={moving:#?}");
    assert!(
        matches!(&moving.terminal, PeriodTerminal::Survived)
            && moving.samples == 3125
            && moving.last_q.is_some_and(|q| (q - 0.045).abs() < 1e-12)
            && moving.last_raw_dq.is_some_and(|dq| dq > 0.0 && dq < 0.02),
        "CS24 period: 2500 ms of coherent slow motion must survive at 1250 Hz; moving={moving:#?}"
    );

    // A 1ms-per-call clamp would fault before sample2500 and is invalid.
    // Allow the unchanged law's geometric/planner reachability delay, up to
    // 2.5 nominal seconds total; require actual unchanged2000ms fuse evidence.
    assert!(matches!(&stationary.terminal,
        PeriodTerminal::AscentStall { joint, ms, sample }
            if joint == "period_joint" && *ms >= 2000 && *ms <= 2500
                && *sample >= 2500 && *sample <= 3125),
        "CS24 period: a finite 0.0008 s tick must accumulate the nominal 2000 ms stall fuse at 1250 Hz; stationary={stationary:#?}, moving={moving:#?}");
}
