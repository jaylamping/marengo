//! Run-loop wiring tests (WP-M / WP-Q): explicit Chappe socket resolution
//! (L-marengo-pi-08) and cumulative overrun accounting (L-marengo-pi-12).

use std::time::Duration;

use super::{resolve_chappe_socket, LoopTimingWindow};

#[test]
fn missing_socket_resolves_to_none_with_no_default() {
    // Env-only contract: no fallback path is ever invented here.
    assert_eq!(resolve_chappe_socket(None), None);
}

#[test]
fn configured_socket_resolves_verbatim() {
    let path = std::path::PathBuf::from("/run/marengo/chappe.sock");
    assert_eq!(
        resolve_chappe_socket(Some(path.as_os_str().to_os_string())),
        Some(path)
    );
}

#[test]
fn overruns_accumulate_as_lifetime_total() {
    let period = Duration::from_millis(5);
    let mut timing = LoopTimingWindow::new(0);
    timing.record_tick(Duration::from_millis(1), period, 0, 0);
    assert_eq!(timing.total_overruns(), 0);
    timing.record_tick(Duration::from_millis(9), period, 0, 0);
    timing.record_tick(Duration::from_millis(6), period, 0, 0);
    assert_eq!(timing.total_overruns(), 2);
}
