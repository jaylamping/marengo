// Candidate-interface law conformance. Original source lacks the threshold API.
// Intended unchanged cfg(test) child of position_hold; no duplicate wrapper.
use super::*;

fn numeric_parameters() -> HoldJointParams {
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
        law: HoldLaw::Legacy,
        descent_cap: None,
    }
}

fn numeric_sample(
    hold: &mut PositionHold,
    parameters: &[HoldJointParams],
    names: &[String],
    q: f64,
    dq: f64,
    dt: f64,
    tick: u64,
) -> Result<HoldTickOut, HoldError> {
    let mut wave = None;
    hold.tick(HoldWorld {
        q: &[q],
        dq_meas: &[dq],
        tau_g: &[0.0],
        joints: parameters,
        joint_names: names,
        dt,
        hz: 200,
        tick_count: tick,
        wave: &mut wave,
    })
}

fn numeric_output_valid(out: &HoldTickOut, q: f64, dq: f64) -> bool {
    let (Some(diag), Some(command)) = (out.diag.first(), out.mit.first()) else {
        return false;
    };
    out.diag.len() == 1
        && out.mit.len() == 1
        && diag.q == q
        && diag.dq_raw == dq
        && diag.target == 0.30
        && diag.dq_filt.is_finite()
        && command.joint == "numeric_joint"
        && command.position_rad.is_finite()
        && command.velocity_rad_s.is_finite()
        && command.torque_ff_nm.is_finite()
}

#[derive(Debug, PartialEq)]
struct NumericState {
    armed: bool,
    targets: Option<Vec<f64>>,
    raw_targets: Option<Vec<f64>>,
    planner: Option<(f64, f64)>,
    filtered_velocity: Option<f64>,
    stalled_ms: u64,
    retarget_tick: Option<u64>,
    effective_max_lead: f64,
}

fn numeric_state(hold: &PositionHold) -> NumericState {
    NumericState {
        armed: hold.is_armed(),
        targets: hold.targets().map(<[f64]>::to_vec),
        raw_targets: hold.setpoints_raw.clone(),
        planner: hold.planner_state(0),
        filtered_velocity: hold.dq_filtered_at(0),
        stalled_ms: hold.ascent_stall_ms_at(0),
        retarget_tick: hold.retarget_tick_at(0),
        effective_max_lead: hold.tick_effective_max_lead_at(0),
    }
}

#[derive(Debug)]
struct TinyGridObservation {
    prefix_samples: usize,
    prefix_valid: bool,
    cleared: Option<NumericState>,
    valid_outputs: bool,
    samples: usize,
    largest_stall_ms: u64,
    last_q: Option<f64>,
    error: Option<String>,
}

fn observe_tiny_grid(clear_and_rearm: bool) -> TinyGridObservation {
    let mut hold = PositionHold::with_progress_thresholds(vec![2.1572190615032845e-24]);
    // Start at zero: adding a 1e-24 input to .02 would erase its f64 displacement.
    hold.arm(&[0.0], &[0.30], 0);
    let parameters = [numeric_parameters()];
    let names = [String::from("numeric_joint")];
    let mut observation = TinyGridObservation {
        prefix_samples: 0,
        prefix_valid: true,
        cleared: None,
        valid_outputs: true,
        samples: 0,
        largest_stall_ms: 0,
        last_q: None,
        error: None,
    };
    if clear_and_rearm {
        // Populate real planner/filter/budget state before ordinary lifecycle clear.
        for tick in 1..=100 {
            match numeric_sample(&mut hold, &parameters, &names, 0.0, 0.0, 0.005, tick) {
                Ok(out) => {
                    observation.prefix_samples += 1;
                    observation.prefix_valid &= numeric_output_valid(&out, 0.0, 0.0);
                }
                Err(error) => {
                    observation.prefix_valid = false;
                    observation.error = Some(format!("pre-clear: {error}"));
                    break;
                }
            }
        }
        hold.clear();
        observation.cleared = Some(numeric_state(&hold));
        hold.arm(&[0.0], &[0.30], 0);
    }
    for step in 1_u32..=1800 {
        // Independent tiny installed-grid measurements, one literal code per1.5s.
        // Zero vendor velocity is an explicit numeric input, not a planner echo.
        let q = f64::from(step / 300) * 3.835_069_113_452_648e-24;
        match numeric_sample(
            &mut hold,
            &parameters,
            &names,
            q,
            0.0,
            0.005,
            u64::from(step),
        ) {
            Ok(out) => {
                observation.samples += 1;
                observation.valid_outputs &= numeric_output_valid(&out, q, 0.0);
                if let Some(diag) = out.diag.first() {
                    observation.largest_stall_ms =
                        observation.largest_stall_ms.max(diag.ascent_stall_ms);
                    observation.last_q = Some(diag.q);
                }
            }
            Err(error) => {
                observation.error = Some(format!("crawl sample {step}: {error}"));
                break;
            }
        }
    }
    drop(hold);
    observation
}

