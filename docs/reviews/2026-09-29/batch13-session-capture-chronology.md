# Batch13: preserve capture chronology

Checked base: `94d3cb48474fbf067e8c5e4bc0317cd6a56a2227`; branch
`codex/session-capture-chronology`. G14 software qualification is verified at implementation
`cabfe945`; final-head and merged-main delivery remain pending.

## Problem and repair

A canonical capture ID ends with a literal `Z`. Parsing it as an offset-bearing
`OffsetDateTime` failed, causing historical import/archive to substitute the
maintenance clock. Newly registered captures also received an invented end time.
These errors distorted date browsing and age retention.

[ADR0028](../../decisions/0028-capture-chronology.md) defines the repair. Store
parses the complete Gregorian capture datetime and explicitly assumes UTC,
including valid leap days and epoch zero. The actual producer's single `profile-`
prefix is supported. Existing registered metadata remains authoritative.
Unknown or invalid new dates return an explicit error before registration or
archive publication; the operator can register authoritative metadata and retry.

Import and archive share one artifact classifier: regular bench/candump `.log`
and position-trace `.csv` files. Exact latest aliases, unsupported extensions,
sidecars, directories and symlinks are excluded. Canonical IDs use their UTC date
bucket; registered arbitrary IDs use `unknown`, without slicing UTF-8 bytes.
Unknown capture ends stay NULL until explicit finalization. Sparse updates preserve
known starts, ends and sibling artifacts. The existing days retention API delegates
to a checked absolute cutoff with the same strict-before deletion semantics.

## Behavioral qualification

Evidence root: `J:/code/marengo-migration-backup-20260929/batch13`.
The tracked ledger binds exact receipt, log, source, probe and runner hashes.

| Whole public test | Original assertion failure | Unchanged repaired replay |
| --- | --- | --- |
| Literal UTC/leap/epoch import, archive, date filter and reopen | `first-original-red-v1`, 0/1/0 | `first-candidate-green-v1`, 1/0/0 |
| Twenty invalid-date fixtures across import and direct archive | `refusal-final-original-red-v2`, 0/1/0 | `refusal-final-candidate-green-v2`, 1/0/0 |
| Profile and authoritative legacy IDs; real artifact eligibility and buckets | `compatibility-final-original-red-v2`, 0/1/0 | `compatibility-final-candidate-green-v2`, 1/0/0 |
| Unknown ends, explicit finalization and repeated import/archive | `unknown-end-original-red-v1`, 0/1/0 | `unknown-end-candidate-green-v1`, 1/0/0 |

Each original snapshot contains all 1,472 actual checked-base Git blobs and only
its whole integration test. Actual manifests/test metadata and local Store/Candump
recompilation bind each execution. Independent literal payloads, public pages,
authoritative metadata, neighbors, reopen and fixture disposal precede each sole
final oracle. All 1,473 snapshot files remain unchanged; fixture inventories are
empty after execution. Git modes are recorded; Windows filesystem executable
permission preservation is not claimed. Independent reviews accepted each initial
public proof. Final-file execution is separately rechecked from raw evidence by root and an
independent audit (SHA256 `c8898bc30682e3874a1239aee5690c5a368bb09b3d04eccd6414526ba33b77ac`).

The new `purge_before` API uses candidate conformance, never a missing-method
original failure. `cutoff-positive-v2` passes 1/0/0. One Store-only mutant changes
both event/session SQL comparisons from `<` to `<=`; all other bytes remain
identical. `cutoff-combined-mutation-red-v2` reaches the sole boundary oracle,
0/1/0, after complete controls, with eight observable differences. The exact
positive source replays 1/0/0 in `cutoff-positive-replay-v2`. This is one combined
mutant, not two isolated failures. Literal captures/events before, exactly at and
after the cutoff verify row/file/FTS preservation, overflow refusal, repeat and
reopen behavior. Calendar values come from an independent UTC calculation.

Three final test files add only the same-line test-only `clippy::panic` allowance.
Their earlier qualified versions remain historical; fresh whole-file final proofs
above establish the new bytes. Preparation reviews confirm that combining Cargo's
output streams preserves the strict control-before-panic classifier. Earlier
missing-pipe, zero-test lint and out-of-order captured mutation attempts remain
unqualified and preserved. No failing receipt is retroactively relabeled.

## Gates and independent review

The exact final source manifest `phase05-source-sha256.json` binds 1,455 inputs,
SHA256 `882e91ae327d3069afb6fafbe95c6e4af4c1f9291cfa8ed9e6725af2c4c8637c`.
Mutable review records are inspected separately. All bound inputs remain unchanged.

- Strict affected formatting/Clippy/tests: 21 passed, 0 failed, 0 ignored.
- Required primary, with fatal main ARM release: 756 Rust passed, 0 failed,
  1 existing ignored; 355 frontend and 72 Pi-tool tests passed. Format, lint,
  proto/build, dependency and ARM cross-build checks completed.
