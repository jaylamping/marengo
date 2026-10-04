//! Conformance of the firmware emulator and Davout's timing rules with the
//! measured bench profile (`docs/commissioning/firmware/
//! robstride-timing-profile.json`, from `marengo-log-cli firmware-timing`).
//! Every range the bench measured must be one the emulator can reproduce, and
//! every Davout constant built on a firmware behaviour must keep its margin.
#![allow(clippy::expect_used, clippy::panic)]

mod physical_firmware;

use std::path::PathBuf;
use std::time::Duration;

use davout::{
    IDENTITY_ADMISSION_RETRY, IDENTITY_ADMISSION_SPACING, IDENTITY_ADMISSION_TIMEOUT,
    POST_SET_ZERO_BLACKOUT_FROM, POST_SET_ZERO_QUIET,
};
use marengo_config::load_control_config_from;
use physical_firmware::{SET_ZERO_BLACKOUT, TIMING_MODEL};
use serde_json::Value;

/// Margin of the post-SetZero quiet over the latest measured blackout end.
const QUIET_MARGIN: Duration = Duration::from_millis(100);
/// Margin of the blackout hold's start under the earliest measured blackout
/// start (host tick and pacing jitter).
const BLACKOUT_FROM_MARGIN: Duration = Duration::from_millis(50);

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn profile() -> Value {
    let path = repo().join("docs/commissioning/firmware/robstride-timing-profile.json");
    let text = std::fs::read_to_string(&path).expect("committed timing profile");
    serde_json::from_str(&text).expect("profile JSON")
}

/// One measured statistic over every drive that observed it: (n, min, max) ms.
fn measured(profile: &Value, field: &str) -> (u64, f64, f64) {
    let drives = profile["drives"].as_object().expect("drives");
    let mut n = 0;
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for stats in drives.values().map(|drive| &drive[field]) {
        let count = stats["n"].as_u64().expect("n");
        if count == 0 {
            continue;
        }
        n += count;
        min = min.min(stats["min"].as_f64().expect("min"));
        max = max.max(stats["max"].as_f64().expect("max"));
    }
    assert!(n > 0, "{field}: the profile measured nothing");
    (n, min, max)
}

