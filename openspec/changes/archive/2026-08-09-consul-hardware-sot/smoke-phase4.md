# Phase 4 smoke checklist (consul-hardware-sot)

Operator or Tailscale bench verification. Cloud agents cannot complete hardware steps — record what was verified in scripts/tests only.

## Pre-flight

- [ ] `MARENGO_CONFIG_DIR=/opt/marengo/config` in `/etc/marengo/env`
- [ ] `robot.yaml` points at `assets/urdf/marengo.urdf` on Pi
- [ ] Local repo matches deployed `.deploy-rev` on Pi

## Deploy / sync

- [ ] `./scripts/deploy-pi.sh --install <pi-host>` (or `pi_sync_main`) stages root `config/` + `assets/`
- [ ] `pi_sync_bench_config` installs master YAML to `/opt/marengo/config`
- [ ] After Set Limits `persist_status=durable`, `pi_sync_bench_urdf` (or pull Pi URDF locally) — **ADR 0017**: unchecked URDF sync can clobber expand-only limits

## Runtime

- [ ] `./scripts/homing-preflight.sh` — all commissioned joints Verified
- [ ] `motor-repl status` — 4-DOF right arm on `can0`
- [ ] Enable → homing → gravity-on smoke (4-DOF)
- [ ] Optional 3-DOF harness: `MARENGO_JOINT_SUBSET=right_shoulder_roll,right_shoulder_pitch,right_upper_arm_yaw`

## Consul / Hardware

- [ ] `/hardware` shows completeness badges (warn-only)
- [ ] Inventory actuator sheet: limits read-only + deep-link to Hardware
- [ ] Set Limits on Hardware sheet → `persist_status` pending → durable

## Cloud agent (this run)

| Step | Status |
|------|--------|
| Script defaults → master paths | Verified (unit tests + grep) |
| MCP `npm test` | Run in CI gate below |
| `cargo test -p marengo-config bench_joints` | Run in CI gate below |
| `cargo test -p marengo-pi overlay` | Run in CI gate below |
| Pi deploy / Enable / homing / gravity | **Not run** (no Tailscale bench in cloud VM) |
