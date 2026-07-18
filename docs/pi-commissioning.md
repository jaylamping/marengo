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
sudo apt install -y iproute2 can-utils git i2c-tools
sudo usermod -aG dialout,i2c $USER   # re-login
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

### BNO085 torso IMU (I2C)

Commissioned on the Pi 5 bench: **Teyleten Robot GY-BNO085** on header I2C at **`0x4b`** (ADR/SA0 high; default breakout address is `0x4a`).

**Enable I2C** in `/boot/firmware/config.txt` (requires `sudo`):

```text
dtparam=i2c_arm=on
dtparam=i2c_arm_baudrate=400000
```

Reboot, then verify:

```bash
ls -l /dev/i2c-*
i2cdetect -y 1
# expect 0x4b (Pi 5 uses i2c-1 for header pins; i2c-0 is absent — ignore i2cdetect -y 0 errors)
```

**Wiring (3.3 V logic only):**

| BNO085 | Pi 5 header |
|--------|-------------|
| VCC | 3.3 V (pin 1) |
| GND | GND (pin 6) |
| SDA | pin 3 (GPIO2 / SDA) |
| SCL | pin 5 (GPIO3 / SCL) |

**Rust probe (after deploy):**

```bash
/opt/marengo/bin/imu-probe --bus /dev/i2c-1 --address 0x4b --samples 10
```

**Python smoke test (optional fallback):** Bookworm/Trixie block system `pip`; use a venv with system site-packages so `python3-lgpio` works on Pi 5:

```bash
sudo apt install -y python3-venv python3-lgpio
python3 -m venv --system-site-packages ~/imu-venv
~/imu-venv/bin/pip install adafruit-blinka adafruit-circuitpython-bno08x
# pass address=0x4b to the driver (not default 0x4a)
```

See [hardware/electrical/wiring/harness.md](../hardware/electrical/wiring/harness.md) for harness notes.

**Security (recommended):** SSH key-only, `fail2ban`, `ufw` allow SSH from bench subnet, `unattended-upgrades`.

## Phase 2 — Build Marengo on the Pi

```bash
git clone <repo-url> ~/marengo
cd ~/marengo && git lfs pull
# Install Rust 1.88.0 + protoc (see docs/dev-setup.md native section)
cargo build -p motor-repl -p marengo-pi --features socketcan --release
sudo ./scripts/install-pi.sh
```

Cross-build from laptop: `./scripts/deploy-pi.sh --install joey@marengo.local`. `install-pi.sh` provisions `/opt/marengo`, group-writable config/calibration directories, and narrow passwordless sudo rules for future MCP config sync/install.

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
| **A1 — right bench only** | `config/bringup/arm_2dof_right` | `right_shoulder_pitch` on can0, id 2 |
| **A1 — left bench only** | `config/bringup/shoulder_pitch_left_only` | `left_shoulder_pitch` on can1, id 12 (mirrors right tuning) |
| **B — left 4-DOF arm** | `config/bringup/arm_4dof_left` or `config` | all on can0, IDs 11–14 |

Single-shoulder bench profiles use conservative position hold tuning while feedback velocity guards are active. The current right-only profile uses impedance **kp=8 / kd=1.25**, trapezoid **v=2.0 / a=4.8**, slew **0.15 rad/s**, max lead **0.10**, and trim **0.0** once firmware zero is set at arm-down. Operator shoulder-pitch limits are **[-0.872665, 3.141593] rad** (arm down = 0, -50° to +180°) from URDF `safety_controller`; hard bench/URDF limits are **[-0.9, 3.17] rad**. Berthier/Davout apply a velocity-scaled command envelope (ADR 0009) so fast moves shrink the commandable band before hard limits — full-range sweeps may reach soft limits only after decel, not in one gravity-assisted leg.

### Homing (interim — manual reference)

Until Hall sensors are installed on shoulder roll (see [hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md)):

1. Place arm at mechanical reference (arm down for pitch).
2. **`pi_set_zero`** with `confirm: true` — verifies |pos| < tolerance, writes calibration record.
3. **`motor-repl home`** or `marengo-pi` stdin `home` — all joints must be Verified before Ready.
4. **`enable`** only after homing succeeds.