#[test]
fn tiny_installed_grid_progress_survives_clear_and_rearm() {
    let initialized = observe_tiny_grid(false);
    let rearmed = observe_tiny_grid(true);
    // Both real owners are dropped before any control/outcome assertion.
    assert!(initialized.valid_outputs && rearmed.valid_outputs);
    assert!(rearmed.prefix_valid && rearmed.prefix_samples == 100);
    assert!(
        rearmed.cleared.as_ref().is_some_and(|state| {
            !state.armed
                && state.targets.is_none()
                && state.raw_targets.is_none()
                && state.filtered_velocity.is_none()
                && state.stalled_ms == 0
        }),
        "ordinary clear must remove old intent before the same profile is rearmed"
    );
    assert!(
        initialized.error.is_none() && rearmed.error.is_none()
            && initialized.samples == 1800 && rearmed.samples == 1800
            && initialized.largest_stall_ms > 0 && rearmed.largest_stall_ms > 0
            && initialized.largest_stall_ms < 2000 && rearmed.largest_stall_ms < 2000
            && initialized.last_q.is_some_and(|q| q > 2e-23 && q < 3e-23)
            && rearmed.last_q == initialized.last_q,
        "CS24 numeric conformance: a valid tiny installed encoder grid must survive 1800 calls before and after clear/rearm, without a continuous one-unit epsilon floor; initialized={initialized:#?}, rearmed={rearmed:#?}"
    );
}

#[derive(Debug)]
struct InvalidObservation {
    seconds: f64,
    rejected_with_matching_period: bool,
    error: Option<String>,
    before: NumericState,
    after: NumericState,
}

