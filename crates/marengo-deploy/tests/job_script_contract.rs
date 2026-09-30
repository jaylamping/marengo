//! Contract: JSON shapes written by pi-enqueue / pi-self-update deserialize as DeployJob.

#![allow(clippy::expect_used)]

use marengo_deploy::{DeployJob, DeployJobState, DeployPhase};

fn parse(raw: &str) -> DeployJob {
    serde_json::from_str(raw).expect("fixture must deserialize as DeployJob")
}

#[test]
fn enqueue_script_job_shape() {
    let job = parse(include_str!("fixtures/deploy-job-enqueue.json"));
    assert_eq!(job.state, DeployJobState::Running);
    assert_eq!(job.phase, DeployPhase::Enqueue);
    assert_eq!(job.unit_name, "marengo-self-update");
    assert!(!job.job_id.is_empty());
    assert_eq!(job.target_sha.len(), 40);
}

#[test]
fn self_update_build_phase_shape() {
    let job = parse(include_str!("fixtures/deploy-job-build.json"));
    assert_eq!(job.state, DeployJobState::Running);
    assert_eq!(job.phase, DeployPhase::Build);
}

#[test]
fn self_update_done_phase_shape() {
    let job = parse(include_str!("fixtures/deploy-job-done.json"));
    assert_eq!(job.state, DeployJobState::Succeeded);
    assert_eq!(job.phase, DeployPhase::Done);
    assert_eq!(job.result_sha, job.target_sha);
}

#[test]
fn known_script_phases_roundtrip() {
    for (raw, expected) in [
        ("init", DeployPhase::Init),
        ("dirty", DeployPhase::Dirty),
        ("fetch", DeployPhase::Fetch),
        ("lfs", DeployPhase::Lfs),
        ("build", DeployPhase::Build),
        ("install", DeployPhase::Install),
        ("enqueue", DeployPhase::Enqueue),
        ("done", DeployPhase::Done),
        ("timeout", DeployPhase::Timeout),
        ("orphan", DeployPhase::Orphan),
        ("error", DeployPhase::Error),
    ] {
        let json = format!(
            r#"{{"state":"running","job_id":"j","target_sha":"a","result_sha":"","unit_name":"u","started_at":"","updated_at":"","message":"","phase":"{raw}"}}"#
        );
        let job: DeployJob = serde_json::from_str(&json).expect(raw);
        assert_eq!(job.phase, expected, "phase {raw}");
        let out = serde_json::to_value(&job).expect("serialize");
        assert_eq!(out["phase"], raw);
    }
}
