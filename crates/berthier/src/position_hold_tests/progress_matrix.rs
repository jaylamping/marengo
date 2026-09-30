// Independent same-source law matrix; eventual parent is position_hold.
// Existing new/arm/tick only. Measurements never come from planner output.
// All fixtures finish/drop before setup, neighbor and named defect assertions.

use super::*;

#[derive(Clone, Copy)]
struct MatrixSample {
    q: f64,
    dq: f64,
}

#[derive(Debug)]
enum MatrixTerminal {
    Survived,
    AscentStall {
        joint: String,
        ms: u64,
        stage: &'static str,
        sample: usize,
    },
    OtherError {
        message: String,
        stage: &'static str,
        sample: usize,
    },
}

#[derive(Debug)]
struct MatrixObservation {
    label: &'static str,
    valid_outputs: bool,
    prefix_samples: usize,
    sequence_samples: usize,
    prefix_max_stall_ms: u64,
    sequence_max_filtered_dq: Option<f64>,
    sequence_min_q: Option<f64>,
    sequence_max_q: Option<f64>,
    sequence_min_raw_dq: Option<f64>,
    sequence_max_raw_dq: Option<f64>,
    sequence_up_steps: usize,
    sequence_down_steps: usize,
    sequence_returns_to_anchor: usize,
    last_prefix_q: Option<f64>,
    last_sequence_q: Option<f64>,
    terminal: MatrixTerminal,
}

fn matrix_parameters() -> HoldJointParams {
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

// Input kinematics only: derive consistent dq from declared successive q.
// This does not inspect, predict or reproduce the production planner/filter.
fn matrix_coherent_samples(
    initial_q: f64,
    positions: impl IntoIterator<Item = f64>,
) -> Vec<MatrixSample> {
    let mut previous = initial_q;
    positions
        .into_iter()
        .map(|q| {
            let sample = MatrixSample {
                q,
                dq: (q - previous) / 0.005,
            };
            previous = q;
            sample
        })
        .collect()
}

fn matrix_observe(
    label: &'static str,
    initial_q: f64,
    prefix: &[MatrixSample],
    sequence: &[MatrixSample],
) -> MatrixObservation {
    let mut hold = PositionHold::new(1);
    hold.arm(&[initial_q], &[0.30], 0);
    let parameters = [matrix_parameters()];
    let names = [String::from("matrix_joint")];
    let mut wave = None;
    let mut tick_count = 0;
    let anchor = prefix.last().map_or(initial_q, |sample| sample.q);
    let mut previous_sequence_q = anchor;
    let mut observation = MatrixObservation {
        label,
        valid_outputs: true,
        prefix_samples: 0,
        sequence_samples: 0,
        prefix_max_stall_ms: 0,
        sequence_max_filtered_dq: None,
        sequence_min_q: None,
        sequence_max_q: None,
        sequence_min_raw_dq: None,
        sequence_max_raw_dq: None,
        sequence_up_steps: 0,
        sequence_down_steps: 0,
        sequence_returns_to_anchor: 0,
        last_prefix_q: None,
        last_sequence_q: None,
        terminal: MatrixTerminal::Survived,
    };

    'samples: for (stage, inputs) in [("prefix", prefix), ("sequence", sequence)] {
        for (index, input) in inputs.iter().enumerate() {
            tick_count += 1;
            match hold.tick(HoldWorld {
                q: &[input.q],
                dq_meas: &[input.dq],
                tau_g: &[0.0],
                joints: &parameters,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count,
                wave: &mut wave,
            }) {
                Ok(out) => {
                    if stage == "prefix" {
                        observation.prefix_samples += 1;
                    } else {
                        observation.sequence_samples += 1;
                    }
                    let (Some(diag), Some(command)) = (out.diag.first(), out.mit.first()) else {
                        observation.valid_outputs = false;
                        continue;
                    };
                    observation.valid_outputs &= out.diag.len() == 1
                        && out.mit.len() == 1
                        && diag.q == input.q
                        && diag.dq_raw == input.dq
                        && diag.target == 0.30
                        && diag.dq_filt.is_finite()
                        && command.joint == "matrix_joint"
                        && command.position_rad.is_finite()
                        && command.velocity_rad_s.is_finite()
                        && command.torque_ff_nm.is_finite();
                    if stage == "prefix" {
                        observation.prefix_max_stall_ms =
                            observation.prefix_max_stall_ms.max(diag.ascent_stall_ms);
                        observation.last_prefix_q = Some(diag.q);
                    } else {
                        observation.sequence_max_filtered_dq = Some(
                            observation
                                .sequence_max_filtered_dq
                                .map_or(diag.dq_filt, |old| old.max(diag.dq_filt)),
                        );
                        observation.sequence_min_q = Some(
                            observation
                                .sequence_min_q
                                .map_or(diag.q, |old| old.min(diag.q)),
                        );
                        observation.sequence_max_q = Some(
                            observation
                                .sequence_max_q
                                .map_or(diag.q, |old| old.max(diag.q)),
                        );
                        observation.sequence_min_raw_dq = Some(
                            observation
                                .sequence_min_raw_dq
                                .map_or(diag.dq_raw, |old| old.min(diag.dq_raw)),
                        );
                        observation.sequence_max_raw_dq = Some(
                            observation
                                .sequence_max_raw_dq
                                .map_or(diag.dq_raw, |old| old.max(diag.dq_raw)),
                        );
                        observation.sequence_up_steps += usize::from(diag.q > previous_sequence_q);
                        observation.sequence_down_steps +=
                            usize::from(diag.q < previous_sequence_q);
                        observation.sequence_returns_to_anchor += usize::from(diag.q == anchor);
                        previous_sequence_q = diag.q;
                        observation.last_sequence_q = Some(diag.q);
                    }
                }
                Err(HoldError::AscentStall { joint, ms }) => {
                    observation.terminal = MatrixTerminal::AscentStall {
                        joint,
                        ms,
                        stage,
                        sample: index + 1,
                    };
                    break 'samples;
                }
                Err(error) => {
                    observation.terminal = MatrixTerminal::OtherError {
                        message: error.to_string(),
                        stage,
                        sample: index + 1,
                    };
                    break 'samples;
                }
            }
        }
    }
    drop(hold);
    observation
}

