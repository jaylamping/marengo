//! Parsing of host diagnostic observations supplied by Linux collectors.

pub(crate) fn can_state(text: &str) -> String {
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() != Some(&"can") {
            continue;
        }
        if let Some(state) = fields
            .windows(2)
            .find_map(|pair| (pair[0] == "state").then_some(pair[1]))
        {
            if matches!(
                state,
                "ERROR-ACTIVE"
                    | "ERROR-WARNING"
                    | "ERROR-PASSIVE"
                    | "BUS-OFF"
                    | "STOPPED"
                    | "SLEEPING"
            ) {
                return state.to_string();
            }
        }
    }
    "UNKNOWN".into()
}

/// A failed command is an unavailable observation, not a healthy CAN state.
pub(crate) fn collect_can_state(name: &str, read: impl FnOnce(&str) -> Option<String>) -> String {
    read(name)
        .map(|text| can_state(&text))
        .unwrap_or_else(|| "UNKNOWN".into())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod regression {
    use super::*;
    use armee_proto::prost::Message;
    use armee_proto::{HostMetrics, NetworkInterfaceMetrics};

    fn published_state(text: &str) -> String {
        let wire = HostMetrics {
            network: vec![NetworkInterfaceMetrics {
                name: "can0".into(),
                can_state: can_state(text),
                ..Default::default()
            }],
            ..Default::default()
        }
        .encode_to_vec();
        HostMetrics::decode(wire.as_slice()).expect("wire").network[0]
            .can_state
            .clone()
    }

    #[test]
    fn restart_delay_is_not_the_published_can_state() {
        assert_eq!(
            published_state("can state ERROR-ACTIVE restart-ms 100\n"),
            "ERROR-ACTIVE"
        );
    }

    #[test]
    fn kernel_flagged_can_example_preserves_its_state() {
        assert_eq!(
            published_state("can <TRIPLE-SAMPLING> state ERROR-ACTIVE restart-ms 100\n"),
            "ERROR-ACTIVE"
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod collector_tests {
    use super::*;
    use armee_proto::prost::Message;
    use armee_proto::{HostMetrics, NetworkInterfaceMetrics};

    #[test]
    fn command_adapter_publishes_states_and_unknown_failures() {
        for (output, expected) in [
            (
                Some("can <FD> state ERROR-ACTIVE (berr-counter tx 0 rx 0) restart-ms 0"),
                "ERROR-ACTIVE",
            ),
            (
                Some("can <LISTEN-ONLY> state BUS-OFF restart-ms 100"),
                "BUS-OFF",
            ),
            (
                Some("can state ERROR-PASSIVE restart-ms 100"),
                "ERROR-PASSIVE",
            ),
            (
                Some("can state ERROR-WARNING restart-ms 0"),
                "ERROR-WARNING",
            ),
            (Some("can state STOPPED restart-ms 0"), "STOPPED"),
            (Some("2: can0: state UP\nstatistics BUS-OFF 100"), "UNKNOWN"),
            (Some("can state invalid restart-ms 0"), "UNKNOWN"),
            (None, "UNKNOWN"),
        ] {
            let state = collect_can_state("can0", |name| {
                assert_eq!(name, "can0");
                output.map(str::to_owned)
            });
            let wire = HostMetrics {
                network: vec![NetworkInterfaceMetrics {
                    name: "can0".into(),
                    can_state: state,
                    ..Default::default()
                }],
                ..Default::default()
            }
            .encode_to_vec();
            assert_eq!(
                HostMetrics::decode(wire.as_slice()).expect("wire").network[0].can_state,
                expected
            );
        }
    }
}

/// Collector inputs are supplied by the host or deterministic fixtures.
pub(crate) trait Sources {
    fn command(&self, program: &str, args: &[&str]) -> Option<String>;
    fn read_file(&self, path: &str) -> Option<String>;
}

pub(crate) fn collect_disks(
    source: &impl Sources,
    mounts: &[&str],
) -> Vec<armee_proto::DiskMetrics> {
    mounts
        .iter()
        .filter_map(|mount| {
            let output = source.command("df", &["-B1", mount])?;
            let line = output.lines().nth(1)?;
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 {
                return None;
            }
            let total: u64 = cols[1].parse().ok()?;
            let used: u64 = cols[2].parse().ok()?;
            let used_pct = if total > 0 {
                used as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            Some(armee_proto::DiskMetrics {
                mount_point: (*mount).into(),
                filesystem: cols[0].into(),
                total_bytes: total,
                used_bytes: used,
                read_only: cols.contains(&"ro"),
                nearly_full: used_pct >= 90.0,
                ..Default::default()
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod disk_regression {
    use super::*;
    use armee_proto::{prost::Message, HostMetrics};
    struct Fixture;
    impl Sources for Fixture {
        fn command(&self, program: &str, args: &[&str]) -> Option<String> {
            assert_eq!(program, "df");
            assert!(args.contains(&"/"));
            Some(
                "Filesystem 1B-blocks Used Available Use% Mounted on\n/dev/root 100 90 10 90% /\n"
                    .into(),
            )
        }
        fn read_file(&self, path: &str) -> Option<String> {
            assert_eq!(path, "/proc/self/mountinfo");
            Some("36 35 98:0 / / ro,noatime shared:1 - ext4 /dev/root rw,errors=continue\n".into())
        }
    }
    #[test]
    fn published_read_only_comes_from_mount_flags_not_df_columns() {
        let wire = HostMetrics {
            disks: collect_disks(&Fixture, &["/"]),
            ..Default::default()
        }
        .encode_to_vec();
        let metric = HostMetrics::decode(wire.as_slice()).expect("wire");
        assert!(
            metric.disks[0].read_only,
            "read-only root must not be reported writable"
        );
    }
}
