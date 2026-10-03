# Intent card: `marengo-imu`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-imu` |
| Path | `crates/marengo-imu` |
| Kind | lib (feature `linux-i2c`) |
| Baseline | `a2b55b3` |
| LOC | src 916 (`driver.rs` 344, `shtp.rs` 231, `bus.rs` 157, `i2c_linux.rs` 69, `types.rs` 58, `lib.rs` 38, `error.rs` 19); no `tests/` dir; 12 inline unit tests (`metrics/test-counts.md`) |
| Sources consulted | `src/lib.rs` //! docs; `codemap.md`, `src/codemap.md`; crates/AGENTS.md:20,52; crates/codemap.md:22; docs/rust-patterns.md §11a (lines 377-389); bins/AGENTS.md:14; control.md CS17/CS18; repository-review.md:83; ledger CS17/CS18; `git log -- crates/marengo-imu` incl. `git show 6439835`; consumers `bins/marengo-pi/src/imu.rs`, `bins/imu-probe/src/main.rs`; `scripts/env.example:28-31`; `metrics/*` |

## 2. Intent

The crate is the **BNO085 (SH-2 over SHTP) I2C driver**. It initializes the sensor (soft reset, product-ID check), enables the rotation-vector feature, and parses sensor reports into a normalized quaternion with an accuracy class (`lib.rs:1-15`; crates/AGENTS.md:20; commit `82b319d`). It isolates the bus behind a 3-call `I2cBus` trait. That trait encodes the Adafruit-style "peek 4-byte header, then read the full packet" pattern, found during Pi bring-up (`bus.rs:13-22`; docs/rust-patterns.md:377-389; commits `7f6beba`, `4743395`).

Its consumers:
- Torso-orientation telemetry: `marengo-pi` publishes `sensors/imu/torso` when `MARENGO_IMU_BUS` is set (`bins/marengo-pi/src/imu.rs:40-65,121-139`).
- The `imu-probe` bench check (`bins/imu-probe/src/main.rs:116-160`).

The doc states the crate does not publish Chappe topics and does not perform estimation or gravity compensation (`lib.rs:12-15`). The prior review confirms orientation is not used by the gravity model (repository-review.md:83). The crate is therefore currently **telemetry-grade, not control-grade**. control.md CS17 notes it "would become a control safety concern if torso orientation is used later".

Conflicting statements of intent:
- `codemap.md:4` says "Publishes orientation data" and crates/codemap.md:22 "IMU driver and **frame publishing**". `lib.rs:13` says it does **not** publish. The code agrees with `lib.rs`: no Chappe dependency (`Cargo.toml:17-22`).
- `lib.rs:10` says it parses "(and optional accel/gyro)" reports. Accel and gyro are parsed into private fields that nothing reads (§9). `types.rs:49` `ImuSample` says "for future Chappe/proto wiring", but `marengo-pi` uses `armee_proto::ImuSample` instead (`bins/marengo-pi/src/imu.rs:9`).
- Report length: commit `6439835` set the rotation vector to 12 bytes, reasoning "firmware on this board omits the optional 2-byte timestamp". CS18 (control.md:181-189, citing the Adafruit driver) says 0x05 is 14 bytes, with the trailing 2 bytes being accuracy, not a timestamp. The same commit also made truncated tail reports non-fatal (`shtp.rs:108-111`), which may have been the real fix [INFERENCE]. This needs a captured hardware batch to settle (§10).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| SHTP header parse (15-bit length, 0/0x7FFF invalid), outgoing framing, per-channel sequence numbers | `shtp.rs:33-67`; `driver.rs:232-243` |
| SH-2 commands: soft reset ×2 with 1 s waits, product-ID request (5 s timeout, 3 attempts), Set Feature + Get Feature response tracking | `driver.rs:44-82,110-142,163-177` |
| Batch splitting by report ID, meta reports 0xFA/0xFB skipped, unknown IDs stop the batch, truncated tail dropped | `shtp.rs:81-117`; `driver.rs:179-187` |
| Rotation vector Q14 → normalized quaternion; accuracy from status bits 0-1 | `shtp.rs:123-135`; `types.rs:11-24,35-45`; `driver.rs:188-193` |
| Linux `/dev/i2c-*` backend mapping EREMOTEIO/ENODEV/EAGAIN text to "no packet" | `i2c_linux.rs:8-69` |
| Defaults: address 0x4B, 50 Hz | `lib.rs:33-37` |

