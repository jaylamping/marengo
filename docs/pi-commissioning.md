# Pi commissioning runbook — dual-shoulder bring-up through single-arm Milestone B.
#
# Hardware confirmed (prototype):
#   left_shoulder_pitch  can1  device_id 12  firmware 0.3.1.42
#   right_shoulder_pitch can0  device_id 2   firmware 0.3.1.42
#
# See also: docs/safety.md, hardware/electrical/wiring/can_topology.md

## Phase 0 — Bench pre-flight

- [ ] CAD joint names deferred; YAML uses `*_shoulder_pitch`
- [x] Firmware IDs: left **12**, right **2**; set-zero after ID change
- [ ] CAN termination verified (`0x3041 can_status` in Robstride tool if bus errors)
- [ ] E-stop ordered: NC mushroom in **motor power** path (TWTADE/Schneider); GPIO 17 sense optional initially

## Phase 1 — Pi OS and CAN HAT

**OS:** Raspberry Pi OS Lite 64-bit (Bookworm). Enable SSH + your public key in Imager.

```bash
sudo apt update && sudo apt full-upgrade -y
sudo apt install -y iproute2 can-utils git
sudo usermod -aG dialout $USER   # re-login
```

**Waveshare 2-CH CAN HAT (`/boot/firmware/config.txt`):**

```text
dtparam=spi=on
dtoverlay=mcp2515-can0,oscillator=12000000,interrupt=25
dtoverlay=mcp2515-can1,oscillator=12000000,interrupt=24
```

Reboot, then:

```bash
ip link show type can
# Manual (one-off): sudo ./scripts/can-up.sh can0 can1
# Automatic (recommended after install-pi.sh): marengo-can.service enables on boot
sudo systemctl enable --now marengo-can.service
systemctl status marengo-can.service
ip -br link show type can   # expect can0 UP, can1 UP
```

After `install-pi.sh`, `marengo-can` is **enabled and started** — no manual `can-up` needed on each reboot unless the unit failed (check `journalctl -u marengo-can`).

**Security (recommended):** SSH key-only, `fail2ban`, `ufw` allow SSH from bench subnet, `unattended-upgrades`.

## Phase 2 — Build Marengo on the Pi

```bash
git clone <repo-url> /opt/marengo
cd /opt/marengo && git lfs pull
# Install Rust 1.88 + protoc (see docs/dev-setup.md native section)
cargo build -p motor-repl -p marengo-pi --features socketcan --release
sudo ./scripts/install-pi.sh
```

Cross-build from laptop: `./scripts/deploy-pi.sh user@marengo` then `install-pi.sh` on Pi.

**Environment** (`/etc/marengo/env` — template in `scripts/env.example`):

```bash
MARENGO_ROOT=/opt/marengo
MARENGO_CONFIG_DIR=/opt/marengo/config/bringup/shoulder_pitch_dual
RUST_LOG=robstride=info,davout=info,berthier=info,motor_repl=info
```

## Phase 3 — Config profiles

| Milestone | `MARENGO_CONFIG_DIR` | Joints |
|-----------|----------------------|--------|
| **A — dual shoulders** | `config/bringup/shoulder_pitch_dual` | left/right pitch on can0/can1 |
| **A1 — right bench only** | `config/bringup/shoulder_pitch_right_only` | `right_shoulder_pitch` on can0, id 2 |
| **A1 — left bench only** | `config/bringup/shoulder_pitch_left_only` | `left_shoulder_pitch` on can1, id 12 (mirrors right tuning) |
| **B — left 4-DOF arm** | `config/bringup/arm_4dof_left` or `config` | all on can0, IDs 11–14 |

Single-shoulder bench profiles share impedance **kp=12 / kd=0.75**, slew **15 rad/s**, max lead **0.5**, trim **-0.043**, upper **1.65 rad**. Commission each side with **`pi_set_zero`** at mechanical home before hold-at round trips.

Trim `motors.yaml` joint list to only wired actuators while building the arm incrementally.

## Phase 4 — CAN validation (no enable)

```bash
cd /opt/marengo
export MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual

RUST_LOG=robstride=trace,davout=debug \
  ./target/release/motor-repl status

# Single bus debug:
MARENGO_CAN_INTERFACE=can0 ./target/release/motor-repl --can-interface can0 status
```

**Pass:** both interfaces open; trace shows TX/RX for id **2** on can0 (robot right) and id **12** on can1 (robot left).

Raw bus (motor power on):

```bash
candump -ta can0 &
cansend can0 0300FF0C#0000000000000000   # enable probe id 12
cansend can0 0400FF0C#0000000000000000   # disable
```

## Phase 5 — Safe motor test ([safety.md](safety.md))

Each `motor-repl` command is a **separate process**; `enable` and `jog` mark homing complete for bench use. Use **`marengo-pi`** for gravity comp (running control loop).

1. E-stop reachable; shoulder supported
2. `motor-repl enable bench` (or `home` then `enable` in `marengo-pi`)
3. Sign test per joint (small torque_ff; fix `direction` in YAML if inverted)
4. `set-zero <joint>` at mechanical zero
5. `gravity-preview` with two angles (defaults 0,0)
6. `marengo-pi` with stdin: `home`, `enable`, `gravity-on` — **not** `motor-repl gravity-on` alone

```bash
MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual ./target/release/marengo-pi
# stdin: home / enable bench / gravity-on / status / disable / quit
```

## Phase 7 — Milestone B (left arm on can0)

```bash
export MARENGO_CONFIG_DIR=config/bringup/arm_4dof_left
motor-repl --can-interface can0 status
```

Commission joints in order: shoulder_roll (11) → shoulder_pitch (12) → upper_arm_yaw (13) → elbow (14).

Remove unmounted joints from `motors.yaml` + `robot.yaml` until each actuator is wired.

## Device ID scheme (commissioned)

| Block | IDs |
|-------|-----|
| Right arm | 1–10 |
| Left arm | 11–20 |
| Right leg | 21–30 |
| Left leg | 31–40 |
| Waist + aux | 41+ |

Left arm chain: roll **11**, pitch **12**, yaw **13**, elbow **14**. Right arm: **1–4**.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `can1` missing | Check `config.txt` typos (`spi=on`, `interrupt=24` not `24n`); `dmesg \| grep mcp2515` |
| `Device or resource busy` on can0 | Already UP — use `ip link show type can` |
| Motor silent | Power, termination, wrong `device_id`, run `candump` |
| Config not found | Set `MARENGO_ROOT` + `MARENGO_CONFIG_DIR` or clone full repo |

## Agent-assisted bench

Cursor agents can drive the Pi over SSH via [`tools/marengo-pi-mcp/`](../tools/marengo-pi-mcp/README.md) (MCP server on your dev Mac).

- **Read-only / logs:** no confirmation — agent should call `pi_logs_*`, `pi_health`, `pi_motor_repl_status` freely.
- **Deploy:** `pi_sync_main` (pull `main`, cross-build, `install-pi.sh`) — pre-authorized.
- **Motion:** `confirm: true`; weighted profile also needs `confirm_weighted_motion: true` after two chat approvals.
- **Session logs:** harness and `pi_marengo_pi_script` tee to `$MARENGO_ROOT/var/log/bench-latest.log`.
- **Weighted tests backlog:** [bench-test-backlog.md](bench-test-backlog.md)

```bash
# Manual bench run with session log (same paths MCP uses)
export MARENGO_ROOT=/opt/marengo
mkdir -p "$MARENGO_ROOT/var/log"
./tools/marengo-pi-mcp/harness/log-tee.sh -- /opt/marengo/bin/motor-repl status
```
