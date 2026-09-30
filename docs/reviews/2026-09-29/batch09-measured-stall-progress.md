# Ninth repair batch: measured encoder progress and stalled ascent

Checked baseline: PR222 merge `04e4ea2474f67cb3947b74a72fd4dafb09625060`.
Branch: `codex/measured-stall-progress`. Windows source, local CAD and all
evidence remain under `J:\code`. Evidence root:
`J:/code/marengo-migration-backup-20260929/batch09`.
Status: verified software at the implementation head of [PR223](https://github.com/jaylamping/marengo/pull/223); final97881d8/run36756367932 and equal-tree merged7a25bbb/main36757300231 also pass all five jobs and the fatal main release. Verified backup and exact branch cleanup are complete.
All five jobs pass at `eb1359cc82697966cc4e6d32db3fef72111e5999` in
[run36755369469](https://github.com/jaylamping/marengo/actions/runs/36755369469).
The102-ID ledger now records14 verified,10 partial and78 open findings.

## Software change

The positive velocity filter tail previously renewed the ascent budget forever.
The watchdog also lost its lifetime on velocity spikes/planner recovery exits;
inactive peers could fault a selected owner, and integer milliseconds lost all
time above1000Hz. [ADR0025](../../decisions/0025-measured-ascent-progress.md)
separates geometric progress from planner recovery. Only a qualifying new
measured high-water level on an actually commanded unresolved ascent resets
the unchanged2-second budget. Retreat, repeated peaks, velocity and resync do
not renew it. Subthreshold movement accumulates against the credited level;
settle, true retarget, stop/mode exit and the existing Wave exemption end the
episode. Identical-target retarget retains it. Existing lead, recovery,
friction, gains, master limits and Wave sign-off are preserved.

Robstride describes the actual decoder grid. Davout converts that metadata
through its frozen installed mapping, validates finite normal f32 coverage,
and supplies a read-only threshold. Berthier consumes it without CAN or mutable
coordinate interpretation. Intent clear preserves the installed profile. A
standalone continuous-law input has only its f64 roundoff floor; that floor is
never applied to a real decoded grid. The independent endpoint/adjacent-level
controls cover both directions, non-unit gears and unsupported numeric ranges.
This does not fix whole Supervisor/model/config installation ownership.

Duration uses the same nominal dt as the planner. Zero-dt composition stays
valid; negative, nonfinite, overflowing and positive zero-rounded durations
reject before mutation. A zero-rounded loop period rejects before Supervisor
construction. Default200Hz/5ms and2seconds are unchanged. This is no claim of
physical1250Hz scheduling or acquisition timing.

## Actual original-source failures and unchanged replays

All1435 original Git blobs in the exact04e4 archive match their object IDs.
Per-run source bindings and raw logs are retained, with forced actual package
compilation and before/after hashing. Bound subset counts1438–1442 are not
described as all archive files. Candidate-v1's entire1449-file binding is
`ada532e44174a24e66a320ff848da2a3c6f23ce1665ffbb7607549fef7d51cdb`.
Each original group executed exactly one intended failing assertion; each
whole-file probe passed unchanged on that candidate. P/F/I counts and timings:

| Group | Original P/F/I | Repaired P/F/I | Body red/green seconds | Full red/green seconds |
|---|---|---|---|---|
| law | 0/1/0 | 1/0/0 | 0.00 / 0.00 | 18.914 / 22.068 |
| controller | 0/1/0 | 1/0/0 | 0.64 / 0.39 | 17.894 / 18.030 |
| matrix | 0/1/0 | 1/0/0 | 0.00 / 0.00 | 17.989 / 20.792 |
| period | 0/1/0 | 1/0/0 | 0.01 / 0.01 | 13.355 / 21.288 |
| inactive-peer | 0/1/0 | 1/0/0 | 0.38 / 0.58 | 19.450 / 17.812 |
| constructor | 0/1/0 | 1/0/0 | 0.05 / 0.04 | 19.950 / 16.573 |

Law/matrix/period use an archive-only wrapper around seven actual unchanged
law modules. Included52/53 or55/56 filtered cases are not counted as executed.
The wrapper is external so the default workspace never runs a duplicate suite.
Controller/peer/constructor probes use the actual existing exported controller
and closed virtual adapter, not a replacement control implementation.

The law case first runs finite coherent moving controls, then500 fixed encoder
samples; original residual velocity is3.207202185380473e-15 and maximum stalled
time is zero. Matrix controls include independent slow motion and settle,
literal bounded triangle peaks, coherent reversal and an explicitly held dq
cache value. Revisited peaks and velocity spikes cannot discard the lifetime.
The1250Hz case distinguishes actual0.8ms from both old0ms and a false1ms clamp.

The actual public controller reaches selected gain-bearing MIT output before
fresh stationary raw frames. It then returns the typed stall and preserves
all15 installed Speed0/neutralMIT/Disable attempts before cleanup. Healthy
feedback, Disable, attempted re-enable and torque commands cannot erase the
fault or emit later MIT. The peer control runs600 selected ticks without peer
Enable/MIT or a false fault. The constructor red uses valid200Hz construction
before the old u32MAX/zero-period failure; generic original rejection and typed
candidate precedence are separate so unrelated errors cannot impersonate it.

| Unchanged whole-file probe | SHA256 |
|---|---|
| law | `fbf89148376524d8e6f41ce35751bdad1bc220d37f3befeebc259dcfa5fcaa6c` |
| matrix | `3878864da5ffc71075d16af64ed3419b0c454d654e17ad8b9da1c1b316df1023` |
| period | `4bfb16994324c378fa70736f27659ffd0bdb869d5df03526117a7ddfc635ef63` |
| controller | `01220de43da0341a5f4e70fde194d75203222c3b2ddd7b4e61d38468ee8859fb` |
| inactive-peer | `f974305a88d94b707784ffe52dfa5da72639af4e8c02b944f21cf15a5a30b803` |
| constructor | `a40ccd91f85e1acf44aa84b196abb645b7ca608f193feba1a1c40142b76b8b77` |
| crawl preservation | `cdf39dc03a668246eb1c077e5f167d646daca2b91c26ebb052a177d3de287bd7` |

The raw one-count staircase is preservation, **not an original red**. Original
and repaired owners pass1/0/0, body1.64/1.70s, full22.775/18.759s. After100 unresolved
real owner ticks, six literal adjacent levels each last300ticks, totaling9s;
no planner echo is used. Host-time receive derivatives do not establish physical
slow-speed motion.

## Numeric conformance and test sensitivity

Candidate-v2 adds two law, two actual receive/grid and one typed constructor
conformance case:5/0/0, full43.291s. Its1452-file manifest is
`99a7ff645c30c5c82d42e111c97f7cb525538c003cf568fc17cc4c06578c31cf`.
New-interface cases are not missing-API original failures. Numeric controls
exercise tiny installed one-count motion across fresh/clear/rearm owners,
invalid dt with unchanged state, zero composition, subsequent real progress
and stationary exhaustion. Metadata controls use literal raw endpoints and
actual receive conversion, unknown-joint refusal, frozen mapping despite
public config mutation and reachable unsupported overflow/subnormal profiles.

Two isolated production mutants, with the same numeric probe, each execute an
intended0/1/0 behavioral failure: forcing a continuous absolute floor onto the
tiny grid (31.559s) and dropping the installed profile during clear (27.199s).
Their bindings/logs are preserved as sensitivity proof, not original defects.

Strict Clippy rejected an over-specified test literal before any test ran.
Canonical spelling preserves the exact IEEE64 value `3b128b95ea782d31`
(`0x1.28b95ea782d31p-78`); the final numeric leaf SHA is
`5cbc03d6a199d2cf7e8bc59265dfd24342ce3ef36d0d1154d13bf30ad475073c`.
Prior executed conformance/mutant bytes remain frozen. Primary and affected
gates execute the canonical final leaf. Production runtime bodies are unchanged
after replay; only cfg(test) registration and equivalent numeric spelling follow.
The non-test hold body SHA is
`4c1455a130743d16b7b91154d7abcddd1837e595e30bb44a84df7eda90e15196`.

## Required gates, reviews and source binding

Primary:715Rust/0failed/1existingignored,355frontend,72PiMCP, strict lint/build,
deny/audit and fatal aarch64 release;107.799s. Affected Linux feature gate:
strict Clippy and432/0/0 tests;24.541s. Local kernel lacks virtual CAN;
implementation-head GitHub CI executes73driver/0ignored and5simulation tests,
including actual virtual CAN. All five jobs pass.
Allowed unmaintained dependencies remain recorded broader work.

Executed1042-input manifest SHA:
`a9a3b0574b7352df21608bc92ebef0e47f7d4c490af1937ea2668bc9fb0186e3`.
Reviewed1042-input manifest SHA:
`9e0fbe779c0525182ad6f297abc8d5be83926bf2d54f4937f23adabbc102e9cb`. Original gate bindings are preserved.
Exactly one docs-only post-gate delta updates obsolete rust-patterns§7:
`3596573df67c9545792b366d76e952cb3112e131332122af6086786346fe4cd9`
to `79d2050a0578686991005d2e4d681ad602d7cc279de7946f4f2e78877b74700c`.
All1041 other inputs, including compiled sources/tests/gates/policies, match.
Independent review verifies that correction; expensive gates are not repeated
for prose. A rejected unexecuted encoding rewrite is retained and corrected
from original UTF-8 bytes, with unrelated guide text preserved.

Standards report SHA `f9d4104c7f2be13a5f1f34e83530fd3b1f157b8dd1bbfddef7b845f8acf74657`;
Spec report SHA `7f8d4a53921f8bcab7e7f71098354a5adab22f04a95c2c4eacb4a2a4b8379f2e`. Each independently hashes all
1042 final entries and execution receipts. Authored test leaves are covered by
the other reviewer. Frozen-probe fixture repetition is accepted for independent
inputs/byte identity; no source blocker remains. Source/gate/bootstrap failures,
metadata annotation correction and unexecuted law-v1 timing are classified
honestly in the ledger rather than counted as behavioral reds.

## Delivery and remaining work

Implementation `eb1359c`/run36755369469 passes all five jobs, with715Rust/1existingignored,355frontend,72PiMCP,5simulation and73actual driver tests.
Final `97881d8aa16a6a4815d6377c90c342d45e352edb`/run36756367932 and
equal-tree merged `7a25bbbca0fad1ef579e979309acba666d4409da`/main36757300231
each pass all five jobs with the same715Rust/1ignored,355frontend,72PiMCP,
5simulation and73actual virtual-driver counts. Fatal main release passes.
Checked/merged tree: `4dc75e94c0d4cdc2618cddb9f12f794cb32f8096`.
All-refs backup SHA256 `a256fffcec5b41363e28b2205cfd3a30d13367101428bcc1dcb903b6931f7f24`
is verified and contains the exact final branch. Only `codex/measured-stall-progress`
at97881d8 was removed locally/remotely after main passed; all other branch
references were preserved. External `batch09/merge-receipt.json` is complete.
PR222's checked merge/backup/cleanup is reconciled in tracked history.

No reference permission, robot operation, raised torque/velocity limits or Wave
sign-off change. Decoded software grids do not qualify physical noise, timing,
plant, drive acknowledgement, stop/support behavior or commissioning. Existing
dropout freshness is a separate verified prerequisite, not a new stall proof.
CS05/06/07 stay partial; bounded target-only reference core R2a is next, followed
by recoverable journal/qualified private grant and installed-owner clients.
The other102-ID dispositions and broader software/model tasks remain tracked;
this batch does not complete the entire remediation.