fn count(profile: &Value, field: &str) -> u64 {
    profile["drives"]
        .as_object()
        .expect("drives")
        .values()
        .map(|drive| drive[field].as_u64().expect("counter"))
        .sum()
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

fn assert_covers(field: &str, (low, high): (Duration, Duration), (_, min, max): (u64, f64, f64)) {
    assert!(
        ms(low) <= min && max <= ms(high),
        "{field}: measured [{min}, {max}] ms outside the emulator's [{}, {}] ms",
        ms(low),
        ms(high)
    );
}

#[test]
fn profile_is_firmware_0_3_1_42_with_frames() {
    let profile = profile();
    assert_eq!(profile["firmware"], "0.3.1.42");
    let captures = profile["captures"].as_array().expect("captures");
    assert!(!captures.is_empty());
    let frames: u64 = captures
        .iter()
        .map(|capture| capture["frames"].as_u64().expect("frames"))
        .sum();
    assert_eq!(frames, profile["frames"].as_u64().expect("frames"));
}

#[test]
fn emulator_timing_model_covers_every_measured_range() {
    let profile = profile();
    assert_covers(
        "enable_to_run_ms",
        TIMING_MODEL.enable_to_run,
        measured(&profile, "enable_to_run_ms"),
    );
    assert_covers(
        "identity_reply_ms",
        TIMING_MODEL.identity_reply,
        measured(&profile, "identity_reply_ms"),
    );
    assert_covers(
        "report_period_ms",
        TIMING_MODEL.report_period,
        measured(&profile, "report_period_ms"),
    );
    assert_covers(
        "mit_reply_ms",
        TIMING_MODEL.mit_reply,
        measured(&profile, "mit_reply_ms"),
    );
    // The measured start is the drive's last frame before the silence; the
    // true start is up to one report period later, still inside the model.
    let start = measured(&profile, "set_zero_silence_start_ms");
    assert_covers(
        "set_zero_silence_start_ms",
        TIMING_MODEL.set_zero_blackout_start,
        (start.0, start.1, start.2 + ms(TIMING_MODEL.report_period.0)),
    );
    let length = measured(&profile, "set_zero_silence_ms");
    assert_covers(
        "set_zero_silence_ms",
        TIMING_MODEL.set_zero_blackout_length,
        length,
    );
    // The blackout envelope spans every modeled (and so every measured) one.
    assert_eq!(SET_ZERO_BLACKOUT.0, TIMING_MODEL.set_zero_blackout_start.0);
    assert_eq!(
        SET_ZERO_BLACKOUT.1,
        TIMING_MODEL.set_zero_blackout_start.1 + TIMING_MODEL.set_zero_blackout_length.1
    );
    assert!(start.2 + length.2 <= ms(SET_ZERO_BLACKOUT.1));
}

#[test]
fn post_set_zero_quiet_clears_the_latest_blackout_end_with_margin() {
    let profile = profile();
    let (_, _, latest_start) = measured(&profile, "set_zero_silence_start_ms");
    let (_, _, longest) = measured(&profile, "set_zero_silence_ms");
    // Upper bound of every measured blackout end (start and length may come
    // from different SetZeros).
    let latest_end = latest_start + longest;
    assert!(
        ms(POST_SET_ZERO_QUIET) >= latest_end + ms(QUIET_MARGIN),
        "POST_SET_ZERO_QUIET {} ms < measured blackout end {latest_end} ms + {} ms",
        ms(POST_SET_ZERO_QUIET),
        ms(QUIET_MARGIN)
    );
    // ... and every blackout the emulator can model.
    assert!(POST_SET_ZERO_QUIET > SET_ZERO_BLACKOUT.1);
}

#[test]
fn type_24_hold_starts_before_the_earliest_blackout_start_with_margin() {
    let profile = profile();
    let (_, earliest, _) = measured(&profile, "set_zero_silence_start_ms");
    // The drive acts on a type-24 write until the blackout begins; a write
    // held back from POST_SET_ZERO_BLACKOUT_FROM on never lands inside one.
    assert!(
        ms(POST_SET_ZERO_BLACKOUT_FROM) + ms(BLACKOUT_FROM_MARGIN) <= earliest,
        "POST_SET_ZERO_BLACKOUT_FROM {} ms + {} ms > earliest measured start {earliest} ms",
        ms(POST_SET_ZERO_BLACKOUT_FROM),
        ms(BLACKOUT_FROM_MARGIN)
    );
    // ... and every blackout the emulator can model.
    assert!(POST_SET_ZERO_BLACKOUT_FROM < SET_ZERO_BLACKOUT.0);
    assert!(POST_SET_ZERO_BLACKOUT_FROM < POST_SET_ZERO_QUIET);
}

#[test]
fn identity_admission_outlasts_a_blackout_and_a_reply() {
    let profile = profile();
    let (_, _, reply) = measured(&profile, "identity_reply_ms");
    let (_, _, silence) = measured(&profile, "set_zero_silence_ms");
    // A request answered normally is not re-sent before its reply.
    assert!(ms(IDENTITY_ADMISSION_RETRY) > reply);
    // A first request lost at the start of the longest blackout is re-sent
    // every retry period; the first one after the blackout must still be
    // answered inside the admission window.
    // Requests leave one spacing apart: the last of n targets starts (n - 1)
    // spacings late, and a retry waits for its turn among the missing targets.
    // The deadline grows by two spacings per further target.
    let spacing = ms(IDENTITY_ADMISSION_SPACING);
    for targets in [1_u32, 5, 19, 40] {
        let n = f64::from(targets);
        let retry = ms(IDENTITY_ADMISSION_RETRY).max(n * spacing);
        let worst = (n - 1.0) * spacing + silence + retry + reply;
        let deadline = ms(IDENTITY_ADMISSION_TIMEOUT) + 2.0 * (n - 1.0) * spacing;
        assert!(
            deadline > worst,
            "{targets} targets: deadline {deadline} ms <= stagger + blackout {silence} + retry {retry} + reply {reply} ms"
        );
    }
}

#[test]
fn strict_run_check_from_the_enable_echo_never_sees_reset() {
    let profile = profile();
    // Measured from each Enable's own echo: the Run reply always follows it,
    // and no Reset status or report from that drive lies between the two, so
    // arming the strict Run expectation at the echo cannot trip a healthy drive.
    let (n, earliest, _) = measured(&profile, "enable_to_run_ms");
    assert!(earliest > 0.0);
    assert_eq!(count(&profile, "reset_after_enable"), 0, "{n} Enables");
    assert_eq!(count(&profile, "enable_never_run"), 0, "{n} Enables");
}

#[test]
fn reporting_off_settles_within_one_control_period() {
    // An Enable waits one control period after its type-24 Off echo; no report
    // may follow the Off later than that.
    let control = load_control_config_from(repo().join("config")).expect("control.yaml");
    let period_ms = 1e3 / f64::from(control.control.loop_hz);
    let (_, _, last) = measured(&profile(), "report_off_to_last_ms");
    assert!(
        last < period_ms,
        "report {last} ms after Off >= {period_ms} ms"
    );
}
