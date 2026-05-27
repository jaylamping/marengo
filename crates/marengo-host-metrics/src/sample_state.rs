use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, Default)]
pub(crate) struct CpuLineValues {
    pub total: u64,
    pub idle: u64,
    pub iowait: u64,
}

impl CpuLineValues {
    pub(crate) fn delta_usage(&self, prev: &Self) -> f64 {
        let total_delta = self.total.saturating_sub(prev.total);
        let idle_delta = self.idle.saturating_sub(prev.idle);
        if total_delta == 0 {
            return 0.0;
        }
        1.0 - (idle_delta as f64 / total_delta as f64)
    }

    pub(crate) fn delta_iowait(&self, prev: &Self) -> f64 {
        let total_delta = self.total.saturating_sub(prev.total);
        let iowait_delta = self.iowait.saturating_sub(prev.iowait);
        if total_delta == 0 {
            return 0.0;
        }
        iowait_delta as f64 / total_delta as f64
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
    pub(crate) cpu_per_core: Vec<CpuLineValues>,
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
