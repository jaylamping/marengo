#![allow(clippy::expect_used, clippy::panic)]

use super::*;

const SOURCES: [CommandSource; 2] = [CommandSource::Stdin, CommandSource::Chappe];

#[test]
fn owner_is_admitted_and_non_owner_is_refused_for_motion() {
    for owner in SOURCES {
        let lease = MotionLease::new(owner);
        for from in SOURCES {
            let verdict = lease.admit(from, CommandClass::Motion, "hold-at");
            if from == owner {
                assert_eq!(verdict, Ok(()));
            } else {
                let refusal = verdict.expect_err("non-owner motion must be refused");
                assert_eq!(refusal.owner, owner);
                assert_eq!(refusal.from, from);
                let text = refusal.to_string();
                assert!(text.contains("hold-at refused"), "{text}");
                assert!(text.contains(&format!("owned by {owner}")), "{text}");
            }
        }
    }
}

/// Stop never needs ownership (L-marengo-pi-01): both sources, both owners.
#[test]
fn stop_and_observe_are_admitted_from_every_source_under_every_owner() {
    for owner in SOURCES {
        let lease = MotionLease::new(owner);
        for from in SOURCES {
            assert_eq!(lease.admit(from, CommandClass::Stop, "disable"), Ok(()));
            assert_eq!(lease.admit(from, CommandClass::Observe, "status"), Ok(()));
        }
    }
}

#[test]
fn every_stdin_command_has_a_class_and_only_stops_are_ownerless() {
    let cases: Vec<(&str, CommandClass)> = vec![
        ("disable", CommandClass::Stop),
        ("quit", CommandClass::Stop),
        ("hold-off", CommandClass::Stop),
        ("impedance-off", CommandClass::Stop),
        ("status", CommandClass::Observe),
        ("home", CommandClass::Motion),
        ("home a sign-tested", CommandClass::Motion),
        ("enable bench", CommandClass::Motion),
        ("enable bench force", CommandClass::Motion),
        ("gravity-on", CommandClass::Motion),
        ("gravity-off", CommandClass::Motion),
        ("torque-cmd a 0.1", CommandClass::Motion),
        ("impedance-on", CommandClass::Motion),
        ("hold-on", CommandClass::Motion),
        ("hold-at 0.1", CommandClass::Motion),
        ("hold-at a 0.1", CommandClass::Motion),
        ("wave a 0 1 1", CommandClass::Motion),
    ];
    for (line, expected) in cases {
        let cmd = crate::parse_command(line).unwrap_or_else(|| panic!("{line:?} parses"));
        assert_eq!(classify_stdin(&cmd).0, expected, "{line}");
    }
}

#[test]
fn owner_resolution_prefers_flag_then_env_then_chappe() {
    assert_eq!(resolve_owner(None, None), Ok(CommandSource::Chappe));
    assert_eq!(resolve_owner(None, Some("stdin")), Ok(CommandSource::Stdin));
    assert_eq!(
        resolve_owner(Some("chappe"), Some("stdin")),
        Ok(CommandSource::Chappe)
    );
    assert!(resolve_owner(Some("both"), None).is_err());
    assert!(resolve_owner(None, Some("")).is_err());
}
