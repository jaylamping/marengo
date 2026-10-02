# Batch32: durable virtual reference history under qualification

Preparation against merged main df468496eae0b3c30104874c3bc2b6ad218ec827,
on codex/reference-history-journal at
J:/code/marengo-worktrees/reference-history-journal. The candidate implements
the private R2b1 producer, exact-value codec, concrete SQLite worker, recovery
reader and owner completion consumer described in [ADR0034](../../decisions/0034-durable-virtual-reference-history.md).
It remains under qualification; no finding closure or physical acceptance is claimed.

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
worker unwind and controlled child death before/after commit. One helper test
does no work outside its explicitly launched child process. These are candidate
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

The dc91d08 primary gate and native Pi software run passed (848 Rust tests, one
existing ignored, 374 Consul tests, build/asset checks and all 1764 committed input
hashes unchanged). Those results remain bound to the preceding candidate.
Frozen mutants, independent review of the repairs, the final primary
gate, exact PR/main CI and final native Pi qualification remain pending. SSH connectivity was
rechecked successfully; the Pi reports aarch64.

All 102 findings and eight maintenance tasks remain: 26 verified, 13 partial,
63 open. Existing CS05/CS06/CS07 statuses are unchanged. G06 lifecycle, T01/T02
activation, T03 generation identity and physical commissioning remain separate.

Installed Pi4bc77ba remains unchanged. No install, sudo, restart, physical CAN
operation or motor movement occurred. Gravity home does not establish support,
repaired current reference, E-stop or recovery. Prompt before every movement
with bounds and stop procedure; require explicit confirmation and commissioning
checks. Ten-minute silence allows independent work only. Preserve safety limits,
Wave sign-off, paused automation, CAD, branches and historical worktrees.