Do not use `pi_hold_on` with `set_zero: true` unless intentionally recalibrating at mechanical zero.

Full homing contract: [homing.md](homing.md).

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

Each `motor-repl` command is a **separate process**. **`enable` requires verified homing** — run `set-zero` then `home` first. Use **`marengo-pi`** for gravity comp (running control loop).

1. E-stop reachable; shoulder supported
2. Sign test per joint (small torque_ff; fix `direction` in YAML if inverted)
3. `set-zero <joint>` at mechanical zero (writes calibration + marks Verified)
4. `motor-repl home` (supervisor Ready when all joints Verified)
5. `gravity-preview` with two angles (defaults 0,0)
6. `marengo-pi` with stdin: `enable`, `gravity-on` — **not** `motor-repl gravity-on` alone

```bash
MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual ./target/release/marengo-pi
# stdin: home / enable bench / gravity-on / status / disable / quit
```

### Phase 5b — Weighted gravity sign test

After bare-motor sign checks, run the asymmetric-load procedure in [bench-weighted-gravity-sign.md](bench-weighted-gravity-sign.md) (`shoulder_pitch_weighted` profile, `weighted_single_arm` harness).

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
| `i2cdetect -y 0` fails | Harmless on Pi 5 — use bus **1** for header I2C |
| No device at `0x4a` | This board uses **`0x4b`** when ADR/SA0 is high |
| `Permission denied` on `/dev/i2c-1` | Add user to `i2c` group (`sudo usermod -aG i2c $USER`), re-login; or re-run `install-pi.sh` |
| `config.txt` edit denied | Use `sudo nano /boot/firmware/config.txt` or `sudo tee` |
| Python `externally-managed-environment` | Use venv (`python3 -m venv`), not `sudo pip3` |
| `ModuleNotFoundError: lgpio` | `python3 -m venv --system-site-packages` + `apt install python3-lgpio` |
| IMU probe timeout / no quaternions | BNO085 uses **SHTP**, not MPU6050-style register reads; confirm `i2cdetect -y 1` shows `0x4b` and wiring is 3.3 V |
| `product id response` timeout | Chip wedged — **stop all `marengo-pi`** (only one I2C client), **power-cycle BNO085** (3.3 V off 10 s), retry `imu-probe` before starting `marengo-pi` |
| `Remote I/O error (121)` at `0x4a` | Wrong address — this bench uses **`0x4b`** (ADR high) |

## Consul (robot-hosted UI)

1. Build/install includes `marengo-gateway` and Consul static assets (`deploy-pi.sh` builds `consul/dist` → `www/`; `install-pi.sh` installs to `/opt/marengo/www`).
2. After `install-pi.sh`, **`marengo-gateway`** and **`marengo-can`** are enabled on boot. Gateway listens on **`[::]:8080`** (HTTP API), **`[::]:8444`** (HTTPS Consul UI), **`[::]:8443`** (WebTransport/QUIC).
3. On the bench LAN, open **`https://marengo.local:8444`** and accept the self-signed certificate once.
4. Run `marengo-pi` manually for live telemetry (`MARENGO_CHAPPE_SOCKET=/run/marengo/chappe.sock` in `/etc/marengo/env`). **`marengo-pi.service` stays disabled** by default — no automatic motor control after reboot.

**Boot state after power loss:** CAN up, gateway/UI up, `marengo-pi` stopped, motors not enabled.

**Local dev (optional):** copy `consul/.env.example` → `consul/.env.local`, then `cd consul && npm run dev` on the dev Mac.

WebTransport uses **QUIC (UDP)**. `ssh -L` forwards TCP only and will not carry `:8443`; use LAN hostnames instead.

Local demo without hardware:

```bash
cd consul && npm run build
cargo run -p marengo-gateway -- --demo \
  --https-listen 127.0.0.1:8444 --web-root consul/dist
```

Then open `https://127.0.0.1:8444`.

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
