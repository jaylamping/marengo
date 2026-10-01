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
