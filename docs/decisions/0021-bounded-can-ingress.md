# ADR 0021: Frame integrity and bounded CAN ingress

Status: Accepted for this software slice. September 30, 2026 UTC.

## Problem

The fixed eight-byte transmit frame also represents receive traffic. SocketCAN
pads a short frame and discards its remote/error class, so malformed traffic can
be decoded as a complete status or detailed fault report. The outer receive
timeout cannot bound a backend that drains a busy socket until it becomes empty.
The multi-interface router drains one whole port before visiting its peers.
Enable flushes consequently have no trustworthy completion boundary.

## Decision

Keep fixed-size command encoding separate from receive evidence. The receive
envelope preserves identifier format, frame class, actual classic-CAN payload
length, payload bytes and host read time. A remote frame has no data payload;
its requested length is not permission to invent bytes. A CAN error frame is
transport evidence and never passes through the vendor device-ID decoder.
Recognized status types 2/24 and detailed type 21 require an extended data frame
with exactly eight bytes. A malformed recognized frame from a configured address
remains an ordered observation with its actual bytes and reason. Only qualified
header evidence from a data frame is retained separately; no malformed frame
refreshes a pose or becomes a complete detailed report.

Every backend implements a required bounded, nonblocking receive primitive.
There is no default implementation that calls an unbounded bulk method and
truncates its result. The driver engine owns one total work budget, shared by
all interfaces and polling rounds, counting every raw frame and read attempt
including noise, empty reads and interruptions. The initial per-poll limits are
64 raw frames and 256 read attempts. These are bounded software capacities,
not measured Pi throughput or permission to change motion limits. A caller's
positive time budget adds a host deadline. Zero time budget permits queued work
within the finite work limits and never waits for future traffic.

The router visits a stable interface inventory in round-robin order, reading
at most one frame per interface per round and rotating its first interface
between polls. A hot interface cannot drain its backlog before an already
queued peer is visited. A peer error retains the delivered prefix and first
terminal error; other peers still receive a bounded visit where possible.
Concrete sockets stay nonblocking in this path. Waiting for a quiet interval
belongs to the bounded engine, not a timeout per socket. Queue storage preserves
unread suffixes without copying the complete backlog on every limited read.

The pinned classic SocketCAN reader uses `read_exact`, which retries interrupted
reads inside the dependency. Open/configure a classic socket, safely clone its
owned descriptor and wrap it with the dependency's single-read receiver; this
preserves the same classic-mode kernel options without enabling FD reception.
Reject unexpected FD envelopes. Subscribe to all kernel error classes, preserving
their complete error mask and actual length. Use a single nonblocking write of
the dependency's encoded frame bytes and report errors/short writes without
retrying. No kernel ABI is reimplemented and no workspace `unsafe` is added.

Raw and decoded reports distinguish observed quiescence from exhaustion of a
work/deadline limit. Observed idle means every source was seen empty in a
bounded pass; observed quiet additionally satisfies the requested quiet policy.
Neither is an atomic physical snapshot. A completed idle pass may finish early
when insufficient tokens remain for another whole source round. Positive time
budgets are maximum waits; adaptive waits between idle passes avoid spending
the entire attempt quota on a healthy empty source. Hitting a cap without an
observed idle pass does not prove whether more traffic exists and must not
claim quiescence. An independent terminal
error and all observations remain available together. Vendor and transport
events retain their common raw delivery ordinal; separate projection lists
must not reorder the initiating hazard when receive timestamps tie.
The public feedback API exposes the lossless bounded report directly; legacy
bulk/cache projections were removed because callers must inspect ordered events,
terminal errors and completion together rather than silently reducing them to
latest-state caches or fixed-width frame vectors.

Davout inspects malformed evidence, terminal failure and incomplete completion
before authorizing further output. These observed receive hazards latch through
the existing private fault authority and attempt all-address stop. Ignoring the
returned error cannot restore permission. Ordinary invalid operator requests
remain rejected requests. Both enable flushes must establish quiescence before
the enable epoch is installed; a saturated or malformed post-enable flush
requires stop and refusal. No repeated unbounded flush is added to stop/rearm.

## Verification and scope

Use the already authorized public receive, Supervisor enable and motion
boundaries. Retain baseline-compatible regressions for oversized decoding,
bounded finite backlogs, unknown traffic, ignored overload and enable rollback.
New report/completion API tests establish conformance but do not count as a
baseline regression merely because the old API fails to compile. Real
SocketCAN tests use virtual interfaces and literal short data, remote and
two-interface traffic. Preserve their unchanged-baseline failure evidence and
candidate success on a Linux kernel with virtual CAN support; Docker Desktop's
missing local kernel support is an environment limitation, not a test pass.
Independent review and required primary/applicable checks precede merge.

This slice does not qualify firmware byte order, physical acquisition time,
drive limits/timeouts, stop acknowledgement, recovery, reference, GPIO inputs,
Pi publication or realtime jitter. Arbitrary third-party trait implementations
cannot be preempted by a Rust trait; all repository adapters must implement the
nonblocking work contract explicitly. The robot is outside this test environment.
