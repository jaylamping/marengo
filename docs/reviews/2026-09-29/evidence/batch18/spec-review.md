# Spec review of b23c82b...5084ad0

Independent reviewer /root/batch17_spec found one acceptance coverage gap: original
G17 requests truncated/corrupt gzip, but only truncation was exercised. Added a
complete gzip fixture with a flipped CRC checksum; actual public scan returns
Error::Io. One passing collected test and its output/hash are preserved.
Follow-up verification of this correction remains pending.

Otherwise the implementation matches G17: checked numeric conversions, documented
domains, preserved malformed timestamp skip/regression refusal, ASCII DLC match,
preallocation byte bounds, plain/gzip and expansion limits, exclusive absolute
micros upper boundary and actual CLI exit1 instead of101. Original/green probe
classification is accurate. No scope creep; G16 remains open. Full exact-head
Linux/default-feature gate and delivery remain pending.
