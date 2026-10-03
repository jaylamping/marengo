# LOC (baseline a2b55b3)

Method: physical lines per file (`wc -l` equivalent).

## Per package (src vs tests)

| package | kind | src lines | test lines | src files | test files |
|---------|------|-----------|------------|-----------|------------|
| armee-dynamics | crate | 1514 | 556 | 3 | 3 |
| armee-kinematics | crate | 740 | 0 | 3 | 0 |
| armee-proto | crate | 143 | 0 | 1 | 0 |
| berthier | crate | 10628 | 2608 | 21 | 14 |
| chappe | crate | 1776 | 490 | 5 | 3 |
| davout | crate | 17375 | 10800 | 26 | 21 |
| fouche | crate | 1 | 0 | 1 | 0 |
| imu-probe | bin | 174 | 0 | 1 | 0 |
| marengo-candump | crate | 1007 | 449 | 3 | 3 |
| marengo-config | crate | 5124 | 579 | 10 | 1 |
| marengo-deploy | crate | 1048 | 59 | 8 | 1 |
| marengo-gateway | bin | 7791 | 639 | 19 | 2 |
| marengo-homing | crate | 1325 | 1242 | 6 | 5 |
| marengo-host-metrics | crate | 1354 | 0 | 4 | 0 |
| marengo-imu | crate | 916 | 0 | 7 | 0 |
| marengo-jetson | bin | 6 | 0 | 1 | 0 |
| marengo-limit-sync | bin | 71 | 0 | 1 | 0 |
| marengo-log-cli | bin | 1752 | 1048 | 2 | 5 |
| marengo-pi | bin | 8764 | 0 | 17 | 0 |
| marengo-store | crate | 2755 | 6810 | 9 | 16 |
| marengo-support | crate | 10 | 0 | 1 | 0 |
| motor-repl | bin | 465 | 0 | 1 | 0 |
| probe | bin | 6 | 0 | 1 | 0 |
| robstride | crate | 3772 | 1851 | 12 | 7 |
| sim-harness | crate | 113 | 0 | 1 | 0 |
| talleyrand | crate | 1 | 0 | 1 | 0 |
| teleop | bin | 8 | 0 | 1 | 0 |
| wave-demo | bin | 4 | 0 | 1 | 0 |
| **workspace total** | | **68643** | **27131** | | |

## 40 largest `.rs` files (by line count)

| lines | path |
|------:|------|
| 4950 | `crates/davout/src/lib.rs` |
| 3396 | `crates/berthier/src/loop.rs` |
| 1852 | `crates/berthier/src/position_hold.rs` |
| 1812 | `crates/davout/src/reference_transaction.rs` |
| 1706 | `crates/marengo-config/src/lib.rs` |
| 1697 | `bins/marengo-pi/src/main.rs` |
| 1551 | `bins/marengo-pi/src/shutdown_tests.rs` |
| 1529 | `crates/robstride/src/bus.rs` |
| 1353 | `bins/marengo-log-cli/src/gravity_fit.rs` |
| 1185 | `crates/marengo-store/src/recovery.rs` |
| 1097 | `crates/davout/src/reference_journal.rs` |
| 963 | `crates/marengo-store/src/store.rs` |
| 940 | `crates/davout/src/reference_journal_tests.rs` |
| 919 | `crates/armee-dynamics/src/calibration.rs` |
| 909 | `bins/marengo-pi/src/overlay_tests.rs` |
| 881 | `crates/davout/src/simulation.rs` |
| 879 | `crates/davout/src/feedback_consumer.rs` |
| 845 | `crates/davout/src/reference_grant_tests.rs` |
| 822 | `crates/marengo-config/src/urdf_merge.rs` |
| 820 | `bins/marengo-gateway/src/http.rs` |
| 760 | `crates/chappe/src/ipc.rs` |
| 716 | `crates/berthier/src/position_setpoint.rs` |
| 700 | `bins/marengo-gateway/src/hardware_tests.rs` |
| 690 | `bins/marengo-pi/src/overlay.rs` |
| 666 | `crates/davout/src/reference_codec.rs` |
| 643 | `crates/berthier/src/gain_runtime.rs` |
| 637 | `bins/marengo-gateway/src/hardware.rs` |
| 631 | `crates/berthier/src/friction.rs` |
| 628 | `crates/davout/src/active_reporting.rs` |
| 626 | `crates/davout/src/reference_commit.rs` |
| 610 | `bins/marengo-gateway/src/logs.rs` |
| 610 | `bins/marengo-gateway/src/webtransport.rs` |
| 582 | `bins/marengo-gateway/src/actuator.rs` |
| 575 | `crates/robstride/src/lib.rs` |
| 556 | `bins/marengo-pi/src/reference_shutdown_tests.rs` |
| 549 | `crates/marengo-config/src/profile_txn.rs` |
| 541 | `bins/marengo-pi/src/limit_persist.rs` |
| 517 | `crates/marengo-host-metrics/src/lib.rs` |
| 506 | `crates/davout/src/reference_urdf_codec.rs` |
| 487 | `crates/marengo-candump/src/scan.rs` |
