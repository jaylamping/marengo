#![allow(clippy::expect_used)]

use std::collections::{HashMap, VecDeque};

use davout::{ReferenceAudit, ReferenceOutcome};

use super::{ReferenceDriver, ReferenceEvent, ReferenceQueue};

/// Scripted owner: per-joint request result and a sequence of outcomes.
#[derive(Default)]
struct FakeDriver {
    estop: bool,
    refuse: HashMap<String, String>,
    outcomes: HashMap<String, VecDeque<Result<ReferenceOutcome, String>>>,
    requests: Vec<(String, ReferenceAudit)>,
}

impl FakeDriver {
    fn script(&mut self, joint: &str, outcomes: Vec<Result<ReferenceOutcome, String>>) {
        self.outcomes.insert(joint.into(), outcomes.into());
    }

    fn requested(&self) -> Vec<&str> {
        self.requests.iter().map(|(j, _)| j.as_str()).collect()
    }
}

impl ReferenceDriver for FakeDriver {
    type Handle = String;

    fn hardware_estop_asserted(&self) -> bool {
        self.estop
    }

    fn request(&mut self, joint: &str, audit: ReferenceAudit) -> Result<String, String> {
        self.requests.push((joint.into(), audit));
        match self.refuse.get(joint) {
            Some(message) => Err(message.clone()),
            None => Ok(joint.into()),
        }
    }

    fn outcome(&self, handle: &String) -> Result<ReferenceOutcome, String> {
        // Peek: the queue polls once per pump; tests pop between pumps.
        self.outcomes
            .get(handle)
            .and_then(|q| q.front().cloned())
            .unwrap_or(Ok(ReferenceOutcome::InProgress))
    }
}

impl FakeDriver {
    fn advance(&mut self, joint: &str) {
        if let Some(q) = self.outcomes.get_mut(joint) {
            q.pop_front();
        }
    }
}

type Queue = ReferenceQueue<String, &'static str>;

fn joints(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

fn known(joint: &str) -> bool {
    matches!(joint, "a" | "b" | "c")
}

fn lines(events: &[ReferenceEvent]) -> Vec<String> {
    events.iter().map(ToString::to_string).collect()
}

fn current(position_rad: f32) -> Result<ReferenceOutcome, String> {
    Ok(ReferenceOutcome::Current { position_rad })
}

#[test]
fn admit_refuses_missing_sign_tested_unknown_joint_and_busy() {
    let mut queue = Queue::new("s".into());
    assert_eq!(
        queue.admit(&joints(&["a"]), false, "bench", known),
        Err("sign-tested required (operator attestation)".into())
    );
    assert_eq!(
        queue.admit(&[], true, "bench", known),
        Err("no joints given".into())
    );
    assert_eq!(
        queue.admit(&joints(&["a", "zz"]), true, "bench", known),
        Err("unknown joint zz".into())
    );
    assert!(!queue.is_busy(), "refusals queue nothing");
    assert_eq!(queue.admit(&joints(&["a"]), true, "bench", known), Ok(()));
    assert_eq!(
        queue.admit(&joints(&["b"]), true, "bench", known),
        Err("reference queue busy".into())
    );
}

#[test]
fn joints_acquire_in_order_one_at_a_time() {
    let mut queue = Queue::new("marengo-pi-7".into());
    let mut driver = FakeDriver::default();
    driver.script(
        "a",
        vec![Ok(ReferenceOutcome::InProgress), current(0.00012)],
    );
    driver.script("b", vec![current(-0.5)]);
    queue
        .admit(&joints(&["a", "b"]), true, "bench", known)
        .expect("admit");

    assert!(queue.pump(&mut driver).is_empty());
    assert_eq!(driver.requested(), vec!["a"]);
    assert_eq!(driver.requests[0].1.operator, "bench");
    assert_eq!(driver.requests[0].1.session, "marengo-pi-7");

    assert!(queue.pump(&mut driver).is_empty(), "a still in progress");
    driver.advance("a");
    let events = queue.pump(&mut driver);
    assert_eq!(lines(&events), vec!["reference a current pos=0.0001"]);
    assert_eq!(driver.requested(), vec!["a", "b"], "b starts after a");

    let events = queue.pump(&mut driver);
    assert_eq!(lines(&events), vec!["reference b current pos=-0.5000"]);
    assert!(!queue.is_busy());
    assert!(queue.pump(&mut driver).is_empty());
}

#[test]
fn failure_skips_remaining_joints_and_discards_deferred_commands() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    driver.script(
        "a",
        vec![Ok(ReferenceOutcome::Failed {
            message: "readback timeout".into(),
        })],
    );
    queue
        .admit(&joints(&["a", "b", "c"]), true, "bench", known)
        .expect("admit");
    assert!(queue.defer("enable"));
    assert!(queue.defer("hold-on"));
    assert!(queue.pump(&mut driver).is_empty());
    let events = queue.pump(&mut driver);
    assert_eq!(
        lines(&events),
        vec![
            "reference a failed: readback timeout",
            "reference b skipped: earlier joint failed",
            "reference c skipped: earlier joint failed",
            "discarded 2 deferred command(s)",
        ]
    );
    assert_eq!(driver.requested(), vec!["a"]);
    assert!(!queue.is_busy());
    assert_eq!(queue.take_ready_deferred(), None);
}

