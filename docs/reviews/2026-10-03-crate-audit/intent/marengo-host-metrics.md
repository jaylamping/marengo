# Intent card: `marengo-host-metrics`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-host-metrics` |
| Path | `crates/marengo-host-metrics` |
| Kind | lib (+ `build.rs`) |
| Baseline | `a2b55b3` |
| LOC | src 1,354 (`lib.rs` 517 of which `mod linux` is lines 102-499, `diagnostics.rs` 421, `cpu.rs` 247, `sample_state.rs` 169), `build.rs` 15; no `tests/` dir (15 inline unit tests, `metrics/test-counts.md`) |
| Sources consulted | `src/lib.rs` //! doc (1 line); `codemap.md`, `src/codemap.md`; crates/AGENTS.md:22; crates/codemap.md:23; `Cargo.toml` description; gateway.md G18/G19 + design concern 8; ledger G18/G19; `git log -- crates/marengo-host-metrics`; consumer `bins/marengo-pi/src/host_metrics.rs`; Consul `components/dashboard/cards/{pi,jetson}-host-card.tsx`; `metrics/*`; `cargo check -p marengo-host-metrics` on macOS (4 dead-code warnings) |

## 2. Intent

The crate turns one host sample into the protobuf `HostMetrics` that operators see on the Consul Pi/Jetson host cards (`lib.rs:1`; `Cargo.toml:4` "Linux host metrics collector for Chappe HostMetrics protobuf"; commit `04a36cc`). A sample covers CPU, memory, load, thermal, disk/mount, network/CAN state, systemd service state, clock sync, Pi throttling, log-disk usage, build/deploy identity, and Chappe transport health supplied by the caller. It is a **Linux-only reader**:
- The collector lives in `#[cfg(target_os = "linux")] mod linux` (`lib.rs:102-499`).
- Non-Linux targets get a stub with hostname `"dev-host"` (`lib.rs:63-93`).

The 2026-10-01 repairs (G18, G19; commits `7f19fc9`…`8446666`) changed its stance to **"unknown, not healthy zero"**:
- CPU rates are `sample_valid` only with consistent per-ID baselines (`cpu.rs:39-53`).
- CAN state is `UNKNOWN` when unreadable (`diagnostics.rs:29-34`).
- Disk read-only and capacity carry `*_known` flags (`diagnostics.rs:165-179`).

It owns no socket and no policy. The caller (`marengo-pi`) supplies transport facts as primitives (`sample_state.rs:54`) and publishes the result.

Conflicting statements of intent:
- `codemap.md:4` says "CPU, memory, disk … for Consul health dashboard", and `:8` "Published via Chappe **or gateway HTTP**". The crate publishes nothing. Only `marengo-pi` uses it (`bins/marengo-pi/Cargo.toml:33`; `host_metrics.rs:11-31`). The gateway does not depend on it (§8).
- Pi and Jetson roles are both modeled (`lib.rs:16-22,159-172,405`), but only the Pi role has a producer. `bins/marengo-jetson` is a 6-line scaffold.

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| `/proc/stat` CPU parsing with keyed per-core baselines, reset/hotplug invalidation | `cpu.rs:11-81` |
| Mount table (`/proc/self/mountinfo`) + `df -B1` capacity, read-only from mount flags, escaped paths | `diagnostics.rs:127-261` |
| CAN state from `ip -details link show` | `diagnostics.rs:3-34`; `lib.rs:382-390` |
| Memory, load, thermal, network rates, services (`systemctl show`), clock (`timedatectl`), Pi throttle (`vcgencmd`), Jetson nvpmodel | `lib.rs:245-492` |
| Build identity: git SHA at compile time (`build.rs`), `.deploy-rev` at runtime | `build.rs:1-15`; `lib.rs:24-42` |
| Host→proto conversion of supplied Chappe/IPC health | `sample_state.rs:44-99` |
| Topic name per role | `lib.rs:13-22` |

| Must not (derived; no explicit does-not list in crate doc) | Evidence | Violation? |
|---|---|---|
| Own sockets / Chappe publishing | `sample_state.rs:54` "no socket ownership" | None. Publishing is in `bins/marengo-pi/src/host_metrics.rs:25-31`. |
| Perform control/safety decisions | crates/AGENTS.md:22 | None. Output is telemetry only. |
| Own log-retention policy | ADR 0011 (store owns retention) | **Overlap.** Re-implements log disk usage and hardcodes the 5 GiB budget (`lib.rs:91,199-208`) instead of reading the store's setting `log_disk_budget_bytes` or constant `DEFAULT_LOG_DISK_BUDGET_BYTES` (`marengo-store/src/paths.rs:31`). |
| Own deploy revision parsing | marengo-deploy owns `.deploy-rev` (`marengo-deploy/src/lib.rs:4-6`) | **Duplicate.** `read_deploy_rev` (`lib.rs:36-42`) returns the whole trimmed line (`"<sha> <iso>"`); `marengo_deploy::parse_deploy_rev` splits SHA and timestamp (`marengo-deploy/src/rev.rs:11-34`). |