| Must not | Evidence | Violation? |
|---|---|---|
| Control motors | `lib.rs:13` | None (deps: thiserror, tracing, optional i2cdev; `Cargo.toml:17-22`). |
| Publish Chappe topics | `lib.rs:13` | None. Docs contradict this (§8). |
| Gravity compensation / state estimation | `lib.rs:14` | None. |
| Be used in the control loop as-is | [INFERENCE from CS17 + this card] | Not violated today. Orientation is never consumed by Berthier/armee-dynamics (repository-review.md:83). |

## 4. Interface

| Concept | Public surface | Consumers |
|---|---|---|
| Driver | `Bno085::{new, initialize, enable_rotation_vector, enable_feature, poll, wait_rotation_vector, last_rotation}` (`driver.rs:31-108`) | marengo-pi `imu.rs:109-118` (`new`, `initialize`, `enable_rotation_vector`, `poll`); imu-probe `main.rs:128-136` (same set). `wait_rotation_vector`: **zero** refs (`metrics/pub-usage.md`). `enable_feature` and `last_rotation` used only internally/tests. |
| Bus seam | `I2cBus` trait, `BusError` (`bus.rs:3-22`) | implementors: `LinuxI2cBus` (`i2c_linux.rs:16-69`), `MockI2cBus` (`bus.rs:41-136`) |
| Linux backend | `LinuxI2cBus::open` (feature `linux-i2c` + Linux) | marengo-pi `imu.rs:11,109`; imu-probe `main.rs:119,128` |
| Test double | `MockI2cBus`, `MockTransaction`, `TransactionKind` (public in the production API, `lib.rs:26`) | crate tests only |
| Types | `Quaternion`, `ImuAccuracy`, `RotationVectorSample`, `ImuSample` | marengo-pi `imu.rs:31-36` (`ImuAccuracy`); `ImuSample` **zero** refs |
| Errors | `ImuError::{Bus, Protocol, Timeout, FeatureNotEnabled, NoSample, BackendUnavailable}` | `NoSample`, `BackendUnavailable` never constructed (grep of `crates/marengo-imu/src`) |
| Consts | `DEFAULT_I2C_ADDRESS`, `DEFAULT_REPORT_INTERVAL_US` | marengo-pi `imu.rs:48`; imu-probe `main.rs:7,26-35` |

Cargo features: `linux-i2c = ["dep:i2cdev"]` (`Cargo.toml:13-15`). Enabled in deploy builds (`scripts/deploy-pi.sh:193`; `scripts/pi-native-build.sh:27-28`, `--features socketcan,linux-i2c`) through `marengo-pi`/`imu-probe` features (`bins/marengo-pi/Cargo.toml:16`; `bins/imu-probe/Cargo.toml:15`).

Depth: `Bno085` is **medium-deep**. A small API hides the SHTP sequencing and report parsing, but `poll` leaks "latest cached sample" semantics (§10). `I2cBus` is a **real seam** with 2 adapters: Linux and Mock.