#[test]
fn invalid_periods_preserve_state_and_zero_composition_preserves_budget() {
    let mut hold = PositionHold::with_progress_thresholds(vec![0.0]);
    hold.arm(&[0.02], &[0.30], 0);
    let parameters = [numeric_parameters()];
    let names = [String::from("numeric_joint")];
    let mut prefix_valid = true;
    let mut prefix_error = None;
    let mut prefix_samples = 0;
    for tick in 1..=100 {
        match numeric_sample(&mut hold, &parameters, &names, 0.02, 0.0, 0.005, tick) {
            Ok(out) => {
                prefix_samples += 1;
                prefix_valid &= numeric_output_valid(&out, 0.02, 0.0);
            }
            Err(error) => {
                prefix_error = Some(error.to_string());
                break;
            }
        }
    }
    let initial = numeric_state(&hold);
    let mut invalid = Vec::new();
    for (index, seconds) in [
        -0.1,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        1e-12,
    ]
    .into_iter()
    .enumerate()
    {
        let before = numeric_state(&hold);
        let result = numeric_sample(
            &mut hold,
            &parameters,
            &names,
            0.10,
            0.50,
            seconds,
            101 + index as u64,
        );
        let (rejected_with_matching_period, error) = match result {
            Err(HoldError::InvalidPeriod { seconds: received }) => (
                if seconds.is_nan() {
                    received.is_nan()
                } else {
                    received == seconds
                },
                None,
            ),
            Err(error) => (false, Some(error.to_string())),
            Ok(_) => (false, Some("invalid period was accepted".to_string())),
        };
        invalid.push(InvalidObservation {
            seconds,
            rejected_with_matching_period,
            error,
            before,
            after: numeric_state(&hold),
        });
    }
    let zero_before = hold.ascent_stall_ms_at(0);
    let zero = numeric_sample(&mut hold, &parameters, &names, 0.02, 0.0, 0.0, 107);
    let zero_after = hold.ascent_stall_ms_at(0);

    // Two valid follow-up observations test actual timer and credited-position
    // continuity, rather than merely checking rejected method return values.
    let stationary = numeric_sample(&mut hold, &parameters, &names, 0.02, 0.0, 0.005, 108);
    let stationary_budget = hold.ascent_stall_ms_at(0);
    let progress = numeric_sample(&mut hold, &parameters, &names, 0.021, 0.20, 0.005, 109);
    let progress_budget = hold.ascent_stall_ms_at(0);
    let mut terminal = None;
    let mut other_error = None;
    let mut tail_valid = true;
    // Only fresh constant measured q follows the real progress sample.
    for step in 1_u32..=401 {
        match numeric_sample(
            &mut hold,
            &parameters,
            &names,
            0.021,
            0.0,
            0.005,
            109 + u64::from(step),
        ) {
            Ok(out) => tail_valid &= numeric_output_valid(&out, 0.021, 0.0),
            Err(HoldError::AscentStall { joint, ms, .. }) => {
                terminal = Some((joint, ms, step));
                break;
            }
            Err(error) => {
                other_error = Some(error.to_string());
                break;
            }
        }
    }
    drop(hold);

    assert!(prefix_valid && prefix_samples == 100 && prefix_error.is_none());
    assert!(initial.armed && initial.stalled_ms > 0 && initial.stalled_ms < 2000);
    assert!(initial.planner.is_some() && initial.filtered_velocity == Some(0.0));
    for rejected in &invalid {
        assert!(
            rejected.rejected_with_matching_period && rejected.error.is_none()
                && rejected.before == initial && rejected.after == initial,
            "CS24 period conformance: invalid dt {:?} must return matching InvalidPeriod before changing planner, filter, intent or budget; observed={rejected:#?}",
            rejected.seconds
        );
    }
    assert!(
        zero.as_ref()
            .is_ok_and(|out| numeric_output_valid(out, 0.02, 0.0)),
        "explicit zero-time composition remains valid: {zero:?}"
    );
    assert_eq!(zero_before, initial.stalled_ms);
    assert_eq!(
        zero_after, zero_before,
        "zero-time compose cannot consume stall budget"
    );
    assert!(stationary
        .as_ref()
        .is_ok_and(|out| numeric_output_valid(out, 0.02, 0.0)));
    assert_eq!(
        stationary_budget,
        initial.stalled_ms + 5,
        "one real follow-up at the declared5ms period consumes exactly5ms"
    );
    assert!(progress
        .as_ref()
        .is_ok_and(|out| numeric_output_valid(out, 0.021, 0.20)));
    assert_eq!(
        progress_budget, 0,
        "actual new measured progress must renew the untouched budget"
    );
    assert!(tail_valid && other_error.is_none());
    assert!(matches!(&terminal, Some((joint, ms, step))
        if joint == "numeric_joint" && *ms >= 2000 && *ms <= 2005
            && *step >= 400 && *step <= 401),
        "CS24 period conformance: valid follow-up motion then fixed q must expire the declared two-second budget; terminal={terminal:?}, other={other_error:?}");
}
