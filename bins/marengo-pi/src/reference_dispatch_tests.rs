#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use berthier::ControlLoop;
use davout::MemoryBus;

use crate::enable_gate::EnableGate;
use crate::reference_queue::ReferenceEvent;
use crate::{
    defers_while_referencing, dispatch_stdin_command, parse_command, PiCommand, PiReferenceQueue,
};

const JOINT: &str = "right_elbow_pitch";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Plain owner without a reference backend: requests refuse as unsupported.
fn plain_loop() -> ControlLoop<MemoryBus> {
    ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
}

fn home_joints(line: &str) -> (Vec<String>, bool) {
    match parse_command(line) {
        Some(PiCommand::HomeJoints {
            joints,
            sign_tested,
        }) => (joints, sign_tested),
        _ => panic!("expected HomeJoints for {line:?}"),
    }
}

#[test]
fn parse_plain_home_keeps_readiness_command() {
    assert!(matches!(parse_command("home"), Some(PiCommand::Home)));
}

#[test]
fn parse_home_joints_with_sign_tested_flags() {
    assert_eq!(
        home_joints("home a b sign-tested"),
        (vec!["a".to_string(), "b".to_string()], true)
    );
    assert_eq!(
        home_joints("home a --sign-tested"),
        (vec!["a".to_string()], true)
    );
    assert_eq!(home_joints("home a"), (vec!["a".to_string()], false));
    assert_eq!(home_joints("home sign-tested"), (Vec::new(), true));
}

#[test]
fn only_home_joints_disable_and_quit_bypass_deferral() {
    assert!(!defers_while_referencing(&PiCommand::HomeJoints {
        joints: Vec::new(),
        sign_tested: true,
    }));
    assert!(!defers_while_referencing(&PiCommand::Disable));
    assert!(!defers_while_referencing(&PiCommand::Quit));
    assert!(defers_while_referencing(&PiCommand::Home));
    assert!(defers_while_referencing(&PiCommand::Status));
    assert!(defers_while_referencing(&PiCommand::HoldOn));
}

#[test]
fn home_refusals_queue_nothing() {
    let mut loop_ctrl = plain_loop();
    let mut queue = PiReferenceQueue::new("marengo-pi-test".into());
    let mut gate = EnableGate::default();
    let config = repo_root().join("config");
    for line in [
        format!("home {JOINT}"),
        "home no_such_joint sign-tested".to_string(),
        "home sign-tested".to_string(),
    ] {
        let cmd = parse_command(&line).expect("parsed");
        assert!(dispatch_stdin_command(
            &mut loop_ctrl,
            &mut queue,
            &mut gate,
            cmd,
            &config
        ));
        assert!(!queue.is_busy(), "{line:?} must queue nothing");
    }
}

#[test]
fn stdin_discards_deferred_commands_after_reference_failure() {
    let mut loop_ctrl = plain_loop();
    let mut queue = PiReferenceQueue::new("marengo-pi-test".into());
    let mut gate = EnableGate::default();
    let config = repo_root().join("config");
    let home = parse_command(&format!("home {JOINT} {JOINT} sign-tested")).expect("parsed");
    assert!(dispatch_stdin_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        home,
        &config
    ));
    assert!(queue.is_busy());
    for cmd in [
        PiCommand::Status,
        PiCommand::Enable {
            operator_id: "bench".into(),
        },
        PiCommand::HoldOn,
    ] {
        assert!(dispatch_stdin_command(
            &mut loop_ctrl,
            &mut queue,
            &mut gate,
            cmd,
            &config
        ));
    }
    assert!(queue.take_ready_deferred().is_none(), "commands defer");

    let events = queue.pump(loop_ctrl.supervisor_mut());
    assert_eq!(
        events,
        vec![
            ReferenceEvent::Failed {
                joint: JOINT.into(),
                message: "qualified reference acquisition is unsupported by this owner".into(),
            },
            ReferenceEvent::Skipped {
                joint: JOINT.into()
            },
            ReferenceEvent::DeferredDiscarded { count: 3 },
        ]
    );
    assert!(!queue.is_busy());
    assert!(queue.take_ready_deferred().is_none());
}

#[test]
fn disable_and_quit_cancel_the_queue() {
    let mut loop_ctrl = plain_loop();
    let config = repo_root().join("config");
    for (stop, keep_running) in [(PiCommand::Disable, true), (PiCommand::Quit, false)] {
        let mut queue = PiReferenceQueue::new("marengo-pi-test".into());
        let mut gate = EnableGate::default();
        let home = parse_command(&format!("home {JOINT} sign-tested")).expect("parsed");
        dispatch_stdin_command(&mut loop_ctrl, &mut queue, &mut gate, home, &config);
        dispatch_stdin_command(
            &mut loop_ctrl,
            &mut queue,
            &mut gate,
            PiCommand::HoldOn,
            &config,
        );
        assert_eq!(
            dispatch_stdin_command(&mut loop_ctrl, &mut queue, &mut gate, stop, &config),
            keep_running
        );
        assert!(!queue.is_busy());
        assert!(
            queue.take_ready_deferred().is_none(),
            "cancel discards deferred commands"
        );
    }
}
