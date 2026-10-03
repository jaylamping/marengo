// Frozen CS24 behavioral probe. Intended parent namespace: position_hold.
// Only existing production new/arm/tick and returned public diagnostics are used.
// No planner echo, private setter, reference grant, clock sleep, bus or plant model.

use super::*;

fn measured_sample(
    hold: &mut PositionHold,
    params: &[HoldJointParams],
    names: &[String],
    q: f64,
    dq: f64,
    tick_count: u64,
) -> Result<HoldTickOut, HoldError> {
    let mut wave = None;
    hold.tick(HoldWorld {
        q: &[q],
        dq_meas: &[dq],
        tau_g: &[0.0],
        joints: params,
        joint_names: names,
        dt: 0.005,
        hz: 200,
        tick_count,
        wave: &mut wave,
    })
}

fn assert_measured_sample(out: &HoldTickOut, q: f64, dq: f64) {
    assert_eq!(
        out.diag.len(),
        1,
        "one measured joint must reach the real law"
    );
    assert_eq!(
        out.mit.len(),
        1,
        "real compose must return that joint's command"
    );
    assert_eq!(
        out.diag[0].q, q,
        "diagnostics must retain the supplied encoder position"
    );
    assert_eq!(
        out.diag[0].dq_raw, dq,
        "diagnostics must retain the supplied measured velocity"
    );
    assert_eq!(
        out.diag[0].target, 0.30,
        "the unresolved outbound target must remain latched"
    );
    assert_eq!(out.mit[0].joint, "measured_joint");
    assert!(out.mit[0].position_rad.is_finite());
    assert!(out.mit[0].velocity_rad_s.is_finite());
    assert!(out.mit[0].torque_ff_nm.is_finite());
}

#[test]
fn coherent_motion_then_stationary_encoder_trips_stall_fuse() -> Result<(), HoldError> {
    // Same finite law fixture as the independently executed batch06 tail probe.
    // These are test inputs, not changes to installed limits or sign-off.
    let params = [HoldJointParams {
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
    }];
    let names = [String::from("measured_joint")];

    // Positive control: first arm the real recovery fuse with 500 ms at rest.
    // Then independently advance q by 0.00005 rad per 5 ms at measured 0.01 rad/s
    // for 2500 ms. This is deliberately slower than the 0.02 velocity deadband.
    // Blanket faulting or requiring deadband-sized speed cannot satisfy this case.
    let mut moving = PositionHold::new(1);
    moving.arm(&[0.02], &[0.30], 0);
    let mut armed_stall_ms = 0;
    for tick_count in 1_u64..=100 {
        let out = measured_sample(&mut moving, &params, &names, 0.02, 0.0, tick_count)?;
        assert_measured_sample(&out, 0.02, 0.0);
        armed_stall_ms = armed_stall_ms.max(out.diag[0].ascent_stall_ms);
    }
    assert!(
        armed_stall_ms > 0 && armed_stall_ms < 2000,
        "the moving control must first reach the actual unexpired recovery fuse; got {armed_stall_ms} ms"
    );
    let mut moving_samples = 0;
    let mut last_moving_q = None;
    for step in 1_u32..=500 {
        let q = 0.02 + f64::from(step) * 0.00005;
        let out = measured_sample(&mut moving, &params, &names, q, 0.01, 100 + u64::from(step))?;
        assert_measured_sample(&out, q, 0.01);
        assert!(
            out.diag[0].dq_filt > 0.0,
            "coherent slow motion must reach the real velocity filter"
        );
        last_moving_q = Some(out.diag[0].q);
        moving_samples += 1;
    }
    assert_eq!(moving_samples, 500);
    assert!(
        matches!(last_moving_q, Some(q) if (q - 0.045).abs() < 1e-12),
        "the positive control must execute all 2500 ms of independent measured progress; got {last_moving_q:?}"
    );
    drop(moving);

    // Critical history: actual law/filter see 100 coherent measured-motion
    // samples before the joint stops. The position is never read from q_traj.
    let mut stopped = PositionHold::new(1);
    stopped.arm(&[0.02], &[0.30], 0);
    let mut last_before_stop = None;
    for step in 1_u32..=100 {
        let q = 0.02 + f64::from(step) * 0.00005;
        let out = measured_sample(&mut stopped, &params, &names, q, 0.01, u64::from(step))?;
        assert_measured_sample(&out, q, 0.01);
        assert!(
            out.diag[0].dq_filt > 0.0,
            "prior coherent motion must populate the actual velocity filter"
        );
        last_before_stop = Some((out.diag[0].q, out.diag[0].dq_filt));
    }
    assert!(
        matches!(last_before_stop, Some((q, dq)) if (q - 0.025).abs() < 1e-12 && dq > 0.0),
        "the stopped fixture must first execute 500 ms of coherent measured motion; got {last_before_stop:?}"
    );

    let mut fault = None;
    let mut tail_after_100_stationary_samples = None;
    let mut largest_stationary_stall_ms = 0;
    let mut stationary_samples = 0;
    // All encoder samples remain literally 0.025 rad and 0 rad/s.
    // Allow 500 ms nominal EMA washout plus the unchanged 2000 ms fuse interval.
    // Washout is a fixture diagnostic, not a new production noise threshold.
    for step in 1_u32..=500 {
        stationary_samples += 1;
        match measured_sample(
            &mut stopped,
            &params,
            &names,
            0.025,
            0.0,
            100 + u64::from(step),
        ) {
            Ok(out) => {
                assert_measured_sample(&out, 0.025, 0.0);
                largest_stationary_stall_ms =
                    largest_stationary_stall_ms.max(out.diag[0].ascent_stall_ms);
                if step == 100 {
                    tail_after_100_stationary_samples = Some(out.diag[0].dq_filt);
                    assert!(
                        out.diag[0].dq_filt.abs() < 1e-9,
                        "the nominal filter must wash out before the remaining 2000 ms bound; got {}",
                        out.diag[0].dq_filt
                    );
                }
            }
            Err(HoldError::AscentStall { joint, ms, .. }) => {
                fault = Some((joint, ms, step));
                break;
            }
            Err(error) => return Err(error),
        }
    }
    drop(stopped);

    // No threads, disk, devices or host time are involved. Both law fixtures are
    // dropped before the decisive oracle. Do not require a positive residual
    // after a valid repair: a legitimate exact-zero filter tail is acceptable.
    assert!(
        matches!(&fault, Some((joint, ms, step))
            if joint == "measured_joint" && *ms >= 2000 && *ms <= 2500
                && *step >= 300 && *step <= 500),
        "CS24: 500 unchanged encoder samples after coherent measured motion must trip the existing no-progress fuse; got {fault:?}; stationary_samples={stationary_samples}, tail_after_100={tail_after_100_stationary_samples:?}, largest_stationary_stall_ms={largest_stationary_stall_ms}"
    );
    Ok(())
}
