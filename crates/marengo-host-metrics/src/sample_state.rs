use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, Default)]
pub(crate) struct CpuLineValues {
    pub counters: [u64; 8],
}

impl CpuLineValues {
    pub(crate) fn rates(&self, prev: &Self) -> Option<(f64, f64)> {
        let mut deltas = [0; 8];
        for (i, delta) in deltas.iter_mut().enumerate() {
            *delta = self.counters[i].checked_sub(prev.counters[i])?;
        }
        let total = deltas
            .iter()
            .try_fold(0u64, |sum, value| sum.checked_add(*value))?;
        if total == 0 {
            return None;
        }
        let idle = deltas[3].checked_add(deltas[4])?;
        Some((
            (total - idle) as f64 * 100.0 / total as f64,
            deltas[4] as f64 * 100.0 / total as f64,
        ))
    }
}

#[derive(Clone, Default)]
pub(crate) struct NetCounters {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Previous sample for delta-based rates.
#[derive(Default)]
pub struct SampleState {
    pub sample_at: Option<Instant>,
    pub(crate) cpu_aggregate: Option<CpuLineValues>,
    pub(crate) cpu_per_core: HashMap<u32, CpuLineValues>,
    pub(crate) network: HashMap<String, NetCounters>,
}

/// Runtime Chappe health inputs from marengo-pi.
#[derive(Clone, Copy, Default)]
pub struct ChappeHealthInput {
    pub ipc_connected: bool,
    pub gateway_reachable: bool,
    pub last_publish_age_ms: u64,
    pub gateway_rtt_ms: f64,
    pub ipc_queue: Option<IpcQueueHealthInput>,
}

/// Primitive transport observations supplied by the runtime; no socket ownership.
#[derive(Clone, Copy, Default)]
pub struct IpcQueueHealthInput {
    pub queued_items: u64,
    pub queued_payload_bytes: u64,
    pub item_capacity: u64,
    pub payload_byte_capacity: u64,
    pub oldest_age_ms: u64,
    pub accepted_total: u64,
    pub coalesced_total: u64,
    pub dropped_total: u64,
    pub admitted_disconnected_total: u64,
    pub max_payload_bytes: u64,
    pub max_in_flight_payload_bytes: u64,
    pub write_failures_total: u64,
}

impl IpcQueueHealthInput {
    fn into_proto(self) -> armee_proto::IpcQueueHealth {
        armee_proto::IpcQueueHealth {
            queued_items: self.queued_items,
            queued_payload_bytes: self.queued_payload_bytes,
            item_capacity: self.item_capacity,
            payload_byte_capacity: self.payload_byte_capacity,
            oldest_age_ms: self.oldest_age_ms,
            accepted_total: self.accepted_total,
            coalesced_total: self.coalesced_total,
            dropped_total: self.dropped_total,
            admitted_disconnected_total: self.admitted_disconnected_total,
            max_payload_bytes: self.max_payload_bytes,
            max_in_flight_payload_bytes: self.max_in_flight_payload_bytes,
            write_failures_total: self.write_failures_total,
        }
    }
}

impl ChappeHealthInput {
    pub fn into_proto(self) -> armee_proto::ChappeHealth {
        armee_proto::ChappeHealth {
            ipc_connected: self.ipc_connected,
            gateway_reachable: self.gateway_reachable,
            last_publish_age_ms: self.last_publish_age_ms,
            ipc_queue: self.ipc_queue.map(IpcQueueHealthInput::into_proto),
        }
    }
}

#[cfg(test)]
mod ipc_health_wire_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use armee_proto::prost::Message;

    #[test]
    fn disconnected_queue_observations_survive_host_health_wire_conversion() {
        let input = ChappeHealthInput {
            ipc_connected: false,
            gateway_reachable: true,
            last_publish_age_ms: 31,
            gateway_rtt_ms: 0.0,
            ipc_queue: Some(IpcQueueHealthInput {
                queued_items: 7,
                queued_payload_bytes: 129,
                item_capacity: 135,
                payload_byte_capacity: 983040,
                oldest_age_ms: 1501,
                accepted_total: 100,
                coalesced_total: 999,
                dropped_total: 82,
                admitted_disconnected_total: 41,
                max_payload_bytes: 65536,
                max_in_flight_payload_bytes: 65536,
                write_failures_total: 3,
            }),
        };
        let encoded = input.into_proto().encode_to_vec();
        let decoded =
            armee_proto::ChappeHealth::decode(encoded.as_slice()).expect("host health wire");
        assert!(!decoded.ipc_connected && decoded.gateway_reachable);
        assert_eq!(decoded.last_publish_age_ms, 31);
        let queue = decoded.ipc_queue.expect("explicit queue evidence");
        assert_eq!(
            (
                queue.queued_items,
                queue.queued_payload_bytes,
                queue.oldest_age_ms
            ),
            (7, 129, 1501)
        );
        assert_eq!(
            (queue.item_capacity, queue.payload_byte_capacity),
            (135, 983040)
        );
        assert_eq!(
            (
                queue.accepted_total,
                queue.coalesced_total,
                queue.dropped_total,
                queue.admitted_disconnected_total,
                queue.write_failures_total
            ),
            (100, 999, 82, 41, 3)
        );
        assert_eq!(
            (queue.max_payload_bytes, queue.max_in_flight_payload_bytes),
            (65536, 65536)
        );
        assert!(
            ChappeHealthInput::default()
                .into_proto()
                .ipc_queue
                .is_none(),
            "no transport is not synthetic zero evidence"
        );
    }
}
