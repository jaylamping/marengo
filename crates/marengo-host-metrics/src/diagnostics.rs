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

/// Path-fallback disk collection (attributes the covering mount). Kept for
/// the frozen mount-flag regressions; production uses [`collect_mounts`].
#[cfg(test)]
pub(crate) fn collect_disks(
    source: &impl Sources,
    mounts: &[&str],
) -> Vec<armee_proto::DiskMetrics> {
    let table = read_mount_table(source);
    mounts
        .iter()
        .map(|mount| disk_for_path(source, table.as_ref(), mount))
        .collect()
}

/// Mount-point collection for the production sampler: identity and capacity
/// require the mount itself to appear in mountinfo. An unmounted path (e.g.
/// `/boot/firmware`) yields an explicitly unknown row, never the parent
/// filesystem's identity or usage.
pub(crate) fn collect_mounts(
    source: &impl Sources,
    mounts: &[&str],
) -> Vec<armee_proto::DiskMetrics> {
    let table = read_mount_table(source);
    mounts
        .iter()
        .map(|mount| {
            let mut metric = armee_proto::DiskMetrics {
                mount_point: (*mount).into(),
                ..Default::default()
            };
            let mounted = table
                .as_ref()
                .and_then(|entries| entries.iter().find(|entry| entry.path == *mount));
            let Some(entry) = mounted else {
                return metric;
            };
            metric.filesystem = entry.filesystem.clone();
            metric.source_device = entry.source.clone();
            if let Some(read_only) = entry.read_only {
                metric.mount_status_known = true;
                metric.read_only = read_only;
            }
            if let Some((total, used)) = source
                .command("df", &["-B1", mount])
                .and_then(|text| parse_capacity(&text))
            {
                metric.total_bytes = total;
                metric.used_bytes = used;
                metric.capacity_known = true;
                metric.nearly_full = used as f64 / total as f64 >= 0.9;
            }
            metric
        })
        .collect()
}

fn read_mount_table(source: &impl Sources) -> Option<Vec<Mount>> {
    source
        .read_file("/proc/self/mountinfo")
        .and_then(|text| text.lines().map(parse_mount).collect::<Option<Vec<_>>>())
}

#[cfg(test)]
fn disk_for_path(
    source: &impl Sources,
    table: Option<&Vec<Mount>>,
    mount: &str,
) -> armee_proto::DiskMetrics {
    let mut metric = armee_proto::DiskMetrics {
        mount_point: mount.into(),
        ..Default::default()
    };
    if let Some(entries) = table.as_ref() {
        let candidates: Vec<_> = entries
            .iter()
            .filter(|entry| {
                entry.path == *mount
                    || entry.path == "/"
                    || mount.starts_with(&format!("{}/", entry.path))
            })
            .collect();
        if let Some(longest) = candidates.iter().map(|entry| entry.path.len()).max() {
            let selected: Vec<_> = candidates
                .into_iter()
                .filter(|entry| entry.path.len() == longest)
                .collect();
            if selected.len() == 1 {
                let entry = selected[0];
                metric.filesystem = entry.filesystem.clone();
                metric.source_device = entry.source.clone();
                if let Some(read_only) = entry.read_only {
                    metric.mount_status_known = true;
                    metric.read_only = read_only;
                }
            }
        }
    }
    if let Some((total, used)) = source
        .command("df", &["-B1", mount])
        .and_then(|text| parse_capacity(&text))
    {
        metric.total_bytes = total;
        metric.used_bytes = used;
        metric.capacity_known = true;
        metric.nearly_full = used as f64 / total as f64 >= 0.9;
    }
    metric
}

struct Mount {
    path: String,
    filesystem: String,
    source: String,
    read_only: Option<bool>,
}

fn parse_mount(line: &str) -> Option<Mount> {
    let (before, after) = line.split_once(" - ")?;
    let left: Vec<_> = before.split_whitespace().collect();
    let right: Vec<_> = after.split_whitespace().collect();
    if left.len() < 6 || right.len() != 3 {
        return None;
    }
    left[0].parse::<u64>().ok()?;
    left[1].parse::<u64>().ok()?;
    let (major, minor) = left[2].split_once(':')?;
    major.parse::<u32>().ok()?;
    minor.parse::<u32>().ok()?;
    let path = unescape_mount(left[4])?;
    if !path.starts_with('/') {
        return None;
    }
    let read_only = option_read_only(left[5])
        .zip(option_read_only(right[2]))
        .map(|(mount, superblock)| mount || superblock);
    Some(Mount {
        path,
        filesystem: right[0].into(),
        source: unescape_mount(right[1])?,
        read_only,
    })
}

fn option_read_only(options: &str) -> Option<bool> {
    let fields: Vec<_> = options.split(',').collect();
    match (fields.contains(&"ro"), fields.contains(&"rw")) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

fn unescape_mount(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            let escape = [bytes.next()?, bytes.next()?, bytes.next()?];
            out.push(match &escape {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return None,
            });
        } else {
            out.push(byte);
        }
    }
    String::from_utf8(out).ok()
}

