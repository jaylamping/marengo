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
}

impl ChappeHealthInput {
    pub fn into_proto(self) -> armee_proto::ChappeHealth {
        armee_proto::ChappeHealth {
            ipc_connected: self.ipc_connected,
            gateway_reachable: self.gateway_reachable,
            last_publish_age_ms: self.last_publish_age_ms,
        }
    }
}
