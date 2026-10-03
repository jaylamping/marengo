# User decisions on prune-candidates.md (2026-10-03)

| ID | Decision |
|---|---|
| Batches | Approve every PRUNE and PRUNE-AFTER batch (B1–B16). Each batch lands as one commit and must pass fmt, clippy and tests. |
| D-1 | Delete the roadmap scaffolds: `fouche`, `talleyrand`, `teleop`, and the `marengo-jetson` plumbing (service unit, deploy script, host-metrics Jetson branch, `host/metrics/jetson` slot, `network.yaml` loader). ADR 0014 stays as the design record. |
| D-2 | Delete the Chappe `transport.rs` NATS/MQTT seam. |
| D-3 | Retire the ADR 0006/0022 history, verifier and Ready language. Fold what remains of marengo-homing into Davout, and keep `calibration_record_path` because it locates the reference journal. |
| D-4 | Delete the dormant Hall-sensor homing module. |
| D-5 | Keep `sim-harness` as the M6 extension point. |
| D-6 | Enforce the log retention settings (`log_archive_days`, `log_disk_budget_bytes`) instead of deleting them. |
| D-7 | Delete `recover_known_v2`/`recovery.rs` and `import-legacy`, but only if the Pi holds no marker-1 DB and no pre-store hot files (check read-only first). |
| D-8 | Delete `robot.bench.max_joint_velocity_rad_s`. |
| D-9 | Delete the write-only `JointFacetInput.drive_active` facet. |
| D-10 | Remove the stale `~/.codex` worktrees and local `codex/*` branches that hold no unpushed work. Remote branches stay. |
| D-11 | Keep the gain-tuning route (`/command/actuator`, `TuningTier`, `robot/audit/tuning`) and document it as unwired. |
