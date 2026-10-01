//! Parsing of host diagnostic observations supplied by Linux collectors.

pub(crate) fn can_state(text: &str) -> String {
    for line in text.lines() {
        if line.contains("can state") {
            return line
                .split_whitespace()
                .last()
                .unwrap_or("unknown")
                .to_string();
        }
    }
    if text.contains("BUS-OFF") {
        return "BUS-OFF".to_string();
    }
    if text.contains("ERROR-PASSIVE") {
        return "ERROR-PASSIVE".to_string();
    }
    if text.contains("ERROR-WARNING") {
        return "ERROR-WARNING".to_string();
    }
    String::new()
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
