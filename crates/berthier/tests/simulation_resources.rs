//! Public simulation constructors must use the supplied resource tree on the Pi.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use berthier::ControlLoop;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{OperationalMode, Supervisor};

const CHILD_CASE: &str = "MARENGO_SIMULATION_RESOURCE_CASE";
const FIXTURE_ROOT: &str = "MARENGO_SIMULATION_FIXTURE_ROOT";

fn check_owner(owner: &Supervisor<SimulationBus>) {
    assert_eq!(owner.mode(), OperationalMode::Disabled);
    assert_eq!(owner.motors.motors.len(), 5);
    assert!(!owner.control.control.bench.active_reporting_diagnostics);
    assert!(owner.bus().transmissions().is_empty());
}

#[allow(clippy::panic)] // An unknown child selector is a test harness error.
fn check_child(root: &Path, case: &str) {
    if case == "ordinary" {
        let owner = Supervisor::from_repo(root, SimulationBus::default())
            .expect("ordinary runtime constructor uses ambient configuration");
        assert_eq!(owner.mode(), OperationalMode::Disabled);
        assert!(owner.control.control.bench.active_reporting_diagnostics);
        // Construction writes the first reporting On per interface; `sync`
        // writes the rest one per interface per control period.
        let interfaces: std::collections::HashSet<&str> = owner
            .motors
            .motors
            .iter()
            .map(|motor| motor.can_interface.as_str())
            .collect();
        assert_eq!(owner.bus().transmissions().len(), interfaces.len());
        assert!(owner
            .bus()
            .transmissions()
            .iter()
            .all(|tx| tx.frame.id >> 24 == 24));
        return;
    }
    let invalid = case.ends_with("-invalid");
    if invalid {
        std::fs::write(
            root.join("config/control.yaml"),
            "control: [invalid schema]",
        )
        .expect("invalidate only copied fixture");
    }
    let build = case.trim_end_matches("-invalid");
    let result = match build {
        "supervisor" => Supervisor::from_simulation(
            root,
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .map(|owner| {
            if !invalid {
                check_owner(&owner);
            }
        })
        .map_err(|error| error.to_string()),
        "controller" => ControlLoop::from_simulation(
            root,
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
            200,
            50,
        )
        .map(|controller| {
            if !invalid {
                assert_eq!(controller.joint_names().len(), 5);
                check_owner(controller.supervisor());
            }
        })
        .map_err(|error| error.to_string()),
        _ => panic!("unknown frozen constructor case"),
    };
    assert_eq!(
        result.is_err(),
        invalid,
        "invalid copied resources must be rejected"
    );
}

#[test]
fn public_simulation_constructors_ignore_competing_config() {
    if let Ok(case) = std::env::var(CHILD_CASE) {
        let root = PathBuf::from(std::env::var_os(FIXTURE_ROOT).expect("owned copied resources"));
        check_child(&root, &case);
        return;
    }
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ambient = support::FixtureTree::new("simulation-ambient", &source);
    let mut failures = Vec::new();
    for constructor in ["supervisor", "controller", "ordinary"] {
        for suffix in ["", "-invalid"] {
            if constructor == "ordinary" && !suffix.is_empty() {
                continue;
            }
            let case = format!("{constructor}{suffix}");
            let fixture = support::fixture_tree_without_diagnostics("simulation-selected", &source);
            let output = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "public_simulation_constructors_ignore_competing_config",
                    "--nocapture",
                ])
                .env(CHILD_CASE, &case)
                .env(FIXTURE_ROOT, fixture.path())
                .env("MARENGO_CONFIG_DIR", ambient.path().join("config"))
                .env_remove("MARENGO_JOINT_SUBSET")
                .output()
                .expect("isolated constructor process");
            if !output.status.success() {
                failures.push(format!(
                    "{case}: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
