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
