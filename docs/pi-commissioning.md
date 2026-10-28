# Pi commissioning runbook

Pi OS, CAN HAT, IMU, install and first CAN checks for the Marengo bench. The limb motion procedure (Reference → G-comp → Position ladder → sign-off) is the separate [limb playbook](commissioning/limb-playbook.md).

Current bench hardware (2026-10-28): Raspberry Pi 5, Waveshare 2-CH CAN HAT (MCP2515), BNO085 on I2C `0x4b`, and the right 5-DOF arm on `can0`, device ids 1–5 (firmware 0.3.1.42 on 1–2, 0.2.3.34 on 3–4, 0.0.3.32 on 5). `can1` is wired but unused. See also [safety.md](safety.md) and [can_topology.md](../hardware/electrical/wiring/can_topology.md).

## Phase 0 — Bench pre-flight

- [x] Joint names follow the master URDF (`right_shoulder_pitch` … `right_lower_arm_yaw`)
- [x] Firmware IDs: right arm **1–5** on can0 (an earlier left shoulder pitch used **12** on can1); set-zero after any ID change
- [ ] CAN termination verified (`0x3041 can_status` in Robstride tool if bus errors)
- [x] Physical NC E-stop in the **motor power** path
- [ ] E-stop sense on GPIO 17 (not wired; no software reads it yet)

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
dtoverlay=mcp2515-can0,oscillator=16000000,interrupt=23
dtoverlay=mcp2515-can1,oscillator=16000000,interrupt=25
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
git clone https://github.com/jaylamping/marengo.git ~/marengo
cd ~/marengo && git lfs pull
# Install Rust 1.88.0 + protoc (see docs/dev-setup.md native section)
cargo build -p motor-repl -p marengo-pi --features socketcan --release
sudo ./scripts/install-pi.sh
```

Cross-build from laptop: `./scripts/deploy-pi.sh --install joey@marengo.local`. `install-pi.sh` provisions `/opt/marengo`, group-writable config/calibration directories, and narrow passwordless sudo rules for future MCP config sync/install.

**Environment** (`/etc/marengo/env` — template in `scripts/env.example`):

```bash
MARENGO_ROOT=/opt/marengo
MARENGO_CONFIG_DIR=/opt/marengo/config
RUST_LOG=robstride=info,davout=info,berthier=info,motor_repl=info
```

## Phase 3 — Config

There is one master description: `config/{robot,motors,control,homing}.yaml` plus `assets/urdf/marengo.urdf` (installed under `/opt/marengo/`). Do not fork config per profile. To run a subset of joints, set `MARENGO_JOINT_SUBSET=a,b` for that process (unknown names fail closed) or use Consul commissioning scope. MCP bench profiles (`arm_attached`, `elbow_attached`, …) in `tools/marengo-pi-mcp/src/bench-profiles.ts` select subsets the same way.

The retired `config/bringup/*` trees (dual shoulder pitch, left/right 3- and 4-DOF arms) are in git history; their URDFs are archived in `assets/urdf/archive/`. Tuning lives in `config/control.yaml`; never change gains, velocities, caps or limits without bench evidence. Limb motion commissioning SoT: [limb-playbook.md](commissioning/limb-playbook.md).

### Homing (interim — manual reference)

Hall-sensor homing is designed but not built ([hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md)). Until then:

1. Place the arm at mechanical reference (arm down for pitch).
2. Start one long-running `marengo-pi` process.
3. In that process, run `home <joints> sign-tested`; wait for each `reference <joint> current pos=` result, then run `home` and wait for Ready.
4. Run `enable` only after homing succeeds, in the same process.

Do not use `pi_hold_on` with `set_zero: true` unless intentionally recalibrating at mechanical zero.

Full homing contract: [homing.md](homing.md).

While a limb is only partly built, list its joints in the master config and narrow at runtime (`MARENGO_JOINT_SUBSET` or commissioning scope); Offline joints do not block Limb Ready.

## Phase 4 — CAN validation (no enable)

```bash
cd /opt/marengo
export MARENGO_CONFIG_DIR=/opt/marengo/config

/opt/marengo/bin/motor-repl status

# Single bus debug:
MARENGO_CAN_INTERFACE=can0 /opt/marengo/bin/motor-repl --can-interface can0 status
```

`status` opens SocketCAN but bypasses Davout Supervisor construction and type-24
active-reporting startup writes. `gravity-preview` reads the configured URDF
model locally and does not open CAN.

**Pass:** `status` reports the configured joint count and loop rate, and confirms
SocketCAN opened. It does not send a probe or active-reporting frame.

Raw bus (motor power on):

```bash
candump -ta can0 &
# Observe only. Do not hand-send Enable frames: Davout owns enable sequencing (type-24 Off + echo,
# post-SetZero quiet, paced Enables). Use `motor-repl protocol-inspect` for read-only drive queries.
```

## Phase 5 — Safe motor test ([safety.md](safety.md))

Each `motor-repl` command is a **separate process**, and a current reference lives only inside the process that acquired it ([ADR 0036](decisions/0036-physical-robstride-reference.md)). `motor-repl set-zero` grants end when that command exits. Do not use a separate Set Zero process as readiness for a later owner: acquire the reference and enable inside **one** `marengo-pi`. Without a live `marengo-pi`, MCP `pi_health` / `pi_homing_status` say `no live marengo-pi session` and list the latest reference journal rows (history only).

1. E-stop reachable; shoulder supported
2. Sign test per joint (small torque_ff; fix `direction` in YAML if inverted)
3. `gravity-preview` with no angles (zero pose) or a complete angle vector in `robot.yaml` joint order
4. `marengo-pi` with stdin, each joint at its mechanical reference: `home <joints> sign-tested` (wait for `reference <joint> current pos=` per joint), `home` (Ready), `enable bench`, `gravity-on` — **not** `motor-repl gravity-on` alone. Full procedure: [homing.md](homing.md#pi-bench-procedure).

```bash
MARENGO_MOTION_OWNER=stdin /opt/marengo/bin/marengo-pi
# stdin: home <joints> sign-tested / home / enable bench / gravity-on / status / disable / quit
```

### Phase 5b — Limb commissioning

After Pi bring-up and bare-motor sanity, run the limb motion SoT: [limb-playbook.md](commissioning/limb-playbook.md) (Sign → G-comp → … → Limb sign-off).

## Adding the next limb

1. Assign device ids from the block scheme below (and the full-body bus plan once decided; see [status.md](status.md#open-design-decisions)). Set ids in firmware before editing YAML.
2. Add the joints to `hardware/docs/kinematics.md`, the master URDF and `config/*.yaml` together, and to the limb's `robot.yaml` `limbs` entry.
3. Bring them up with `MARENGO_JOINT_SUBSET` or commissioning scope, then run the [limb playbook](commissioning/limb-playbook.md) for that limb.

## Device ID scheme (commissioned)

| Block | IDs |
|-------|-----|
| Right arm | 1–10 |
| Left arm | 11–20 |
| Right leg | 21–30 |
| Left leg | 31–40 |
| Waist + aux | 41+ |

Right arm (live): pitch **1**, roll **2**, upper-arm yaw **3**, elbow pitch **4**, lower-arm yaw **5**. Earlier left-arm slices used roll **11**, pitch **12**, yaw **13**, elbow **14**. The humanoid template `config/motors_humanoid.yaml` does not follow this scheme yet (open decision).

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
4. Run `marengo-pi` manually for live telemetry (`MARENGO_CHAPPE_SOCKET=/run/marengo/chappe.sock` in `/etc/marengo/env`). **`marengo-pi.service` stays disabled** by default — no automatic motor control after reboot. A manual run started from SSH and driven by typed stdin commands must claim motion with `marengo-pi --motion-owner stdin` (or `MARENGO_MOTION_OWNER=stdin`); the default owner is Consul (Chappe), and stdin then only accepts `disable`/`quit`/`status`. MCP sessions claim stdin automatically. See [safety.md](safety.md#single-motion-owner-stdin-vs-chappe).

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

Editor agents (Cursor, Claude Code) can drive the Pi over SSH via [`tools/marengo-pi-mcp/`](../tools/marengo-pi-mcp/README.md) (MCP server on your dev Mac).

- **Read-only / logs:** no confirmation — agent should call `pi_logs_*`, `pi_health`, `pi_motor_repl_status` freely.
- **Deploy:** `pi_sync_main` (pull `main`, cross-build, `install-pi.sh`) — pre-authorized.
- **Motion:** `confirm: true`; weighted profile also needs `confirm_weighted_motion: true` after two chat approvals.
- **Session logs:** harness and `pi_marengo_pi_script` tee to `$MARENGO_ROOT/var/log/bench-latest.log`.
- **Limb commissioning SoT:** [limb-playbook.md](commissioning/limb-playbook.md)

```bash
# Manual bench run with session log (same paths MCP uses)
export MARENGO_ROOT=/opt/marengo
mkdir -p "$MARENGO_ROOT/var/log"
./tools/marengo-pi-mcp/harness/log-tee.sh -- /opt/marengo/bin/motor-repl status
```
