---
name: can-timeline
description: Reconstruct what happened on Marengo's CAN wire - fetch the candump, decode Robstride frames, align them with host log events, and measure replies, silences, overruns and bus load around the event. Use for any communication-level symptom - rx_over_errors growth, kernel error frames, Transport/DriveState/Communication latches, grant revocations, a drive going silent or rebooting, enable or reference timing failures, drive-side stops - or whenever the logs and the arm's behaviour disagree.
---

# CAN timeline

Logs record what the host meant to do; the candump records what reached the wire and what came back. Build one timeline from both and measure it.

## 1. Get both sides

- **Wire.** An enable soak copies `var/enable-soak/<TS>/candump.log`. Other sessions name `/opt/marengo/var/log/candump-<TS>.log` in the session log's final JSON line. Copy it read-only into the gitignored capture folder:
  ```bash
  scp -o BatchMode=yes \
    "${MARENGO_PI_USER:-joey}@${MARENGO_PI_HOST:-joey-robot.tail0b414.ts.net}:/opt/marengo/var/log/candump-<TS>.log" \
    var/firmware-captures/
  ```
  Quick looks: `pi_candump_summary` (latest session), `pi_candump_once` (2 s live).
- **Host.** `bench-session.txt` / `bench-<TS>.log` for the same session.

## 2. Read frames

A line reads `(000.006519)  can0  180001FD   [8]  80 49 …`: seconds since capture start (`candump -t z`), interface, 29-bit ID, data. Host id is `0xFD`. Device ids map to joints in `config/motors.yaml`: can0 1 = pitch, 2 = roll, 3 = upper-arm yaw, 4 = elbow, 5 = lower-arm yaw.

| Type (`id>>24`) | Host → drive | Drive → host |
|---|---|---|
| 0 | identity request | identity reply (`… <<8 \| 0xFE`) |
| 1 | MIT command; bits 8–23 carry the torque field, not the host id | – |
| 2 | – | status reply |
| 3 / 4 / 6 | Enable / Disable (stop) / SetZero | – |
| 17 / 18 | parameter read / write | parameter read reply |
| 21 | – | fault report |
| 24 | reporting On/Off | periodic report, about every 10 ms, lowest priority |

- Host frames other than type 1: `type<<24 | 0xFD<<8 | device`, so `0300FD02` = Enable to drive 2. Its timestamp is the SocketCAN echo, i.e. when it reached the wire.
- Drive status (types 2 and 24): `type<<24 | status<<16 | device<<8 | 0xFD`. Mode is `(id>>22)&3` (0 Reset, 1 Calibration, 2 Run) and fault flags are `(id>>16)&0x3F`. So `020001FD` = drive 1, Reset, no flags; `028001FD` = drive 1, Run.
- Kernel error frames are `2000xxxx#…`, i.e. controller events such as RX overflow. Captures record them since `84da1838`.
- On the bench, every host frame of types 1, 3, 4, 6, 18 and 24 solicits exactly one type-2 reply.

## 3. Measure with the tools

Build natively on the Mac:

```bash
cargo run --release -p marengo-log-cli -- candump summary --file <f> --timestamp delta \
  --enrich --config-dir <session>/config        # counts, Hz, top IDs, joints
cargo run --release -p marengo-log-cli -- candump page --file <f> --timestamp delta \
  --enrich --config-dir <session>/config --offset <n> --limit 200   # decoded frames
cargo run --release -p marengo-log-cli -- firmware-timing [--json] <f>
```

`firmware-timing` reports:

- Enable→Run latency, Reset after Enable;
- post-SetZero silence;
- identity, parameter-read and MIT reply latencies, plus `mit_unanswered`;
- report period, and report Off→last report;
- Disable→Reset;
- `non_neutral_mit`;
- `kernel_error_frames`;
- per-interface peak frames per 10 ms and gaps over 5 ms.

Exact definitions are in the module doc of `bins/marengo-log-cli/src/firmware_timing.rs`. Compare against the measured envelope in `docs/commissioning/firmware/robstride-firmware-behavior.md` (data in `robstride-timing-profile.json`). A value outside that envelope is new firmware behaviour, which the emulator (`crates/davout/tests/physical_firmware`) no longer covers.

## 4. Align host and wire

Host log lines carry UTC wall-clock time and are written after the action; the capture is relative to its own start. Pick an anchor frame that has a matching log line: the pre-session `disable can0:N sent` burst, a SetZero (`0600FD0N`) and its reference line, or the first Enable. Compute the offset, then check it against a second anchor. The difference between the two is your alignment uncertainty (typically a few ms); quote it with every timing claim that crosses sources.

## 5. Build the window

Around the event (±50 to 500 ms), list in wire order the host writes, replies, reports, error frames and gaps. For every host write: did a reply come, and how late? For every drive: its last frame before the silence and its first after. Check the known hazards:

- **2-frame RX FIFO:** more than two drive frames inside about 0.7 ms (a write's replies plus coincident type-24 reports). An error frame or a Δ`rx_over_errors` confirms an overrun. Distinguish cause A (burst or solicit coincidence) from cause B (a receive blackout right after Disabling a drive in Run); see `docs/commissioning/handoff-2026-10-04-liveness-hardening.md`.
- **Post-SetZero blackout:** silence of 45–61 ms starting 511–614 ms after a SetZero. Any Enable or type-24 write inside it is ignored by the drive.
- **Reset-mode report after an Enable echo:** a DriveState latch.
- **Type-24 streams running at startup** with no owner: Transport `WorkLimit`.
- **A gap of 30 ms or more in host writes to an Active drive:** the drive-side CanTimeout (600 ≈ 30 ms on the right arm) stops it.
- **Mode or flag bits changing** in status IDs, type-21 fault frames, and a drive restarting (reset traffic after silence).

Done when the event is explained frame by frame with timestamps, or the timeline shows exactly what remains unexplained.

## 6. Report and route

Give:

- a timeline table (time, source, frame or log line, meaning);
- the measurements;
- the alignment uncertainty;
- the conclusion.

New firmware behaviour belongs in `robstride-firmware-behavior.md` (then `cargo test -p davout --test firmware_profile`). Confirmed cause: add it to the `fault-signatures` catalog, then `bench-to-test` (FirmwareBus).
