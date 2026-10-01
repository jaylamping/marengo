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
            for usage in cpu.per_core_usage_percent {
                assert!(
                    (usage - 20.0).abs() < 0.000001,
                    "core label must not shift counters: {usage}"
                );
            }
        }
    }

