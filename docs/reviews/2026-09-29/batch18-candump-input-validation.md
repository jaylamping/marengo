# Batch18: refuse unrepresentable candump input without unwinding

Status: native Mac qualification complete; independent review and exact-head Linux
CI/delivery pending. Baselineb23c82ba73b6372889f950379db97273053fe147.
Branchcodex/candump-input-validation, isolated from batch17. G17 remains open
until review and required checks complete; G16 remains open for archive worker,
page/count/download/cancellation acceptance.

## Behavior

Use checked Duration conversion for scanner timestamps/offsets and Frame JSON
offsets. Delta raw timestamps and normalized offsets must fit Duration; absolute
raw timestamps must round to fewer than2^64 Unix microseconds. Finite out-of-domain
capture timestamps return a typed source-line error; JSON offsets return a serde
error. Negative/nonfinite capture timestamps remain skipped malformed frames;
regressions retain their existing typed refusal. Zero/subnanosecond rounding and
representable upper-range JSON offsets remain accepted.

ASCII classic CAN DLC must be0..8 and match decoded payload bytes. Mismatched or
oversize DLC is skipped as malformed, preserving physical-line and parsed-page
accounting. Bound the reader before line allocation to4096bytes including newline;
count at most256MiB of decompressed input separately from compressed source size.
Oversized plain/gzip lines and gzip expansion return typed errors; truncated gzip
propagates I/O failure. No public limit override or caller bypass was added.
The existing CLI propagates typed input errors as exit1 instead of panic101.

## Evidence

The independently frozen public probes reproduce four library assertion failures
on delivered production: scanner/serde unwind on1e30, accepted DLC mismatches and
accepted8192-byte padding. One numeric/malformed-frame control already passes.
The actual CLI assertion reproduces panic exit101. Both entire probes replay
byte-identically green after the repair. Evidence/batch18 original-binding.json
and qualification.json bind whole probes to captured original/green output.

Candidate-only boundary tests qualify exact4096-byte plain/gzip lines,
256MiB expansion acceptance and one extra byte refusal, truncated gzip I/O error,
and an absolute-microsecond float upper bound that must not saturate to u64::MAX.
Native affected Candump/Store/plain CLI59tests pass, none ignored; strict affected
clippy and workspace formatting pass. Default-feature CLI/enrichment and full
Linux/runtime checks remain for exact-head CI. No hardware or performance/durability
acceptance is inferred.

Excluded preparation: the first candidate boundary fixture was4097bytes because
its padding calculation omitted the newline; corrected fixture4096passes. The
first strict clippy identified manual range syntax; fixed with Range::contains.
These are candidate preparation/lint failures, not original defect evidence.
The256MiB gzip boundary fixture generation takes approximately17seconds natively;
no speedup claim is made. Its peak fixture-generation buffer is one4096-byte block
plus compressed output; scanner line retention is bounded before allocation.

Review and delivery are pending. The paused Windows schedule, robot, motor limits,
Wave sign-off and Windows-local CAD/recovery material remain untouched.
