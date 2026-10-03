//! Host metrics sampling for Chappe `HostMetrics` protobuf.

#[cfg(any(target_os = "linux", test))]
mod cpu;
#[cfg(any(target_os = "linux", test))]
mod diagnostics;
mod sample_state;

pub use sample_state::{ChappeHealthInput, IpcQueueHealthInput, SampleState};

use armee_proto::{BuildInfo, HostMetrics, HostNodeRole};

const TOPIC_HOST_METRICS_PI: &str = "host/metrics/pi";

/// Only the Pi publishes host metrics; the Jetson role has no producer (ADR 0014).
pub fn host_metrics_topic(_role: HostNodeRole) -> &'static str {
    TOPIC_HOST_METRICS_PI
}

fn git_sha() -> &'static str {
    env!("MARENGO_GIT_SHA")
}

fn build_info(semver: &str) -> BuildInfo {
    BuildInfo {
        deploy_rev: marengo_deploy::read_deploy_rev(&marengo_deploy::resolve_deploy_rev_path()).sha,
        git_sha: git_sha().to_string(),
        semver: semver.to_string(),
    }
}

/// Sample host metrics; on non-Linux returns a minimal stub for tests.
pub fn sample(
    role: HostNodeRole,
    semver: &str,
    prev: &mut SampleState,
    chappe: ChappeHealthInput,
) -> HostMetrics {
    let timestamp_ms = now_ms();
    #[cfg(target_os = "linux")]
    {
        linux::sample(role, semver, prev, chappe, timestamp_ms)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (role, semver, prev, chappe);
        stub_metrics(timestamp_ms, role, semver, chappe)
    }
}