**Metrics baseline** (`metrics/`, `a2b55b3`, macOS):
- Crate coverage is **72.4 / 76.6 / 80.9 %** (`coverage-by-crate.md`).
- By file: `driver.rs` **61.9 %** (153/247), `bus.rs` 75.3 %, `shtp.rs` 86.9 %, `types.rs` 83.3 % (`coverage-by-file.md`).
- `i2c_linux.rs` is not compiled on macOS (feature + OS gated), so it is unmeasured.
- `bin:imu-probe` is 0.0 % (`coverage-by-crate.md`).
- Zero-use `pub`: `wait_rotation_vector`, `CHANNEL_SHTP_COMMAND` (`pub-usage.md`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Header with length 0 or 0x7FFF means no packet | `shtp.rs:41-46` | `shtp.rs:172` `header_parse_empty`, `:162` |
| Packet length ≤512 B | `driver.rs:215-220` | **untested** |
| Outgoing frame layout + sequence increments per channel | `shtp.rs:59-67`; `driver.rs:237-241` | `shtp.rs:177`; `driver.rs:330` |
| Feature enabled only after Get Feature Response for that ID | `driver.rs:73-81,170-175` | `driver.rs:314` |
| Rotation parsed only from channel 3; 0xFA/0xFB prefixes skipped | `driver.rs:179-187` | `driver.rs:266,291` |
| Quaternion normalized (when norm > ε) | `types.rs:11-24` | indirectly `driver.rs:266` (magnitude check) |
| Rotation report length correct (0x05 = 14 per CS18) | `shtp.rs:84` = **12** (contested) | `shtp.rs:202-211` `split_batch_two_reports` **pins 12** (CS18) |
| Only **new** samples reported by `poll` | **not enforced**: returns cached `last_rotation` (`driver.rs:84-87`) | **untested** (CS17) |
| Reset clears stale sample cache | **not enforced**: `soft_reset` clears features/sequence only (`driver.rs:110-121`) | **untested** |
| Device disappearance is an error, not "no packet" | **not enforced**: "No such device" maps to `NoPacket` (`i2c_linux.rs:8-13`) | **untested** (Linux-only) |

## 6. Inputs / outputs

- **Device:** `/dev/i2c-N` at 7-bit address (default 0x4B). Plain I2C write; header read 4 B; full packet read (`i2c_linux.rs:21-69`).
- **SHTP channels:** 1 (executable: reset), 2 (control: product ID, set feature), 3 (input sensor reports) (`shtp.rs:6-9`).
- **Reports:**
  - Parsed: 0x05 rotation vector, 0x01 accel, 0x02 gyro, 0xF8 product ID, 0xFC get-feature response, 0xFA/0xFB timestamps.
  - Length-known only: 0x14-0x16, 0xF1 (`shtp.rs:17-24,81-91`).
- **Env (read by consumers, not the crate):** `MARENGO_IMU_BUS`, `MARENGO_IMU_ADDRESS`, `MARENGO_IMU_REPORT_HZ`, `MARENGO_IMU_FRAME_ID` (`bins/marengo-pi/src/imu.rs:40-64`; `scripts/env.example:28-31`).
- **Downstream topic (consumer):** `sensors/imu/torso`, `armee_proto::ImuSample` with accel/gyro forced to 0 and `has_* = false` (`bins/marengo-pi/src/imu.rs:121-137`).
- **Blocking sleeps:** reset 2× 1 s (`driver.rs:115-117`), retry 500 ms (`:51`), poll loops 5-10 ms (`:79,99,137`).

## 7. Prior review reconciliation

| Prior ID | Status | Evidence |
|---|---|---|
| CS17 (P2) obsolete samples republished with fresh timestamps | **Open** (ledger: open) | `driver.rs:84-87` returns cached sample; `driver.rs:110-121` does not clear it; the marengo-pi publisher stamps wall-clock time per poll (`bins/marengo-pi/src/imu.rs:121-122`); poll errors only `warn!` without ending the session (`imu.rs:147`) |
| CS18 (P2) rotation vector split at 12 bytes | **Open** (ledger: open), and **contested** by commit `6439835` (hardware observation of 12-byte reports). Needs a captured raw batch. | `shtp.rs:84`; test pins 12 (`shtp.rs:202-211`); comment calls bytes 12-13 a timestamp (`shtp.rs:119-122,133`), CS18 says accuracy estimate |
| repository-review.md:83 "orientation not used by gravity model" | **Still true** | zero `marengo_imu` refs in `crates/` outside the crate |

## 8. Drift

| Doc | Code |
|---|---|
| `codemap.md:4` "Publishes orientation data"; crates/codemap.md:22 "frame publishing" | Does not publish (`lib.rs:13`); marengo-pi publishes. |
| `codemap.md:7` "`shtp.rs`: … BNO085 register access" | No register access. SHTP is packet-based; register reads are explicitly the anti-pattern (docs/rust-patterns.md:381-389). |
| `src/codemap.md:4` "I2C read loop"; `:7` "`shtp.rs`: primary module" | The read loop is in `driver.rs:144-230`; `shtp.rs` holds pure helpers. |
| `lib.rs:4-5` "(later) by `marengo-pi` sensor polling" | Already used by marengo-pi (`bins/marengo-pi/src/imu.rs`). |
| `shtp.rs:127` "report[1] is status" | SH-2 input reports are `[id, seq, status, delay, …]`. `report[1]` is the sequence and the code reads status from `report[2]` (`:128`) [INFERENCE from the Adafruit layout cited in control.md:189]. The comment is wrong; the code is consistent. |
| `shtp.rs:119-122` "optional u16 timestamp" | Per CS18 the trailing 2 bytes are rotation accuracy (Q12 rad). |
| `bus.rs:16` "`data_length == 0` means no packet" | Driver also treats invalid headers as no packet (`driver.rs:210-213`). |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting it touches |
|---|---|---|---|
| `types::ImuSample` | Zero references (marengo-pi uses `armee_proto::ImuSample`) | high | `types.rs:50-58`, `lib.rs:29` |
| `last_accel`, `last_gyro`, `parse_accel`, `parse_gyro`, `parse_vec3_report`, `Q_POINT_8/9_SCALAR` | Write-only state: no accessor; no consumer enables accel/gyro (`bins/marengo-pi/src/imu.rs:127-134` hard-codes zeros) | high for the unread fields; med for the parsers (future accel/gyro is a stated intent, `lib.rs:10`) | `driver.rs:26-27,38-39,194-199`; `shtp.rs:17-18,27-28,137-153` |
| `Bno085::wait_rotation_vector` | Zero references (`metrics/pub-usage.md`) | high | `driver.rs:89-104` |
| `ImuError::NoSample`, `ImuError::BackendUnavailable` | Never constructed | high | `error.rs:15-18` |
| `CHANNEL_SHTP_COMMAND` (+ `#[allow(dead_code)]`) | Zero references (`metrics/pub-usage.md`) | high | `shtp.rs:5-6` |
| `MockI2cBus`/`MockTransaction`/`TransactionKind` in the public API | Test double exported from production API | low (move under `#[cfg(test)]` or a `test-util` feature; keep for downstream bin tests if planned) | `lib.rs:26`, `bus.rs:24-136` |
| `split_batch_reports` `Result` return | Never returns `Err` since `6439835` (`shtp.rs:98-117`) | med | `driver.rs:252-254` |
| Test `split_batch_two_reports` asserting 12-byte rotation | Test pinning implementation detail (codifies CS18 length) | med (rewrite with a captured fixture, not delete) | `shtp.rs:202-211` |

## 10. Phase-B leads

1. **CS17 stale-sample republication** (`driver.rs:84-87`). Combined with fresh wall-clock timestamps (`bins/marengo-pi/src/imu.rs:121-122`), a silent or unplugged sensor looks live forever. `imu-probe` counts the same cached sample repeatedly as N distinct samples (`bins/imu-probe/src/main.rs:134-148`), so the bench probe can pass with one real report.
2. **Disconnect masked as idle.** `"No such device"` is classified as `NoPacket` (`i2c_linux.rs:11`), so a physically removed sensor produces neither an error nor a session restart (with lead 1, it stays "live"). Matching on error message text is also locale and i2cdev-version fragile (`i2c_linux.rs:8-13`, commit `031ceae`).
3. **CS18 length contradiction.** If firmware actually sends 14-byte 0x05 reports, the 12-byte stride misaligns the next report. The next ID byte becomes accuracy byte 0, so any report after a rotation vector in the same batch is dropped or misparsed. If it truly sends 12, CS18's fix would break the bench. Needs a raw SHTP capture (`scripts/pi-bno085-shtp-init.py`, docs/rust-patterns.md:389) before changing.
4. **Zero-norm quaternion passes as valid.** `normalize` leaves a 0-norm quaternion unnormalized (`types.rs:13-21`) and it is published with its accuracy class. A truncated or zero report yields (0,0,0,0).
5. **Unknown report ID aborts the rest of the batch** (`shtp.rs:103-107`). Any enabled-but-unlisted report (e.g. 0x08 game rotation, 0x03 linear accel) placed before a rotation vector hides the rotation sample.
6. **`wait_rotation_vector` returns the cached sample immediately** (`driver.rs:96`). Unused today (§9), but wrong if adopted.
7. **Sequence numbers are never checked inbound** (`driver.rs:204-230` ignores `header.sequence`), so dropped or duplicated packets go undetected. The continuation bit (0x8000) is masked and ignored (`shtp.rs:43`).
8. **Blocking init on the publisher thread.** Up to about 3 × (2 s reset + 5 s ID timeout + 0.5 s) ≈ 22 s per attempt (`driver.rs:44-59,110-142`). Shutdown is checked only between sessions (`bins/marengo-pi/src/imu.rs:81-105`), so `marengo-pi` shutdown can wait on it [INFERENCE: verify whether shutdown joins this thread].
9. **Coverage gap on the safety-relevant read path** (`metrics/coverage-by-file.md`): `driver.rs` 61.9 %; `i2c_linux.rs` unmeasured (not built on macOS); `imu-probe` 0 %. These are gaps, not prune signals.
10. **`report_hz` → interval integer division** (`bins/marengo-pi/src/imu.rs:50-54`). Values >1 MHz give `report_interval_us = 0`, a busy poll loop and a "0 µs" Set Feature. This is a consumer-side bound.