## 4. Interface

| Concept | Public surface | Consumers |
|---|---|---|
| Sampling | `sample(role, semver, &mut SampleState, ChappeHealthInput) -> HostMetrics` (`lib.rs:45`) | `bins/marengo-pi/src/host_metrics.rs:24` (1 Hz dedicated thread, `:18-38`) |
| State | `SampleState` (`sample_state.rs:37`; only `sample_at` is pub) | same |
| Transport health inputs | `ChappeHealthInput`, `IpcQueueHealthInput`, `ChappeHealthInput::into_proto` | `host_metrics.rs:41-67` |
| Topic | `host_metrics_topic(role)` | `host_metrics.rs:26` |
| Build identity | `git_sha()`, `build_info(semver)` | internal only (`lib.rs:79,183`); zero external refs |

Depth: `sample` is **deep**: one call with many sources behind it. Seams:
- `diagnostics::Sources` trait (`diagnostics.rs:128-131`) has **2 adapters**: `SystemSources` (`lib.rs:119-134`) and test `Fixture`s (`diagnostics.rs:269,307`). A real seam, but it covers only disks and CAN.
- CPU uses a pure-text seam (`sample_cpu_from_stat(&str, …)`, `cpu.rs:11`), which is also real.
- Memory, load, thermal, network, services, clock and throttle read `/proc`, `/sys` and subprocesses directly, so they have **no seam** and are untestable without a Linux host.

Cargo features: none. Platform split is by `cfg(target_os = "linux")` and `cfg(any(target_os="linux", test))` (`lib.rs:3-6,52-60,102`).

