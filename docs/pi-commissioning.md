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
sudo ./scripts/can-up.sh
# or: sudo systemctl enable --now marengo-can.service   (after install-pi.sh)
```

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
| **B — left 4-DOF arm** | `config/bringup/arm_4dof_left` or `config` | all on can0, IDs 11–14 |

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

1. E-stop reachable; shoulder supported
2. `motor-repl home` → `enable bench`
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
