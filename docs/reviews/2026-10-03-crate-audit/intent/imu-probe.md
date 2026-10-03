# Intent card — `imu-probe`

## 1. Header

| Field | Value |
|---|---|
| Crate | `imu-probe` |
| Path | `bins/imu-probe` |
| Kind | bin; feature `linux-i2c` (→ `marengo-imu/linux-i2c`), `Cargo.toml:13-15` |
| Baseline | `a2b55b3` |
| LOC | src 174 (`main.rs`); tests 0 (`metrics/loc.md:16`) |
| Sources | `bins/imu-probe/codemap.md`, `src/codemap.md`, `bins/AGENTS.md`, `scripts/deploy-pi.sh:192-216`, `scripts/install-pi.sh:158,181`, `scripts/check.sh:184-186`, `scripts/pi-native-build.sh:26-27`, `tools/marengo-pi-mcp/src/tools/readonly.ts:49-50,175-190`, ADR 0014 §2 (IMU stays on Pi), prior `control.md` CS17/CS18, ledger, `git log` (3 commits; created `82b319d` 2026-05-26 "feat(imu): add BNO085 I2C driver and imu-probe") |

## 2. Intent

A read-only **hardware check** for the torso BNO085: open the Pi I2C bus, initialise SHTP, enable the rotation-vector report, print N quaternion samples and fail if none arrive — without touching motors or Chappe (`main.rs:1`; `bins/imu-probe/codemap.md` "Read-only BNO085 IMU hardware check … without enabling motors"). It exists so bring-up can verify wiring/address/i2c group independently of `marengo-pi`'s background IMU publisher (`marengo-pi/src/imu.rs`), and is invoked by MCP `pi_imu_probe` (`readonly.ts:175-190`). ADR 0014 §2 confirms the IMU stays on the Pi I2C. No conflicting statements found.

## 3. Owns / Must not

| Owns | Must not |
|---|---|
| CLI parsing and a bounded sampling loop (`main.rs:31-96,133-150`) | Open CAN or motors (none; deps are `marengo-imu`, `marengo-support`, `tracing`, `Cargo.toml:21-24`) — upheld |
| Non-Linux / no-feature build returns an explicit error (`main.rs:166-173`) | Implement SHTP/BNO085 protocol (lives in `marengo-imu`) — upheld |

## 4. Interface

| Surface | Detail | Consumers |
|---|---|---|
| CLI | `--bus PATH` (default `/dev/i2c-1`), `--address HEX` (default `0x4b`), `--samples N` (default 10, >0), `--report-interval-us US` (default 20 000), `--timeout SEC` (default 5), `-h` (`main.rs:10-12,23-96`; defaults from `marengo-imu/src/lib.rs:35,38`) | MCP `pi_imu_probe` (`readonly.ts:188`), `pi_health` existence check (`readonly.ts:49-50`) |
| Stdout | `sample=<n> i= j= k= real= accuracy=` (`main.rs:138-146`) | humans / MCP output |
| Exit | 2 bad args, 1 failure/zero samples (`main.rs:101-113,152-156`) | MCP |
Depth: thin adapter over `marengo_imu::Bno085<LinuxI2cBus>`; no seam. Coverage 0.0 % (0/91 lines, `metrics/coverage-by-file.md:7`). Built and deployed with `--features socketcan,linux-i2c` (`deploy-pi.sh:193`).

## 5. Invariants owned

| Invariant | Code | Test |
|---|---|---|
| Fails non-zero when no sample arrives before timeout | `main.rs:135,152-156` | **untested** |
| `--samples 0` rejected | `main.rs:85-87` | **untested** |

## 6. Inputs / outputs

I2C device file (`--bus`), `RUST_LOG` (`init_tracing`, `main.rs:99`), stdout/stderr. No config, env (beyond logging), Chappe, CAN or files written.

## 7. Prior review reconciliation

| ID | Prior | Current | Evidence |
|---|---|---|---|
| CS17 | driver returns cached rotation with no new packet | **open**, and it invalidates this probe's sample count (L1) | `marengo-imu/src/driver.rs:84-87` returns `self.last_rotation`; ledger open |
| CS18 | 14-byte rotation report split as 12 | **open** (driver) | ledger open |

## 8. Drift

| Doc | Says | Code |
|---|---|---|
| `bins/imu-probe/codemap.md` | CLI args `--bus`, `--address`, `--samples` | also `--report-interval-us`, `--timeout` (`main.rs:61-76`) |
| `bins/AGENTS.md` | Host "Pi" | consistent |

## 9. Prune candidates

None with evidence. Overlap with `marengo-pi/src/imu.rs` (both drive `Bno085::initialize` + `enable_rotation_vector`) is two small callers of one driver, not a duplicate implementation.

## 10. Phase-B leads

| # | Location | Suspicion |
|---|---|---|
| L1 | `main.rs:135-150` + `marengo-imu/src/driver.rs:84-87` | `poll()` returns the cached last sample every call; with a 10 ms loop sleep, one real packet yields 10 identical "samples" in ~100 ms and exit 0. The probe can report success for a sensor that delivered a single report and then went silent (CS17). |
| L2 | `main.rs:136` | Any transient `poll` error aborts the whole probe via `?` (strict, opposite of marengo-pi which only warns); acceptable for a probe but differs from runtime behaviour. |
| L3 | `main.rs:50` | `--address` parsed as hex after stripping `0x`; decimal input such as `75` silently means 0x75. |
| L4 | `main.rs:149` | Fixed 10 ms sleep independent of `--report-interval-us`; with short intervals, reports may be dropped/batched (interacts with CS18 batch-splitting bug). |
