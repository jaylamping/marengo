# Batch23 — CPU counter columns and identity

G18 remains open. Baseline37a4dd1, branch codex/cpu-counter-columns.
The original collector has been extracted into a fixture seam without changing
parsing or delta behavior. An executed HostMetrics wire-roundtrip regression
asserts20% busy and10% iowait from independent literal kernel counters; it fails
on the original iowait behavior (0%). Probe bytes and baseline are frozen under
evidence/batch23. Production repair, CPU-ID/hotplug/reset unknown baselines,
malformed input checks, independent reviews, primary Linux CI and delivery remain
pending. No robot, deploy, limits, Wave or automation action.