fn matrix_expected_fault(observation: &MatrixObservation) -> bool {
    matches!(&observation.terminal,
        MatrixTerminal::AscentStall { joint, ms, stage, sample }
            if joint == "matrix_joint" && *stage == "sequence"
                && *ms >= 2000 && *ms <= 2500 && *sample >= 300 && *sample <= 500)
}

#[test]
fn ascent_watchdog_distinguishes_progress_dither_reversal_cache_and_settle() {
    // Positive crawl: first reach a real unexpired fuse at rest, then move slowly
    // and coherently for 2500 ms. Raw speed remains below the 0.02 deadband.
    let crawl_prefix = vec![MatrixSample { q: 0.02, dq: 0.0 }; 100];
    let crawl_sequence = matrix_coherent_samples(
        0.02,
        (1_u32..=500).map(|step| 0.02 + f64::from(step) * 0.00005),
    );

    // Five independently declared decoder count levels around 0.025 rad:
    // positive ratio-1 RS03 raw position codes 803e,803f,8040,8041,8042.
    // These literals are inputs; the test calls no decoder, encoder or plant.
    const LEVELS: [f64; 5] = [
        0.024160198867321014,
        0.02454519085586071,
        0.024928687140345573,
        0.025312181562185287,
        0.02569567784667015,
    ];
    const TRIANGLE: [usize; 8] = [3, 4, 3, 2, 1, 0, 1, 2];
    let dither_prefix = vec![
        MatrixSample {
            q: LEVELS[2],
            dq: 0.0
        };
        100
    ];
    let dither_sequence =
        matrix_coherent_samples(LEVELS[2], (0..500).map(|step| LEVELS[TRIANGLE[step % 8]]));

    // Coherent reversal after real forward measured movement. It stays above
    // home while moving away from the outstanding positive target.
    let reversal_prefix = matrix_coherent_samples(
        0.10,
        (1_u32..=100).map(|step| 0.10 + f64::from(step) * 0.00005),
    );
    let reversal_anchor = 0.10 + 100.0 * 0.00005;
    let reversal_sequence = matrix_coherent_samples(
        reversal_anchor,
        (1_u32..=500).map(|step| reversal_anchor - f64::from(step) * 0.00005),
    );

    // Deliberately held cached values, NOT fresh coherent encoder movement.
    // The position cannot earn progress from a repeated positive velocity value.
    let cached_prefix = vec![MatrixSample { q: 0.025, dq: 0.0 }; 100];
    let cached_sequence = vec![MatrixSample { q: 0.025, dq: 0.05 }; 500];

    // Actual measured approach to target, then 2500 ms settled. No teleport,
    // force-Hold, retarget or q_traj import stands in for the approach.
    let settle_prefix = matrix_coherent_samples(
        0.02,
        (1_u32..=280).map(|step| (0.02 + f64::from(step) * 0.001).min(0.30)),
    );
    let settle_sequence = vec![MatrixSample { q: 0.30, dq: 0.0 }; 500];

    // Each helper collects and drops its real PositionHold before returning.
    // Run EVERY fixture before any assertion, even when an earlier case fails.
    let observations = [
        matrix_observe("slow_crawl", 0.02, &crawl_prefix, &crawl_sequence),
        matrix_observe(
            "zero_net_dither",
            LEVELS[2],
            &dither_prefix,
            &dither_sequence,
        ),
        matrix_observe("reversal", 0.10, &reversal_prefix, &reversal_sequence),
        matrix_observe(
            "cached_positive_velocity",
            0.025,
            &cached_prefix,
            &cached_sequence,
        ),
        matrix_observe("target_settle", 0.02, &settle_prefix, &settle_sequence),
    ];
    let [crawl, dither, reversal, cached, settled] = &observations;
    let unexpected_errors: Vec<String> = observations
        .iter()
        .filter_map(|outcome| match &outcome.terminal {
            MatrixTerminal::OtherError {
                message,
                stage,
                sample,
            } => Some(format!(
                "{}: {stage} sample {sample}: {message}",
                outcome.label
            )),
            _ => None,
        })
        .collect();
    let setup_valid = unexpected_errors.is_empty()
        && observations.iter().all(|outcome| outcome.valid_outputs)
        && crawl.prefix_samples == 100
        && dither.prefix_samples == 100
        && reversal.prefix_samples == 100
        && cached.prefix_samples == 100
        && settled.prefix_samples == 280
        && crawl.prefix_max_stall_ms > 0
        && crawl.prefix_max_stall_ms < 2000
        && dither.prefix_max_stall_ms > 0
        && dither.prefix_max_stall_ms < 2000
        && cached.prefix_max_stall_ms > 0
        && cached.prefix_max_stall_ms < 2000
        && dither.sequence_max_filtered_dq.is_some_and(|dq| dq > 0.025)
        && dither.sequence_min_q == Some(LEVELS[0])
        && dither.sequence_max_q == Some(LEVELS[4])
        && dither.sequence_up_steps > 0
        && dither.sequence_down_steps > 0
        && dither.sequence_returns_to_anchor >= 2
        && reversal.sequence_max_raw_dq.is_some_and(|dq| dq < 0.0)
        && reversal.sequence_min_q.is_some_and(|q| q > 0.05)
        && cached.sequence_min_q == Some(0.025)
        && cached.sequence_max_q == Some(0.025)
        && cached.sequence_min_raw_dq == Some(0.05)
        && cached.sequence_max_raw_dq == Some(0.05)
        && cached.sequence_max_filtered_dq.is_some_and(|dq| dq > 0.025);
    assert!(setup_valid,
        "CS24 matrix setup must exercise all real neighboring and failure-path inputs before classification; unexpected_errors={unexpected_errors:?}; {observations:#?}");

    let positive_controls = matches!(&crawl.terminal, MatrixTerminal::Survived)
        && crawl.sequence_samples == 500
        && crawl
            .last_sequence_q
            .is_some_and(|q| (q - 0.045).abs() < 1e-12)
        && matches!(&settled.terminal, MatrixTerminal::Survived)
        && settled.sequence_samples == 500
        && settled.last_prefix_q == Some(0.30)
        && settled.sequence_min_q == Some(0.30)
        && settled.sequence_max_q == Some(0.30);
    assert!(positive_controls,
        "CS24 matrix: coherent slow crawl and actual target settle must survive beyond 2000 ms; {observations:#?}");

    let failed_cases: Vec<&str> = [dither, reversal, cached]
        .into_iter()
        .filter(|outcome| !matrix_expected_fault(outcome))
        .map(|outcome| outcome.label)
        .collect();
    assert!(failed_cases.is_empty(),
        "CS24 matrix: zero-net dither, reversed motion and cached positive velocity cannot renew the 2000 ms ascent fuse; failed_cases={failed_cases:?}; {observations:#?}");
}