#[cfg(not(target_os = "linux"))]
fn stub_metrics(
    timestamp_ms: u64,
    role: HostNodeRole,
    semver: &str,
    chappe: ChappeHealthInput,
) -> HostMetrics {
    use armee_proto::{ClockMetrics, CpuMetrics, LoadMetrics, MemoryMetrics, ThermalMetrics};

    HostMetrics {
        timestamp_ms,
        hostname: "dev-host".to_string(),
        node_role: role as i32,
        uptime_sec: 0,
        kernel_version: String::new(),
        os_pretty_name: String::new(),
        build: Some(build_info(semver)),
        cpu: Some(CpuMetrics::default()),
        memory: Some(MemoryMetrics::default()),
        load: Some(LoadMetrics::default()),
        thermal: Some(ThermalMetrics::default()),
        disks: vec![],
        network: vec![],
        services: vec![],
        chappe: Some(chappe.into_proto()),
        clock: Some(ClockMetrics::default()),
        platform: None,
        log_disk_bytes: 0,
        log_disk_budget_bytes: marengo_store::DEFAULT_LOG_DISK_BUDGET_BYTES,
        // Every section above is synthetic: this stub must never be mistaken
        // for live Pi data.
        simulated: true,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use armee_proto::{
        ClockMetrics, CpuMetrics, DiskMetrics, HostMetrics, HostNodeRole, LoadMetrics,
        MemoryMetrics, NetworkInterfaceMetrics, PiPlatformMetrics, ServiceState, ServiceStatus,
        ThermalMetrics, ThermalZone,
    };

    use super::build_info;
    use crate::cpu::sample_cpu_from_stat;
    use crate::{ChappeHealthInput, SampleState};

    /// A hung helper must fail one sample section, never stall the 1 Hz
    /// publisher and IPC health telemetry.
    const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
    /// Sample gaps beyond this invalidate per-second rates instead of
    /// silently averaging a stall into them.
    const MAX_SAMPLE_GAP_SECS: f64 = 5.0;

    /// Run a diagnostic helper with a timeout. `None` is an unavailable
    /// observation, never a healthy value.
    fn run_bounded(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
        use std::io::Read as _;
        use std::process::Stdio;
        let mut child = std::process::Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait().ok()? {
                Some(status) => {
                    if !status.success() {
                        return None;
                    }
                    let mut out = String::new();
                    child.stdout.take()?.read_to_string(&mut out).ok()?;
                    let _ = child.wait();
                    return Some(out);
                }
                None => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    struct SystemSources;
    impl crate::diagnostics::Sources for SystemSources {
        fn command(&self, program: &str, args: &[&str]) -> Option<String> {
            run_bounded(program, args, COMMAND_TIMEOUT)
        }
        fn read_file(&self, path: &str) -> Option<String> {
            fs::read_to_string(path).ok()
        }
    }

    /// B13 (D-1): the `HostNodeRole::Jetson` arm is deprecated (no producer) but
    /// kept so a Jetson role maps to no platform metrics, like Unspecified.
    #[allow(deprecated)]
    pub fn sample(
        role: HostNodeRole,
        semver: &str,
        prev: &mut SampleState,
        chappe: ChappeHealthInput,
        timestamp_ms: u64,
    ) -> HostMetrics {
        let (elapsed, sampler_live) = match prev.sample_at {
            Some(t) => {
                let elapsed = t.elapsed().as_secs_f64().max(0.001);
                (elapsed, elapsed <= MAX_SAMPLE_GAP_SECS)
            }
            // First sample: no baseline, so every per-second rate is
            // unknown rather than zero.
            None => (1.0, false),
        };
        prev.sample_at = Some(Instant::now());

        let cpu = sample_cpu(prev);
        let memory = sample_memory();
        let load = sample_load();
        let thermal = sample_thermal();
        let disks = sample_disks(elapsed);
        let network = sample_network(prev, elapsed, sampler_live);
        let services = sample_services(role);
        let clock = sample_clock();
        let (log_disk_bytes, log_disk_budget_bytes) = sample_log_disk();
        let platform = match role {
            HostNodeRole::Pi => Some(armee_proto::host_metrics::Platform::Pi(sample_pi_platform(
                &thermal,
            ))),
            HostNodeRole::Jetson | HostNodeRole::Unspecified => None,
        };

        HostMetrics {
            timestamp_ms,
            hostname: read_file_trim("/proc/sys/kernel/hostname")
                .unwrap_or_else(|| "unknown".to_string()),
            node_role: role as i32,
            uptime_sec: sample_uptime_sec(),
            kernel_version: read_file_trim("/proc/sys/kernel/osrelease").unwrap_or_default(),
            os_pretty_name: read_os_pretty_name(),
            build: Some(build_info(semver)),
            cpu: Some(cpu),
            memory: Some(memory),
            load: Some(load),
            thermal: Some(thermal),
            disks,
            network,
            services,
            chappe: Some(chappe.into_proto()),
            clock: Some(clock),
            platform,
            log_disk_bytes,
            log_disk_budget_bytes,
            // Live collectors: every section above is a real observation
            // (or an explicit unknown), never synthetic.
            simulated: false,
        }
    }

    fn sample_log_disk() -> (u64, u64) {
        let bytes = marengo_store::log_disk_usage_bytes(
            &marengo_store::resolve_marengo_root(),
            &marengo_store::resolve_db_path(),
        )
        .unwrap_or(0);
        (bytes, marengo_store::DEFAULT_LOG_DISK_BUDGET_BYTES)
    }

    fn read_os_pretty_name() -> String {
        fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content.lines().find_map(|line| {
                    line.strip_prefix("PRETTY_NAME=")
                        .map(|v| v.trim_matches('"').to_string())
                })
            })
            .unwrap_or_default()
    }

    fn sample_uptime_sec() -> u64 {
        read_file_trim("/proc/uptime")
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .map(|s| s as u64)
            .unwrap_or(0)
    }

    fn sample_cpu(prev: &mut SampleState) -> CpuMetrics {
        let content = fs::read_to_string("/proc/stat").unwrap_or_default();
        let mut aggregate = sample_cpu_from_stat(&content, prev);
        aggregate.freq_mhz =
            read_file_trim("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
                .and_then(|s| s.parse::<u64>().ok())
                .map(|khz| (khz / 1000) as u32)
                .unwrap_or(0);
        aggregate
    }

    fn sample_memory() -> MemoryMetrics {
        let content = fs::read_to_string("/proc/meminfo").ok().unwrap_or_default();
        let mut map = HashMap::new();
        for line in content.lines() {
            if let Some((key, val)) = line.split_once(':') {
                if let Ok(kb) = val.trim().trim_end_matches(" kB").parse::<u64>() {
                    map.insert(key.to_string(), kb * 1024);
                }
            }
        }
        let total = *map.get("MemTotal").unwrap_or(&0);
        let available = *map.get("MemAvailable").unwrap_or(&0);
        MemoryMetrics {
            total_bytes: total,
            used_bytes: total.saturating_sub(available),
            available_bytes: available,
            buffers_bytes: *map.get("Buffers").unwrap_or(&0),
            cached_bytes: *map.get("Cached").unwrap_or(&0),
            swap_total_bytes: *map.get("SwapTotal").unwrap_or(&0),
            swap_used_bytes: map
                .get("SwapTotal")
                .zip(map.get("SwapFree"))
                .map(|(t, f)| t.saturating_sub(*f))
                .unwrap_or(0),
            // A missing MemTotal never happens on a live kernel: zero
            // byte counts without this flag would read as an empty host.
            meminfo_known: total > 0,
        }
    }

    fn sample_load() -> LoadMetrics {
        let content = fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let parts: Vec<f64> = content
            .split_whitespace()
            .take(3)
            .filter_map(|s| s.parse().ok())
            .collect();
        LoadMetrics {
            load_1m: *parts.first().unwrap_or(&0.0),
            load_5m: *parts.get(1).unwrap_or(&0.0),
            load_15m: *parts.get(2).unwrap_or(&0.0),
        }
    }

    fn sample_thermal() -> ThermalMetrics {
        let mut zones = Vec::new();
        let mut cpu_c = 0.0;
        let mut cpu_known = false;
        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name =
                    read_file_trim(path.join("type")).unwrap_or_else(|| "unknown".to_string());
                let Some(milli): Option<i64> =
                    read_file_trim(path.join("temp")).and_then(|s| s.parse().ok())
                else {
                    // An unreadable zone is absent, never 0 °C.
                    continue;
                };
                cpu_known = true;
                let celsius = milli as f64 / 1000.0;
                if name.contains("cpu") || cpu_c == 0.0 {
                    cpu_c = celsius;
                }
                zones.push(ThermalZone { name, celsius });
            }
        }
        ThermalMetrics {
            cpu_celsius: cpu_c,
            zones,
            cpu_known,
            ..Default::default()
        }
    }

    fn sample_disks(_elapsed: f64) -> Vec<DiskMetrics> {
        crate::diagnostics::collect_mounts(&SystemSources, &["/", "/boot/firmware"])
    }

    fn sample_network(
        prev: &mut SampleState,
        elapsed: f64,
        baseline_valid: bool,
    ) -> Vec<NetworkInterfaceMetrics> {
        let content = fs::read_to_string("/proc/net/dev").unwrap_or_default();
        let mut out = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for line in content.lines().skip(2) {
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_string();
            if name.is_empty() || name == "lo" {
                continue;
            }
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() < 16 {
                continue;
            }
            let rx_bytes = nums[0];
            let tx_bytes = nums[8];
            // Unknown, never zero: first sample, stalled sampler, or a
            // counter reset/re-baseline all invalidate the rate.
            let (rx_bps, tx_bps, rates_valid) = match prev.network.get(&name) {
                Some(prev_net)
                    if baseline_valid
                        && rx_bytes >= prev_net.rx_bytes
                        && tx_bytes >= prev_net.tx_bytes =>
                {
                    (
                        ((rx_bytes - prev_net.rx_bytes) as f64 / elapsed) as u64,
                        ((tx_bytes - prev_net.tx_bytes) as f64 / elapsed) as u64,
                        true,
                    )
                }
                _ => (0, 0, false),
            };
            prev.network.insert(
                name.clone(),
                crate::sample_state::NetCounters { rx_bytes, tx_bytes },
            );
            seen.push(name.clone());
            let up = fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
                .map(|s| s.trim() == "up")
                .unwrap_or(false);
            let mut metric = NetworkInterfaceMetrics {
                name: name.clone(),
                up,
                rx_bytes_per_sec: rx_bps,
                tx_bytes_per_sec: tx_bps,
                rx_errors_total: nums[2],
                tx_errors_total: nums[10],
                rates_valid,
                ..Default::default()
            };
            if name.starts_with("can") {
                metric.can_state = read_can_state(&name);
                metric.can_tx_error_count =
                    read_stat_u64(&format!("/sys/class/net/{name}/statistics/tx_errors"));
                metric.can_rx_error_count =
                    read_stat_u64(&format!("/sys/class/net/{name}/statistics/rx_errors"));
            }
            out.push(metric);
        }
        // Drop baselines for interfaces that vanished (rename/remove);
        // a returning name re-baselines as unknown, never as a burst.
        prev.network.retain(|name, _| seen.contains(name));
        out
    }

    fn read_can_state(name: &str) -> String {
        crate::diagnostics::collect_can_state(name, |name| {
            crate::diagnostics::Sources::command(
                &SystemSources,
                "ip",
                &["-details", "link", "show", name],
            )
        })
    }

    fn read_stat_u64(path: &str) -> u64 {
        read_file_trim(path)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// B13 (D-1): the `HostNodeRole::Jetson` arm is deprecated (no producer) but
    /// kept so a Jetson role maps to no units, like Unspecified.
    #[allow(deprecated)]
    fn sample_services(role: HostNodeRole) -> Vec<ServiceStatus> {
        let units: &[&str] = match role {
            HostNodeRole::Pi => &[
                "marengo-pi.service",
                "marengo-gateway.service",
                "marengo-can.service",
            ],
            HostNodeRole::Jetson | HostNodeRole::Unspecified => &[],
        };
        units
            .iter()
            .filter_map(|unit| {
                // A failed query is an unknown service, never Inactive:
                // absent rows must not read as stopped units.
                let output = run_bounded(
                    "systemctl",
                    &["show", unit, "--property=ActiveState,NRestarts", "--value"],
                    COMMAND_TIMEOUT,
                )?;
                let (state, restarts) = parse_service_status(&output)?;
                Some(ServiceStatus {
                    unit: (*unit).to_string(),
                    state: state as i32,
                    restarts,
                })
            })
            .collect()
    }

    /// Parse `systemctl show --property=ActiveState,NRestarts --value`.
    /// Anything but a clean active/failed/inactive + restart count is
    /// unknown (`None`), never a fabricated state.
    fn parse_service_status(output: &str) -> Option<(ServiceState, u32)> {
        let mut lines = output.lines();
        let state = match lines.next().unwrap_or("unknown") {
            "active" => ServiceState::Active,
            "failed" => ServiceState::Failed,
            "inactive" => ServiceState::Inactive,
            _ => return None,
        };
        let restarts: u32 = lines.next()?.parse().ok()?;
        Some((state, restarts))
    }

    fn sample_clock() -> ClockMetrics {
        if let Some(output) = run_bounded(
            "timedatectl",
            &["show", "-p", "NTPSynchronized", "--value"],
            COMMAND_TIMEOUT,
        ) {
            let synced = output.trim() == "yes";
            return ClockMetrics {
                sync_source: "systemd-timesyncd".to_string(),
                synchronized: synced,
                ..Default::default()
            };
        }
        ClockMetrics::default()
    }

    fn sample_pi_platform(thermal: &ThermalMetrics) -> PiPlatformMetrics {
        let throttled = read_vcgencmd_throttled();
        PiPlatformMetrics {
            throttled_now: throttled.map(|(_, now)| now).unwrap_or(false),
            throttle_events: throttled.map(|(events, _)| events).unwrap_or(0),
            pmic_temp_celsius: thermal.cpu_celsius,
            // vcgencmd unavailable: throttle state is unknown, never clear.
            throttle_known: throttled.is_some(),
            ..Default::default()
        }
    }

    fn read_vcgencmd_throttled() -> Option<(u32, bool)> {
        let text = run_bounded("vcgencmd", &["get_throttled"], COMMAND_TIMEOUT)?;
        parse_throttled(&text)
    }

    /// Parse `vcgencmd get_throttled`. Currently-limiting bits:
    /// under-voltage now (0), throttled now (2), soft temperature limit
    /// (3). The raw mask stays in throttle_events; unknown (None) never
    /// reads as clear.
    fn parse_throttled(text: &str) -> Option<(u32, bool)> {
        let hex = text
            .trim()
            .strip_prefix("throttled=")
            .and_then(|s| s.strip_prefix("0x"))
            .unwrap_or("");
        let events = u32::from_str_radix(hex, 16).ok()?;
        Some((events, events & 0xD != 0))
    }

    fn read_file_trim(path: impl AsRef<Path>) -> Option<String> {
        fs::read_to_string(path.as_ref())
            .ok()
            .map(|s| s.trim().to_string())
    }

    #[cfg(test)]
    mod linux_collector_tests {
        use super::*;

        #[test]
        fn throttled_bits_cover_under_voltage_and_soft_temp() {
            // Bit 0 (under-voltage), bit 2 (throttled), bit 3 (soft temp)
            // all read as currently limiting; the raw mask is preserved.
            for (text, now) in [
                ("throttled=0x0\n", false),
                ("throttled=0x1\n", true),
                ("throttled=0x2\n", false),
                ("throttled=0x4\n", true),
                ("throttled=0x8\n", true),
                ("throttled=0x50005\n", true),
            ] {
                let (events, throttled_now) = parse_throttled(text).expect("parseable");
                assert_eq!(throttled_now, now, "{text}");
                assert!(events > 0 || !now);
            }
            assert!(parse_throttled("throttled=0xZZZ\n").is_none());
            assert!(parse_throttled("").is_none());
        }

        #[test]
        fn service_states_outside_active_failed_inactive_are_unknown() {
            assert_eq!(
                parse_service_status("active\n3\n"),
                Some((ServiceState::Active, 3))
            );
            assert_eq!(
                parse_service_status("failed\n0\n"),
                Some((ServiceState::Failed, 0))
            );
            assert_eq!(
                parse_service_status("inactive\n1\n"),
                Some((ServiceState::Inactive, 1))
            );
            for output in [
                "",
                "activating\n0\n",
                "active\n",
                "active\nmany\n",
                "unknown\n0\n",
            ] {
                assert!(parse_service_status(output).is_none(), "{output:?}");
            }
        }

        #[test]
        fn network_sampler_never_panics_without_procfs() {
            // No /proc/net/dev on macOS: empty content must clear stale
            // baselines, not panic. On Linux this exercises the live table.
            let mut prev = SampleState::default();
            prev.network.insert(
                "eth0".to_string(),
                crate::sample_state::NetCounters {
                    rx_bytes: 10_000,
                    tx_bytes: 5_000,
                },
            );
            let out = sample_network(&mut prev, 1.0, true);
            for metric in &out {
                assert!(metric.rates_valid || metric.rx_bytes_per_sec == 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use armee_proto::HostNodeRole;

    #[test]
    fn sample_returns_hostname() {
        let mut prev = SampleState::default();
        let metrics = sample(
            HostNodeRole::Pi,
            "0.1.0",
            &mut prev,
            ChappeHealthInput::default(),
        );
        assert!(!metrics.hostname.is_empty() || cfg!(not(target_os = "linux")));
    }
}