#[test]
fn deferred_commands_are_bounded() {
    let mut queue = Queue::new("s".into());
    queue
        .admit(&joints(&["a"]), true, "bench", known)
        .expect("admit");
    for _ in 0..super::MAX_DEFERRED_COMMANDS {
        assert!(queue.defer("hold-on"));
    }
    assert!(!queue.defer("enable"));
}

#[test]
fn outcome_error_counts_as_failure() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    driver.script("a", vec![Err("outcome expired".into())]);
    queue
        .admit(&joints(&["a", "b"]), true, "bench", known)
        .expect("admit");
    assert!(queue.pump(&mut driver).is_empty());
    assert_eq!(
        lines(&queue.pump(&mut driver)),
        vec![
            "reference a failed: outcome expired",
            "reference b skipped: earlier joint failed",
        ]
    );
}

#[test]
fn immediate_request_error_fails_and_skips() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    driver.script("a", vec![current(0.0)]);
    driver.refuse.insert("b".into(), "unsupported".into());
    queue
        .admit(&joints(&["a", "b", "c"]), true, "bench", known)
        .expect("admit");
    assert!(queue.pump(&mut driver).is_empty());
    assert_eq!(
        lines(&queue.pump(&mut driver)),
        vec![
            "reference a current pos=0.0000",
            "reference b failed: unsupported",
            "reference c skipped: earlier joint failed",
        ]
    );
}

#[test]
fn commands_defer_until_drained_then_replay_in_order() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    driver.script("a", vec![Ok(ReferenceOutcome::InProgress), current(0.0)]);
    assert_eq!(queue.take_ready_deferred(), None);
    queue
        .admit(&joints(&["a"]), true, "bench", known)
        .expect("admit");
    queue.defer("status");
    queue.defer("enable");
    assert_eq!(queue.take_ready_deferred(), None, "busy before request");
    assert!(queue.pump(&mut driver).is_empty());
    assert_eq!(queue.take_ready_deferred(), None, "busy while in flight");
    driver.advance("a");
    assert_eq!(queue.pump(&mut driver).len(), 1);
    assert_eq!(queue.take_ready_deferred(), Some("status"));
    assert_eq!(queue.take_ready_deferred(), Some("enable"));
    assert_eq!(queue.take_ready_deferred(), None);
}

#[test]
fn cancel_fails_unfinished_joints_and_discards_deferred() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    queue
        .admit(&joints(&["a", "b"]), true, "bench", known)
        .expect("admit");
    assert!(queue.pump(&mut driver).is_empty());
    queue.defer("enable");
    assert_eq!(
        queue.cancel(),
        vec![
            ReferenceEvent::Failed {
                joint: "a".into(),
                message: super::CANCELLED.into(),
            },
            ReferenceEvent::Failed {
                joint: "b".into(),
                message: super::CANCELLED.into(),
            },
            ReferenceEvent::DeferredDiscarded { count: 1 },
        ]
    );
    assert!(!queue.is_busy());
    assert_eq!(queue.take_ready_deferred(), None);
    assert!(queue.cancel().is_empty(), "idle cancel is silent");
}

#[test]
fn hardware_estop_cancels_queue_before_polling() {
    let mut queue = Queue::new("s".into());
    let mut driver = FakeDriver::default();
    driver.script("a", vec![current(0.0)]);
    queue
        .admit(&joints(&["a", "b"]), true, "bench", known)
        .expect("admit");
    assert!(queue.pump(&mut driver).is_empty());
    driver.estop = true;
    assert_eq!(
        lines(&queue.pump(&mut driver)),
        vec![
            "reference a failed: cancelled",
            "reference b failed: cancelled",
        ]
    );
    assert_eq!(driver.requested(), vec!["a"]);
}