**Metrics baseline** (`metrics/`, `a2b55b3`, measured on aarch64-apple-darwin):
- Crate coverage is **97.6 / 96.5 / 97.1 %** (`coverage-by-crate.md`). By file: `cpu.rs` 98.5 %, `diagnostics.rs` 98.2 %, `sample_state.rs` 98.9 %, `lib.rs` 91.1 % on only **79** instrumented lines (`coverage-by-file.md`). The headline is misleading. `mod linux` (`lib.rs:102-499`, about 400 lines and the entire production collector) is not compiled on macOS, so it is **unmeasured**. The figure covers the pure parsers and the non-Linux stub.
- 15 tests (`test-counts.md`).
- `cargo machete` flags `thiserror` as unused (`unused-deps.md`; confirmed: no error type in the crate).
- On macOS non-test builds `SampleState` internals are dead. `cargo check` warns `counters`, `rates`, `rx_bytes/tx_bytes` and `cpu_aggregate/cpu_per_core/network` are never read (`sample_state.rs:6,10,31,39`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| CPU columns follow kernel order (user nice system idle iowait irq softirq steal); guest not double-counted; irq counted busy | `cpu.rs:72-81`; `sample_state.rs:10-26` | `cpu.rs:90` `published_cpu_fields_follow_kernel_counter_columns`; `cpu.rs:228` |
| Per-core baselines keyed by CPU id; hotplug/reorder/reset/overflow/duplicate → unknown, not 0 % | `cpu.rs:15-68` | `cpu.rs:139,175,200` |
| CAN state is one of 6 kernel states else `UNKNOWN`; command failure → `UNKNOWN`; `restart-ms` not mistaken for state | `diagnostics.rs:3-34` | `diagnostics.rs:59,67,83` |
| Disk read-only comes from mount and superblock flags; ambiguous → unknown; capacity unknown on df failure, `total==0` or `used>total` | `diagnostics.rs:133-184,209-261` | `diagnostics.rs:284,333,357,374,394` |
| IPC queue health survives the wire, including disconnected | `sample_state.rs:71-99` | `sample_state.rs:108` |
| Thermal/memory/load/uptime unknown ≠ 0 | **not enforced**: missing values become `0` (`lib.rs:241-242,266-279,291-293,305-307`) | **untested** (Linux-only code, no seam) |
| Service state honest (activating/reloading/failed) | `lib.rs:418-422` collapses all non-`active`/`failed` to `Inactive`; ignores `systemctl` exit status (`:411-415`) | **untested** |
| Network rate non-negative across counter reset | `saturating_sub` (`lib.rs:348-349`) | **untested** |

## 6. Inputs / outputs

- **Files read:** `/proc/{stat,meminfo,loadavg,uptime,net/dev,self/mountinfo,sys/kernel/hostname,sys/kernel/osrelease,device-tree/model}`, `/etc/os-release`, `/sys/class/thermal/*/{type,temp}`, `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq`, `/sys/class/net/<if>/{operstate,statistics/tx_errors,statistics/rx_errors}`, `$MARENGO_ROOT/.deploy-rev`, `$MARENGO_ROOT/var/log/**`, `$MARENGO_ROOT/var/marengo.db` (`lib.rs:36-42,165-499`).
- **Subprocesses per sample:** `df -B1 /` and `df -B1 /boot/firmware` (`lib.rs:322-324`); `ip -details link show <canX>` per CAN interface; `systemctl show` ×3 (Pi); `timedatectl`; `vcgencmd get_throttled` (Pi); `nvpmodel -q` (Jetson).
- **Env:** `MARENGO_ROOT` (`lib.rs:37,200`). Build-time: `git rev-parse --short HEAD` → `MARENGO_GIT_SHA` (`build.rs:2-14`).
- **Output:** `armee_proto::HostMetrics`. Topic `host/metrics/pi` or `host/metrics/jetson` (`lib.rs:13-14`). The caller publishes it at 1 Hz (`bins/marengo-pi/src/host_metrics.rs:1,33-36`).

## 7. Prior review reconciliation

| Prior ID | Status | Evidence |
|---|---|---|
| G18 CPU/iowait wrong columns | **Fixed** (ledger: verified PR237) | Label parsed separately (`cpu.rs:17-31`), iowait = column 4 (`sample_state.rs:21-24`); tests `cpu.rs:90,228` |
| G19 CAN state parses restart-ms; read-only from df | **Fixed** (ledger: verified PR238) | `diagnostics.rs:3-34,193-227`; tests `diagnostics.rs:59,284` |
| gateway.md design concern 8: budget hardcoded 5 GiB; WAL/SHM omitted; unknown reported as healthy zeros | **Open** | `lib.rs:91,201` hardcoded budget; `lib.rs:204-206` counts only `marengo.db`, not `-wal`/`-shm`; zeros on read failure (`lib.rs:241-307`) |
| finding-index.md G18/G19 rows say "Open" | Drift vs ledger (verified) | finding-index.md:67-68 |

## 8. Drift

| Doc | Code |
|---|---|
| `codemap.md:8` "Published via Chappe or gateway HTTP"; `:11` "Consumed by … gateway state API" | Gateway has no dependency on this crate (`bins/marengo-gateway/Cargo.toml`; zero `marengo_host_metrics` refs in gateway). It only relays the proto. |
| `codemap.md:7` "Platform-specific readers (Linux `/proc`, etc.)" | Only Linux; other platforms get a stub (`lib.rs:63-93`). |
| `src/codemap.md:4` "Metric sampling and serialization for wire export" | No serialization; returns a prost struct. |
| `lib.rs:44` "on non-Linux returns a minimal stub **for tests**" | Under `cfg(test)` on Linux the real collector runs. The stub is what a macOS/Windows dev `marengo-pi` publishes as live data with `hostname: "dev-host"` (`lib.rs:74`) and a fabricated 5 GiB budget (`:91`). |
| crates/AGENTS.md:22 "Host-level metrics (CPU, temp, etc.)" | Accurate but omits CAN/service/deploy identity scope. |
| Proto field `pmic_temp_celsius` | Filled with the CPU zone temperature (`lib.rs:456`), not the PMIC. |
| Proto field `throttle_events` | Raw `get_throttled` bitmask, not an event count (`lib.rs:455,475-476`). |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting it touches |
|---|---|---|---|
| `thiserror` dependency | Unused dependency (`metrics/unused-deps.md`; no error type in src) | high | `Cargo.toml:15` |
| `git_sha()` / `build_info()` as `pub` | Zero external refs (internal use only) | med (`pub(crate)`) | `lib.rs:24-34` |
| `read_deploy_rev` (`lib.rs:36-42`) | Duplicate implementation of `marengo_deploy::read_deploy_rev` / `parse_deploy_rev` (and less correct: keeps the timestamp) | med (needs a dep on marengo-deploy or moving rev parsing to a shared crate) | `lib.rs:30,36-42` |
| `sample_log_disk` + `dir_size` (`lib.rs:199-224`) and the literal 5 GiB (`lib.rs:91,201`) | Duplicate implementation of `marengo_store::Store::log_disk_usage_bytes` / `dir_size` (`marengo-store/src/store.rs:655-662,865-878`) and `DEFAULT_LOG_DISK_BUDGET_BYTES` | med | `lib.rs:158,194-195` |
| `CpuMetrics.per_core_usage_percent` population (`cpu.rs:59`) | Superseded by `cores[].usage_percent` (Option, commit `c349b3f`). Legacy field fabricates `0.0` for unknown. Consul reads neither (only `cpu.sampleValid`/`usagePercent`, `consul/src/components/dashboard/cards/pi-host-card.tsx:39`). | low (proto field; check other readers before removal) | `cpu.rs:59`, proto |
| `freq_mhz`, `pmic_temp_celsius`, `ChappeHealth.gateway_reachable` (+ per-second TCP probe in marengo-pi `host_metrics.rs:70-80`) | Scaffold with no consumer: no Consul reference found (grep of `consul/src` excluding `gen/`) | low (proto change; gateway JSON or other clients may read them) | `lib.rs:248-252,456`; proto |
| `ChappeHealthInput.gateway_rtt_ms` for the Pi | Always `0.0` from the only producer (`bins/marengo-pi/src/host_metrics.rs:52`), consumed only on the Jetson branch (`lib.rs:168`) | low | `sample_state.rs:50` |
| Jetson role branch (`lib.rs:163-171,405`, `read_nvpmodel` `:479-492`, `TOPIC_HOST_METRICS_JETSON`) | Scaffold with no consumer: no Jetson producer exists. Consul has a Jetson card. | low (roadmap keeps Jetson; do not prune without a decision) | `lib.rs` |

Not prunable: the CAN-state and mount-flag "unknown" paths. They are diagnostics honesty fixes (G19).

## 10. Phase-B leads

1. **The collector is unmeasured and mostly unseamed.** About 400 production lines (`lib.rs:102-499`) are compiled only on Linux. Coverage was measured on macOS (`metrics/README.md` tool table: aarch64-apple-darwin). Memory, thermal, network, services, clock and throttle have no test or seam. Confirm whether the Linux container CI runs the `#[cfg(test)]` Linux path (`lib.rs:507` is the only test there, and it asserts only a non-empty hostname).
2. **Healthy-zero fabrication.** Missing thermal `temp` gives 0 °C. `cpu_c` takes the first zone if no "cpu" zone exists (`lib.rs:305-311`). Missing meminfo gives 0 bytes (`:266-279`). `systemctl` failures give Inactive. `vcgencmd` failure gives `throttled_now=false` (`:454`). These undo G19's "unknown ≠ healthy" stance outside disk and CAN.
3. **Unmounted `/boot/firmware` reports root's filesystem.** The mount filter always admits `/` (`diagnostics.rs:150-154`), so an absent `/boot/firmware` gets `/`'s filesystem, source and read-only flag. `df -B1 /boot/firmware` then also reports the root fs. The result is mislabeled, not unknown.
4. **Throttle semantics.** `throttled_now = bit 2` (`lib.rs:476`) ignores under-voltage-now (bit 0) and soft-temp-limit (bit 3). `throttle_events` is the raw mask including sticky bits 16-19 [INFERENCE from vcgencmd docs; verify against firmware docs].
5. **Stale build SHA.** `build.rs` emits no `cargo:rerun-if-changed`, so Cargo reruns it only when files in this package change. `MARENGO_GIT_SHA` can lag `HEAD` in incremental builds (`build.rs:1-15`). `.deploy-rev` is read separately (`lib.rs:36-42`), so the card can show two disagreeing revisions.
6. **Subprocess fan-out every second.** Each sample spawns 2× `df`, 1× `ip` per CAN interface, 3× `systemctl`, `timedatectl` and `vcgencmd` (`lib.rs:322-476`). None has a timeout. A hung `systemctl`/`vcgencmd` stalls the publisher thread indefinitely, and with it the IPC queue-health telemetry (`bins/marengo-pi/src/host_metrics.rs:19-37`). There is no control-loop impact (separate thread).
7. **Recursive `dir_size` follows symlinks** (`lib.rs:210-224`, `is_dir()` follows). A loop under `var/log` recurses unboundedly [INFERENCE]. The walk itself is O(files) every second.
8. **Network counter wrap/reset.** `saturating_sub` turns a counter reset into a 0 rate for one sample. The interface rename/remove map `prev.network` never shrinks (`lib.rs:346-357`). Growth is bounded by interface churn, which is small.
9. **`elapsed` defaults to 1.0 s** on the first sample and is clamped to ≥1 ms (`lib.rs:143-147`). A long stall (lead 6) averages rates over the stall without signaling it.
10. **Dev-host stub published as live data** (`lib.rs:63-93`). A non-Linux `marengo-pi` (sim/dev) sends `cpu: Some(default)`, where `sample_valid=false` is honest. It also sends `log_disk_budget_bytes=5 GiB` and `clock.synchronized=false`, which Consul may render as real.
