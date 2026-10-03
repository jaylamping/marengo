//! Pure Linux CPU counter parsing and identity-aware sample deltas.
//! No filesystem reads or commands; the collector supplies /proc/stat text.

use crate::sample_state::CpuLineValues;
use crate::SampleState;
use armee_proto::CpuMetrics;
#[cfg(test)]
use armee_proto::HostMetrics;
use std::collections::HashMap;

pub(crate) fn sample_cpu_from_stat(content: &str, prev: &mut SampleState) -> CpuMetrics {
    let mut aggregate = None;
    let mut seen_aggregate = false;
    let mut cores = std::collections::BTreeMap::new();
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else {
            continue;
        };
        if label == "cpu" {
            aggregate = if seen_aggregate {
                None
            } else {
                parse_cpu_fields(fields)
            };
            seen_aggregate = true;
        } else if let Some(id) = label
            .strip_prefix("cpu")
            .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|id| id.parse::<u32>().ok())
        {
            let vals = parse_cpu_fields(fields);
            cores
                .entry(id)
                .and_modify(|value| *value = None)
                .or_insert(vals);
        }
    }
    let same_ids = cores.len() == prev.cpu_per_core.len()
        && cores.keys().all(|id| prev.cpu_per_core.contains_key(id));
    let rates = aggregate
        .and_then(|now| prev.cpu_aggregate.and_then(|old| now.rates(&old)))
        .filter(|_| same_ids && !cores.is_empty());
    prev.cpu_aggregate = aggregate;
    let mut metrics = CpuMetrics {
        core_count: cores.len() as u32,
        ..Default::default()
    };
    if let Some((usage, iowait)) = rates {
        metrics.usage_percent = usage;
        metrics.iowait_percent = iowait;
        metrics.sample_valid = true;
    }
    let mut next = HashMap::new();
    for (id, values) in cores {
        let usage = values
            .and_then(|now| prev.cpu_per_core.get(&id).and_then(|old| now.rates(old)))
            .map(|(usage, _)| usage);
        metrics.cores.push(armee_proto::CpuCoreMetrics {
            cpu_id: id,
            usage_percent: usage,
        });
        if let Some(values) = values {
            next.insert(id, values);
        }
    }
    prev.cpu_per_core = next;
    metrics
}