fn parse_capacity(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().nth(1)?;
    let cols: Vec<_> = line.split_whitespace().collect();
    if cols.len() < 6 {
        return None;
    }
    let total = cols[1].parse::<u64>().ok()?;
    let used = cols[2].parse::<u64>().ok()?;
    if total == 0 || used > total {
        return None;
    }
    Some((total, used))
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod mount_collector_tests {
    use super::*;
    use armee_proto::{prost::Message, DiskMetrics, HostMetrics};
    struct Fixture {
        mountinfo: Option<&'static str>,
        df: Option<&'static str>,
    }
    impl Sources for Fixture {
        fn command(&self, program: &str, args: &[&str]) -> Option<String> {
            assert_eq!(program, "df");
            assert_eq!(args[0], "-B1");
            self.df.map(str::to_owned)
        }
        fn read_file(&self, path: &str) -> Option<String> {
            assert_eq!(path, "/proc/self/mountinfo");
            self.mountinfo.map(str::to_owned)
        }
    }
    fn published(source: Fixture, path: &str) -> DiskMetrics {
        let wire = HostMetrics {
            disks: collect_disks(&source, &[path]),
            ..Default::default()
        }
        .encode_to_vec();
        HostMetrics::decode(wire.as_slice())
            .expect("wire")
            .disks
            .remove(0)
    }
    fn published_mount(source: Fixture, mount: &str) -> DiskMetrics {
        let wire = HostMetrics {
            disks: collect_mounts(&source, &[mount]),
            ..Default::default()
        }
        .encode_to_vec();
        HostMetrics::decode(wire.as_slice())
            .expect("wire")
            .disks
            .remove(0)
    }
    const DF: &str =
        "Filesystem 1B-blocks Used Available Use% Mounted on\n/dev/root 100 90 10 90% /\n";
    const RW: &str = "36 35 98:0 / / rw,noatime shared:1 - ext4 /dev/root rw,errors=continue\n";
    #[test]
    fn flags_filesystem_source_and_capacity_survive_publication() {
        let rw = published(
            Fixture {
                mountinfo: Some(RW),
                df: Some(DF),
            },
            "/",
        );
        assert!(rw.mount_status_known && rw.capacity_known && !rw.read_only && rw.nearly_full);
        assert_eq!(
            (rw.filesystem.as_str(), rw.source_device.as_str()),
            ("ext4", "/dev/root")
        );
        assert_eq!((rw.total_bytes, rw.used_bytes), (100, 90));
        let super_ro = published(
            Fixture {
                mountinfo: Some("36 35 98:0 / / rw - ext4 /dev/root ro\n"),
                df: Some(DF),
            },
            "/",
        );
        assert!(super_ro.mount_status_known && super_ro.read_only);
    }
    #[test]
    fn escaped_paths_and_sources_choose_the_specific_mount() {
        let metric = published(Fixture {
            mountinfo: Some("36 35 98:0 / / rw - ext4 /dev/root rw\n40 36 8:1 / /mnt/a\\040b ro optional:unknown - vfat /dev/a\\134b rw\n"),
            df: Some(DF),
        }, "/mnt/a b/file");
        assert!(metric.mount_status_known && metric.read_only);
        assert_eq!(metric.filesystem, "vfat");
        assert_eq!(metric.source_device, "/dev/a\\b");
        let sibling = published(Fixture { mountinfo: Some("36 35 98:0 / / rw - ext4 /dev/root rw\n40 36 8:1 / /mnt/a ro - vfat /dev/boot rw\n"), df: Some(DF) }, "/mnt/ab/file");
        assert!(!sibling.read_only);
        assert_eq!(sibling.filesystem, "ext4");
        assert_eq!(
            unescape_mount("/tab\\011line\\012"),
            Some("/tab\tline\n".into())
        );
    }
    #[test]
    fn unavailable_or_ambiguous_mount_flags_are_explicitly_unknown() {
        for mountinfo in [
            None,
            Some("invalid"),
            Some("36 35 98:0 / / rw,ro - ext4 /dev/root rw\n"),
            Some("36 35 98:0 / / rw - ext4 /dev/root rw\n40 36 8:1 / / ro - vfat /dev/boot rw\n"),
            Some("36 35 98:0 / /mnt/bad\\999 ro - ext4 /dev/root rw\n"),
        ] {
            let metric = published(
                Fixture {
                    mountinfo,
                    df: Some(DF),
                },
                "/",
            );
            assert!(!metric.mount_status_known);
            assert!(metric.capacity_known);
        }
    }
    #[test]
    fn unmounted_mount_point_is_unknown_not_the_parent_filesystem() {
        // /boot/firmware absent from mountinfo: no root identity, no
        // parent-fs capacity, even though df would succeed on the path.
        let metric = published_mount(
            Fixture {
                mountinfo: Some(RW),
                df: Some(DF),
            },
            "/boot/firmware",
        );
        assert_eq!(metric.mount_point, "/boot/firmware");
        assert!(metric.filesystem.is_empty());
        assert!(metric.source_device.is_empty());
        assert!(!metric.mount_status_known);
        assert!(!metric.capacity_known);
        assert_eq!((metric.total_bytes, metric.used_bytes), (0, 0));
        // The mounted root still reports fully.
        let root = published_mount(
            Fixture {
                mountinfo: Some(RW),
                df: Some(DF),
            },
            "/",
        );
        assert!(root.mount_status_known && root.capacity_known);
        assert_eq!(root.filesystem, "ext4");
    }

    #[test]
    fn command_failures_and_invalid_capacity_do_not_fabricate_writable_or_zero_capacity() {
        for df in [
            None,
            Some("failed"),
            Some("head\n/dev/root bad 90 10 90% /\n"),
            Some("head\n/dev/root 0 0 0 0% /\n"),
            Some("head\n/dev/root 100 101 0 100% /\n"),
        ] {
            let metric = published(
                Fixture {
                    mountinfo: Some(RW),
                    df,
                },
                "/",
            );
            assert!(metric.mount_status_known && !metric.read_only);
            assert!(!metric.capacity_known && !metric.nearly_full);
        }
        let unknown = published(
            Fixture {
                mountinfo: None,
                df: None,
            },
            "/",
        );
        assert!(!unknown.capacity_known && !unknown.mount_status_known);
    }
}
