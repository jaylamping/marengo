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
sentence is historical and superseded by the delivered receipt. Write the
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
Frozen probes, meaningful mutants, independent review, the primary gate, exact
PR/main CI and native Pi qualification remain pending. SSH connectivity was
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