// Kernel columns: user nice system idle iowait irq softirq steal.
// Guest fields are already included in user/nice; validate but do not add them.
fn parse_cpu_fields<'a>(fields: impl Iterator<Item = &'a str>) -> Option<CpuLineValues> {
    let nums: Vec<u64> = fields.map(str::parse).collect::<Result<_, _>>().ok()?;
    let counters: [u64; 8] = nums.get(..8)?.try_into().ok()?;
    counters
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(*value))?;
    Some(CpuLineValues { counters })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod cpu_regression {
    use super::*;
    use armee_proto::prost::Message;

    #[test]
    fn published_cpu_fields_follow_kernel_counter_columns() {
        let mut prev = SampleState::default();
        sample_cpu_from_stat(
            "cpu 0 0 0 0 0 0 0 0 0 0\ncpu0 0 0 0 0 0 0 0 0 0 0\ncpu1 0 0 0 0 0 0 0 0 0 0\n",
            &mut prev,
        );
        let cpu = sample_cpu_from_stat("cpu 10 0 10 70 10 0 0 0 7 0\ncpu0 10 0 10 80 0 0 0 0 7 0\ncpu1 10 0 10 70 10 0 0 0 7 0\n", &mut prev);
        let wire = HostMetrics {
            cpu: Some(cpu),
            ..Default::default()
        }
        .encode_to_vec();
        let decoded = HostMetrics::decode(wire.as_slice()).expect("wire metric");
        let cpu = decoded.cpu.expect("cpu");
        assert!((cpu.usage_percent - 20.0).abs() < 0.000001);
        assert!(
            (cpu.iowait_percent - 10.0).abs() < 0.000001,
            "iowait reads its own field: {}",
            cpu.iowait_percent
        );
        assert_eq!(cpu.core_count, 2);
        assert_eq!(cpu.cores.len(), 2);
        for core in cpu.cores {
            let usage = core.usage_percent.expect("primed core usage");
            assert!(
                (usage - 20.0).abs() < 0.000001,
                "core label must not shift counters: {usage}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod cpu_lifecycle_tests {
    use super::*;
    use armee_proto::prost::Message;

    fn published(content: &str, prev: &mut SampleState) -> CpuMetrics {
        let wire = HostMetrics {
            cpu: Some(sample_cpu_from_stat(content, prev)),
            ..Default::default()
        }
        .encode_to_vec();
        HostMetrics::decode(wire.as_slice())
            .expect("wire")
            .cpu
            .expect("cpu")
    }

    #[test]
    fn sparse_ids_reordering_and_hotplug_keep_independent_baselines() {
        let mut prev = SampleState::default();
        let first = published(
            "cpu 0 0 0 0 0 0 0 0\ncpu2 0 0 0 0 0 0 0 0\ncpu9 0 0 0 0 0 0 0 0\n",
            &mut prev,
        );
        assert!(!first.sample_valid);
        assert!(first.cores.iter().all(|core| core.usage_percent.is_none()));
        let next = published(
            "cpu 30 0 0 170 0 0 0 0\ncpu9 20 0 0 80 0 0 0 0\ncpu2 10 0 0 90 0 0 0 0\n",
            &mut prev,
        );
        assert!(next.sample_valid);
        assert_eq!(
            next.cores
                .iter()
                .map(|core| (core.cpu_id, core.usage_percent))
                .collect::<Vec<_>>(),
            vec![(2, Some(10.0)), (9, Some(20.0))]
        );
        let removed = published(
            "cpu 40 0 0 260 0 0 0 0\ncpu9 30 0 0 170 0 0 0 0\n",
            &mut prev,
        );
        assert!(!removed.sample_valid);
        assert_eq!(removed.cores[0].usage_percent, Some(10.0));
        let returned = published(
            "cpu 60 0 0 440 0 0 0 0\ncpu2 20 0 0 180 0 0 0 0\ncpu9 40 0 0 260 0 0 0 0\n",
            &mut prev,
        );
        assert!(!returned.sample_valid);
        assert_eq!(returned.cores[0].usage_percent, None);
        assert_eq!(returned.cores[1].usage_percent, Some(10.0));
    }

    #[test]
    fn reset_even_with_increasing_total_invalidates_then_rebaselines() {
        let mut prev = SampleState::default();
        published("cpu 50 0 0 50 0 0 0 0\ncpu0 50 0 0 50 0 0 0 0\n", &mut prev);
        let reset = published(
            "cpu 10 0 0 200 0 0 0 0\ncpu0 10 0 0 200 0 0 0 0\n",
            &mut prev,
        );
        assert!(!reset.sample_valid);
        assert_eq!(reset.cores[0].usage_percent, None);
        let fresh = published(
            "cpu 30 0 0 280 0 0 0 0\ncpu0 30 0 0 280 0 0 0 0\n",
            &mut prev,
        );
        assert!(fresh.sample_valid);
        assert_eq!(fresh.usage_percent, 20.0);
        assert_eq!(fresh.cores[0].usage_percent, Some(20.0));
        let unchanged = published(
            "cpu 30 0 0 280 0 0 0 0\ncpu0 30 0 0 280 0 0 0 0\n",
            &mut prev,
        );
        assert!(!unchanged.sample_valid);
        assert_eq!(unchanged.cores[0].usage_percent, None);
    }

    #[test]
    fn malformed_truncated_duplicate_and_overflow_are_unknown() {
        for fields in [
            "10 0 bad 80 0 0 0 0",
            "10 0 10 80",
            "18446744073709551615 1 0 0 0 0 0 0",
            "10 0 10 80 0 0 0 0 bad",
            "-1 0 0 80 0 0 0 0",
        ] {
            let mut prev = SampleState::default();
            published("cpu 0 0 0 0 0 0 0 0\ncpu0 0 0 0 0 0 0 0 0\n", &mut prev);
            let invalid = published(&format!("cpu {fields}\ncpu0 {fields}\n"), &mut prev);
            assert!(!invalid.sample_valid, "{fields}");
            assert_eq!(invalid.cores[0].usage_percent, None, "{fields}");
            let next = published("cpu 20 0 0 80 0 0 0 0\ncpu0 20 0 0 80 0 0 0 0\n", &mut prev);
            assert!(!next.sample_valid);
            assert_eq!(next.cores[0].usage_percent, None);
        }
        let mut prev = SampleState::default();
        let duplicate = published("cpu 0 0 0 0 0 0 0 0\ncpu 1 0 0 0 0 0 0 0\ncpu3 0 0 0 0 0 0 0 0\ncpu3 1 0 0 0 0 0 0 0\n", &mut prev);
        assert!(!duplicate.sample_valid);
        assert_eq!(duplicate.cores.len(), 1);
        assert_eq!(duplicate.cores[0].usage_percent, None);
        let missing = published("intr 123\ncpuBogus 1 0 0 0 0 0 0 0\n", &mut prev);
        assert!(!missing.sample_valid);
        assert!(missing.cores.is_empty());
    }

    #[test]
    fn irq_is_busy_guest_is_not_double_counted_and_iowait_decrease_is_unknown() {
        let mut prev = SampleState::default();
        published(
            "cpu 0 0 0 0 0 0 0 0 0 0\ncpu0 0 0 0 0 0 0 0 0 0 0\n",
            &mut prev,
        );
        let cpu = published(
            "cpu 10 0 10 60 10 10 0 0 1000 1000\ncpu0 10 0 10 60 10 10 0 0 1000 1000\n",
            &mut prev,
        );
        assert_eq!(cpu.usage_percent, 30.0);
        assert_eq!(cpu.iowait_percent, 10.0);
        let decrease = published(
            "cpu 20 0 20 120 9 20 0 0 1000 1000\ncpu0 20 0 20 120 9 20 0 0 1000 1000\n",
            &mut prev,
        );
        assert!(!decrease.sample_valid);
        assert_eq!(decrease.cores[0].usage_percent, None);
    }
}
