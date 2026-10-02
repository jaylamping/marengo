# Batch32: durable virtual reference history software qualified

Preparation against merged main df468496eae0b3c30104874c3bc2b6ad218ec827,
on codex/reference-history-journal at
J:/code/marengo-worktrees/reference-history-journal. The candidate implements
the private R2b1 producer, exact-value codec, concrete SQLite worker, recovery
reader and owner completion consumer described in [ADR0034](../../decisions/0034-durable-virtual-reference-history.md).
Software qualification is complete at b18faf486073b7311a029761792be171dac457ef.
Final PR/main delivery remains pending; no finding closure or physical acceptance
is claimed. See [source qualification](evidence/batch32/final-source-qualification.json).

Batch31 PR245 is delivered: final PR and equal-tree main each pass all five
CI jobs. Independent production and final metadata reviews are clear. Primary
and native Pi software qualification remain bound to dc88d283; native Pi passes
826 Rust tests (one existing ignored), 374 Consul tests and build/asset checks.
See the [delivery receipt](evidence/batch31/delivery-receipt.json).

The next WP04 dependency is a real bounded noncoalescing reference journal and
its owner-consumed durable history completion. R2b1 remains unusable for motion;
R2b2's selected current virtual grant follows. Read the
[preparation plan](evidence/batch32/preparation-plan.md), whose pending-main
sentence is historical and superseded by the delivered receipt. The
architecture ADR governs the journal, codec and lifecycle implementation.
Use complete committed Git archives for Pi staging, including simulation fixtures.

Focused candidate checks pass: changed-crate compilation, clippy with warnings
denied, all Davout/Berthier/Pi tests, and the new real SQL/controller/shutdown
cases. Coverage includes exact scalar widths/float bits and optional URDF fields,
durable readback, eight retained credits and retry after capacity refusal, actual
lock/full/corrupt/schema/session exhaustion, cancellation/deadline/peer hazards,
worker unwind and controlled child death before/after commit. Two child helper tests
do no work outside their explicitly launched child processes. These are candidate
conformance results, not original-binary behavioral reds or hardware acceptance.
Independent review of dc91d08 found two resource bugs: lexical-only history/journal
separation admitted aliases, and SQLite changed an incompatible WAL header before
refusing corrupt history. Separate real resource tests reproduced three behavioral
failures on that candidate (alias admission, worker mutation and inspection mutation).
The repairs resolve resource aliases before construction and reject unsupported
persistent header versions before SQLite opens the file. All four resource cases
now pass, including the distinct-path lazy-opening control. Three additional actual
owner tests cover a completed-unconsumed real write losing to fresh ordered fault
feedback, checked commit-counter exhaustion before admission and foreign-handle
refusal with healthy neighboring work. The original five new probe files and prior
qualified probes are unchanged. Changed-crate clippy remains clean.

Review of 0bebfae found a remaining SQLite sidecar collision. Two separate probes
reproduced admission through a canonical parent alias and actual deletion of an
existing valid calibration YAML when it occupied the rollback slot. Construction
now reserves the database and all three sidecars before any worker installation.
The preceding seven frozen probe files remain unchanged. The provisional 0bebfae
primary run was deliberately stopped before editing its source; it is not a gate
pass and its complete archive is retained. It was not uploaded or run on the Pi.

The subsequent 0dd5a2b review identified existing hard-link overlap. Real pinned
SQLite testing confirmed factory admission through the shared inode; the worker
completed a real second-session write while preserving the legacy YAML bytes.
This is an independence/admission failure, not demonstrated history corruption.
Construction now also compares existing file identity with already locked
same-file 1.0.6 and propagates identity I/O errors except genuine absence. The
eight preceding probe files remain unchanged. The obsolete 0dd5a2b primary was
stopped before edits; its archive was uploaded but never run or installed on Pi.

Review of a949ce4 identified a blocking identity open for named pipes. A bounded
child-process probe reproduced the construction stall and terminated/reaped its
owned child before reporting the behavioral failure. Type metadata now precedes
file identity opens: nonregular history refuses before YAML loading, while
unsupported journal resources preserve lazy worker refusal. The probe checks
all four journal slots and history, including actual worker failure for journal
slots. Its child helper is inert during ordinary test runs. All nine preceding
probe files remain unchanged; no a949ce4 final primary or native Pi run started.

The dc91d08 primary gate and native Pi software run passed (848 Rust tests, one
existing ignored, 374 Consul tests, build/asset checks and all 1764 committed input
hashes unchanged). Those results remain bound to the preceding candidate.
Final source b18faf4 passes the full primary gate and native Pi software run:
861 Rust tests, one existing ignored, 374 Consul tests, build/asset checks and
fatal primary ARM release. All 1769 committed staged inputs are unchanged before
and after native testing. All five source CI jobs pass in run36987033671.
Independent Standards and scoped Spec reviews are clear on this fixed source.

All ten new probe files and sixteen prior qualified files remain unchanged.
Twelve targeted production mutations each compile and fail their exact frozen
behavioral oracle; unmodified before/after runs pass 34 Davout, two Berthier and
one Pi shutdown tests. All 1769 isolated mutation-source files are restored.
The missing-COMMIT test's first result was incorrectly excluded by the runner's
inline-output parser because child output interrupted the test line. Its real
zero-versus-one-row assertion is preserved, and the same unchanged probe passed
mutation qualification on rerun after fixing only result recognition. Compile
and setup failures are never qualifying reds. No test-binary hash was captured.

The 35-test workspace increase includes two inert child helpers; these are not
35 independent hardware behaviors. The 4096 event bound is enforced but not
separately exhausted by a test; the real page-cap test qualifies SQLITE_FULL.
SQLite/process recovery tests establish no SD-card power-loss behavior. Final
documentation-head and equal-tree merged-main CI remain pending.

The owner requested right-arm movement validation and confirmed presence with
the arm resting at gravity home only. Read-only Pi observations show both CAN
interfaces up, installed services active and the cached safety state Disabled.
Legacy Verified joint flags do not qualify repaired current reference. Physical
support and live reference/client workflow remain unestablished. Prioritize the
remaining reference and installed-owner prerequisites, then propose a supported
single-joint direction test with explicit bounds, stop procedure and confirmation.
No motor command or physical CAN operation was issued by this qualification.

All 102 findings and eight maintenance tasks remain: 26 verified, 13 partial,
63 open. Existing CS05/CS06/CS07 statuses are unchanged. G06 lifecycle, T01/T02
activation, T03 generation identity and physical commissioning remain separate.

Installed Pi4bc77ba remains unchanged. No install, sudo, restart, physical CAN
operation or motor movement occurred. Gravity home does not establish support,
repaired current reference, E-stop or recovery. Prompt before every movement
with bounds and stop procedure; require explicit confirmation and commissioning
checks. Ten-minute silence allows independent work only. Preserve safety limits,
Wave sign-off, paused automation, CAD, branches and historical worktrees.