- The unchanged chronology test passes under observed UTC-8 and UTC+14 offsets:
  2 passed, 0 failed, 0 ignored.
- Minimal simulation plus actual smoke: 5 passed, 0 failed, 0 ignored.

The full independent Standards review has no production blocker; its stale tracking
text finding is corrected. The full independent Spec review has no implementation
blocker or scope creep. Separate final lint reconciliations and actual source
bindings preserve both complete reviews. All five implementation-head GitHub jobs pass in run36818692724. Final-head
and merged-main checks plus guarded backup/merge/cleanup remain pending.

Useful retention liveness, structured-query/FTS and candump tests remain. No test
removal, ignore, negative sleep or cold-build speedup is justified by these five
new behavior groups. Test-body time and build/setup time remain separate evidence.

## Scope and remaining dependencies

Known-invalid preflight is not later I/O/SQL rollback or archive crash recovery.
Existing rows lack provenance to identify earlier fabricated timestamps; automatic
historical rewriting is deliberately deferred. Any future repair needs a backup,
preview and preservation of supplied metadata. G15 serialized migrations and G16
bounded reads/archive work remain separate findings. Reference journaling/grants,
media durability and physical acceptance are not established.

All software, CAD and evidence remain under `J:/code`. No robot operations,
limit increases or Wave sign-off changes occur.

## Docker recurrence

The first recurrence reproduced the Inference socket failure and needed reversible
runtime-parent preservation. Its unchanged report retains exact evidence. A later
disappearance left Desktop and its old launcher absent without a recorded initiating
actor. The reviewed hidden WmiPrvSE-brokered startup passed, followed by independent
no-start/direct-pipe health after the wrapper, origin and tool sessions ended.

Measured Desktop PID43040 belongs to no Windows job. It owns a separate Docker
child lifetime job containing its backend/services/frontend and excluding Codex.
The second startup needed no retained launcher, runtime rename, reset, settings
change or persistent installation. Report:
`docker-recurrence/disappearance-20261001-035600/brokered-docker-recovery.md`,
SHA256 `f2d9d7f895635a00603aedb401d5dbb780fab00e3f990104800e11aa7d2b0f13`.
The initiating disappearance and permanent AF_UNIX cause remain unproved;
M08 stays partial. Docker's installed runtime is distinct from project/CAD storage
under the accepted Windows/macOS development decision.

## Implementation delivery checkpoint

[PR227](https://github.com/jaylamping/marengo/pull/227), implementation
`cabfe945e34a70fc1c145383bcc7eeb750fd9256`, passes all five jobs in
[run36818692724](https://github.com/jaylamping/marengo/actions/runs/36818692724).
Raw evidence confirms 756 Rust/0 failed/1 existing ignored, 355 frontend,
72 Pi-tool, 5 simulation and 73 virtual CAN tests with none ignored in the latter
two jobs. The local fatal ARM release is qualified; PR CI's non-main ARM policy
is recorded separately, never presented as a fatal main release. All reviewed and
gated source bytes match the actual committed implementation.

The independent final execution audit accepts all seven fresh proofs, their
complete source inventories and actual metadata, exact combined cutoff mutant
and unchanged positive replay. The tracking reconciliation verifies every older
history object, all 102 dispositions and exact local evidence. G14 now has a
verified software disposition: 16 findings verified, 10 partial, 76 open.

Final documentation CI, guarded all-refs backup, exact checked-tree merge,
postmerge main CI and completed-branch cleanup are still pending. Actual completed
receipts will reconcile this checkpoint; no future result is inferred.

G15's external design and canonical safe-path test proposal remain UNEXECUTED.
The next iteration must use the actual merged G14 base, read the canonical README
and oracle addendum, then choose one vertical slice before freezing/executing.
Earlier unsafe or incomplete drafts are preserved and excluded.

## Completed delivery, reconciled in batch14

The preceding paragraphs describe the frozen implementation checkpoint. Actual
final documentation commit `1d3d85b2a545518830e8f19910b4f7f5f9372b5f` passes all
five jobs in run36820454955. PR227 merged as
`4c1800d4ffb6be13a920a77b6f9b4b47395b0848`, whose tree equals the checked final
tree `6f6ebd5b42eb8a30c2f887a0a03050fdb7053ed6`. Main run36821026729 passes all
five jobs, including fatal ARM release. All-refs backup was verified, and the
completed branch was removed only after checked main with its exact-head lease.

External batch13 `merge-receipt.json` SHA256
`d1ea2173ddea7db52922d21ab39343f5418df3c9619c03d34e048a23340171a0`
binds the delivery; `branch-cleanup.json` SHA256
`7d36317bf7b988dba07f37c733d72dc9b1e669c9e98d13ad71602bf34ec9e2f2`
and verified all-refs bundle SHA256
`7e278670da33eff86f2f56c4cf07906f0fa5efdfc6e9307633471b2e5727e0ee`
record cleanup/recovery evidence. Batch14 uses this actual checked base and a
separately frozen normal-migration slice; earlier design drafts remain excluded.
