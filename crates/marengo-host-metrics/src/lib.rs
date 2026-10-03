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
    use std::time::Instant;

    use armee_proto::{
        ClockMetrics, CpuMetrics, DiskMetrics, HostMetrics, HostNodeRole, LoadMetrics,
        MemoryMetrics, NetworkInterfaceMetrics, PiPlatformMetrics, ServiceState, ServiceStatus,
        ThermalMetrics, ThermalZone,
    };

    use super::build_info;
    use crate::cpu::sample_cpu_from_stat;
    use crate::{ChappeHealthInput, SampleState};

    struct SystemSources;
    impl crate::diagnostics::Sources for SystemSources {
        fn command(&self, program: &str, args: &[&str]) -> Option<String> {
            let output = std::process::Command::new(program)
                .args(args)
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            String::from_utf8(output.stdout).ok()
        }
        fn read_file(&self, path: &str) -> Option<String> {
            fs::read_to_string(path).ok()
        }
    }

    pub fn sample(
        role: HostNodeRole,
        semver: &str,
        prev: &mut SampleState,
        chappe: ChappeHealthInput,
        timestamp_ms: u64,
    ) -> HostMetrics {
        let elapsed = prev
            .sample_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(1.0)
            .max(0.001);
        prev.sample_at = Some(Instant::now());

        let cpu = sample_cpu(prev);
        let memory = sample_memory();
        let load = sample_load();
        let thermal = sample_thermal();
        let disks = sample_disks(elapsed);
        let network = sample_network(prev, elapsed);
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
        let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
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
        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name =
                    read_file_trim(path.join("type")).unwrap_or_else(|| "unknown".to_string());
                let milli: i64 = read_file_trim(path.join("temp"))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
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
            ..Default::default()
        }
    }

    fn sample_disks(_elapsed: f64) -> Vec<DiskMetrics> {
        crate::diagnostics::collect_disks(&SystemSources, &["/", "/boot/firmware"])
    }

    fn sample_network(prev: &mut SampleState, elapsed: f64) -> Vec<NetworkInterfaceMetrics> {
        let content = fs::read_to_string("/proc/net/dev").unwrap_or_default();
        let mut out = Vec::new();
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
            let (rx_bps, tx_bps) = if let Some(prev_net) = prev.network.get(&name) {
                (
                    ((rx_bytes.saturating_sub(prev_net.rx_bytes)) as f64 / elapsed) as u64,
                    ((tx_bytes.saturating_sub(prev_net.tx_bytes)) as f64 / elapsed) as u64,
                )
            } else {
                (0, 0)
            };
            prev.network.insert(
                name.clone(),
                crate::sample_state::NetCounters { rx_bytes, tx_bytes },
            );
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
                let output = std::process::Command::new("systemctl")
                    .args(["show", unit, "--property=ActiveState,NRestarts", "--value"])
                    .output()
                    .ok()?;
                let lines: Vec<&str> = std::str::from_utf8(&output.stdout).ok()?.lines().collect();
                let active = lines.first().copied().unwrap_or("inactive");
                let restarts: u32 = lines.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let state = match active {
                    "active" => ServiceState::Active,
                    "failed" => ServiceState::Failed,
                    _ => ServiceState::Inactive,
                };
                Some(ServiceStatus {
                    unit: (*unit).to_string(),
                    state: state as i32,
                    restarts,
                })
            })
            .collect()
    }

    fn sample_clock() -> ClockMetrics {
        if let Ok(output) = std::process::Command::new("timedatectl")
            .args(["show", "-p", "NTPSynchronized", "--value"])
            .output()
        {
            if output.status.success() {
                let synced = std::str::from_utf8(&output.stdout)
                    .map(|s| s.trim() == "yes")
                    .unwrap_or(false);
                return ClockMetrics {
                    sync_source: "systemd-timesyncd".to_string(),
                    synchronized: synced,
                    ..Default::default()
                };
            }
        }
        ClockMetrics::default()
    }

    fn sample_pi_platform(thermal: &ThermalMetrics) -> PiPlatformMetrics {
        let throttled = read_vcgencmd_throttled();
        PiPlatformMetrics {
            throttled_now: throttled.map(|(_, now)| now).unwrap_or(false),
            throttle_events: throttled.map(|(events, _)| events).unwrap_or(0),
            pmic_temp_celsius: thermal.cpu_celsius,
            ..Default::default()
        }
    }

    fn read_vcgencmd_throttled() -> Option<(u32, bool)> {
        let output = std::process::Command::new("vcgencmd")
            .arg("get_throttled")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = std::str::from_utf8(&output.stdout).ok()?;
        let hex = text
            .trim()
            .strip_prefix("throttled=")
            .and_then(|s| s.strip_prefix("0x"))
            .unwrap_or("");
        let events = u32::from_str_radix(hex, 16).ok()?;
        Some((events, events & 0x4 != 0))
    }

    fn read_file_trim(path: impl AsRef<Path>) -> Option<String> {
        fs::read_to_string(path.as_ref())
            .ok()
            .map(|s| s.trim().to_string())
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
