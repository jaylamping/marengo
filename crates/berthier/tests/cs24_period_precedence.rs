//! Candidate typed-period conformance; not an original-interface regression.

use berthier::{ControlLoop, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus};

#[test]
fn zero_rounded_period_is_refused_before_configuration_or_virtual_reference() {
    // A missing configuration tree must not mask period refusal. This also prevents
    // Supervisor construction (including configured startup diagnostics) from running.
    let absent_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("period-validation-must-precede-this-absent-config");
    assert!(!absent_root.exists());
    let result = ControlLoop::from_simulation(
        absent_root,
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec!["not-an-installed-joint".to_string()]),
        u32::MAX,
        50,
    );
    assert!(
        matches!(result, Err(LoopError::InvalidLoopPeriod { seconds })
        if seconds.is_finite() && seconds > 0.0 && seconds < 0.5e-9)
    );
}
