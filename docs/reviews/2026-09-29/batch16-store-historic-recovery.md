# Batch16: deliberate known historical recovery

Base: checked PR229 merge `d660112afadd01468c813bf767d7bdc958d0643e`. Software, local CAD,
fixtures and retained evidence remain under `J:/code`. G15 stays **partial**;
the same102IDs remain16verified/11partial/75open and all eight maintenance tasks
are preserved. No hardware acceptance or robot operations occur.

## Change and contract

[ADR0030](../../decisions/0030-deliberate-known-store-recovery.md) assigns
`recover_known_v2(source,backup,output)` to marengo-store. The explicit CLI
`recover-known-v2 --source ... --backup ... --output ...` dispatches before
normal Store open. Normal opening retains the historical refusal.

The recognized profile is a complete trusted v2 schema with stored marker1.
Literal-token/quoted-byte and SQLite object/pragma checks refuse unknown or
partial schema. Read-only source classification and finite online backup share
one pinned read transaction, including committed WAL data. A completed backup
must have integrity, exact ordinary rows and external-content FTS consistency,
close/reopen verification and a hashed standalone rollback-journal image before
publication. Its source marker remains1. Repair changes only a separate output:
promote marker2 inside an Immediate transaction/readback, then call the unchanged
normal owner to reach3. Preserve supplied settings/timestamps and all history;
insert missing defaults once with the migration timestamp.

Full SQLite filename namespaces, canonical parents, same-file identities and
fresh no-overwrite publication protect source/destinations. Owned connections are
validated before/open/work/close, and copies validate actual handles. Cleanup
removes only the explicitly created, identity-checked staging database. Residual
sidecars or unexpected entries are retained/reported. Published flags are set
immediately, so every later failure reports retained artifacts. There is no claim
of atomic hostile-rename-proof SQLite descriptor binding.

Limits are256MiB for source namespace/each image and a30-second **cooperative**
work budget; a SQLite busy wait can take up to5seconds and OS I/O is uncancellable.
No strict wall-clock, power-loss or physical-media durability is established.
Other historical profiles, full statement interruption, competing openers/writers
and publication/crash recovery remain separate G15 qualification.

## Actual independent behavior evidence

The original-public control is exact checked Git source plus one whole literal
probe. A real committed WAL-only row is absent from a main-only copy; historical
Store::open refuses without logical source changes. This is a **positive**
control. No missing API/command or compiler failure is a baseline regression.

The final1354-line whole public probe has SHA256
`303160c9117baeae6def409130164a187f6bc6754160c6766ffb5dcd33f1d52b`. Its five final exact workers pass:
supported backup/output/source/FTS, eight unsupported-schema/marker/FTS cases,
13 namespace/alias/sentinel cases, a real Linux output-creation failure after a
completed backup plus fresh-path retry, and missing-default values/timestamps
through reopen/repeated migrate. Real cleanup precedes each sole collected oracle.
The actual CLI composition test also verifies required arguments, existing-file
refusal, typed JSON receipt, source/neighbor preservation and usable output.
The CLI fixture deliberately uses supported public bootstrap; independent schema
recognition is established by the library fixtures, not that composition test.

The main-only production copy mutant fails at471: the backup loses committed
rows. The FTS rank1-to-rank0 mutant fails at753: externally missing content is
wrongly accepted despite the fixture's independent typed rank1 corruption control.
Both compile and fail only at the named behavioral assertion after controls and
actual cleanup. Byte-identical unmutated source/test replay passes after each.

| Actual completed run | Passed/failed/ignored | Receipt SHA256 prefix |
|---|---|---|
| original-recovery-control-v1 | 1/0/0 positive | `7441dfa40612bbec` |
| final-happy-v1 | 1/0/0 positive | `fbd2d9696e30c206` |
| final-refusal-v1 | 1/0/0 positive | `92f4d9d1e548accd` |
| final-paths-v1 | 1/0/0 positive | `72eafaf1fe6e9280` |
| final-retry-v1 | 1/0/0 positive | `27348675d4912fbf` |
| final-defaults-v1 | 1/0/0 positive | `12a103913ca8b803` |
| mutant-wal-v1 | 0/1/0 behavioral red | `f2f0c8365d52abdf` |
| mutant-fts-v1 | 0/1/0 behavioral red | `25da054d5c9a8826` |
| replay-wal-v1 | 1/0/0 positive | `ec9c6e8a39e4b723` |
| replay-fts-v1 | 1/0/0 positive | `c549f3c856631d9f` |

Full hashes/configs/bindings/logs/metadata/executable/mutation deltas are retained
in `J:/code/marengo-migration-backup-20260929/batch16/local-qualification.json`
(SHA256 `2f5cceb2362a29d83c08ddec31eb1742a49c6fcd6be1ffdf8b3e92aeed1d48c7`). Original Git archive has1486
verified framed blobs; control1487files; candidate1491files. Immutable v5 helpers
use the pinned image/network-none, root-serialized Cargo, actual local-package
recompilation/direct exact worker, bounded waits and empty fixture inventories.
Same-source later workers reuse an explicitly qualified target; no cold-build
claim is made. Preliminary slices remain historical evidence, not final whole-file
qualification. Root's wrong-line phase04v1 is preserved and explicitly excluded;
corrected v2 and the final1190 retry worker supply accepted evidence.

## Gates and test quality

All1464core inputs remain byte-identical before/after: strict affected36/0/0,
native Windows34/0/0, required primary771Rust/0failed/
1existingignored, 355frontend and72PiMCP, including fatal
main-mode ARM release. Native excludes two platform-specific Linux cases;
full native Windows IPC and macOS are not qualified. Primary elapsed
133.39s versus affected28.52s and native24.62s is observed
attempt timing, not comparative performance. Useful existing retention, query,
FTS, candump and migration tests remain; no existing test deletion or new ignore.
Six new behavior tests are justified by preservation/refusal/failure contracts,
not implementation shape. No redundant broad rerun or local sim/vcan run is added
for this Store-only change; exact GitHub sim/vcan jobs remain delivery gates.

## Review and delivery checkpoint

Independent final Standards/Spec and execution audit, exact final-head all-five-job
GitHub CI, merge/equal-tree main CI, all-refs Git backup and scoped branch cleanup
remain later receipts at this frozen tracked checkpoint. Earlier preflight is not
final acceptance. Batch15 completed PR229/main delivery is reconciled in the ledger
and prior report. This batch must not merge from the local checkpoint alone.
Next: finish delivery, then deterministic G15 interruption/refusal/concurrency.
Hardware issues170/176 and Wave sign-off remain unchanged.

## Late CLI review correction and final local qualification

Initial Standards accepted checkpoint2b0 with one optional fixture-duplication
suggestion. Initial Spec found P2: after library success, receipt serialization or
stdout write failure omitted both completed artifact paths; the final newline could
panic. The CLI now catches serialization, writes, newline and flush errors, reports
both retained canonical paths and includes escaped path representations. Pinned
serde's non-UTF8 path rejection is source-established; execution is not claimed.

The actual Linux /dev/full probe first establishes ENOSPC28, source/history/FTS/
neighbor preservation, two standalone artifacts at markers1/3 and real cleanup.
On implemented checkpoint2b0 its sole assertion415 fails with preserved=true and
reported=false; the identical417-line whole probe passes after only CLI main.rs
changes. Probe SHA256 `e08e69b8dd2f9e23e21e2d1132cab84e99b96fe4c0348bf1f0998d94a62612de`;
red receipt `4c6b53e0d452d42b116d84987ca8251dd9af51c917c6f43115eeb3a8e4ddafda`;
green receipt `ac5e887cd883444b14f6569fc77783cc0cf19045b5663d3b5f697aca9b4a8cd4`.
Fresh local compiler artifacts and direct exact workers, bounded waits, source/
executable hashes and empty fixture inventories qualify both runs. Green rebinds
all actual red predecessor evidence before and after. This is an introduced-command
behavior regression on2b0, not a missing API red on the initiald660 baseline.

V3's image startup attempted mkdir in the read-only source mount and failed before
compilation. Its unqualified receipt is preserved; it is never behavioral evidence
or a predecessor. Preparation v1/v2 and one quoting failure are excluded. Immutable
v4 changes the launch to invoke python directly. Phase05 library production/probe/
dependencies remain byte-identical; the only two changed core inputs are CLI main.rs
and the appended CLI probe. Earlier phase05 whole-candidate gates are historical,
not a claim about the final CLI. All1464 final phase06 inputs remain unchanged.

Final actual gates: affected37/0/0; native Windows34/0/0 (three Linux-only cases
excluded); required primary772Rust/0failed/1existingignored,355frontend/72PiMCP
and fatal ARM release. Seven new behavior tests retain all useful existing tests;
no new ignore or performance claim. Final receipt/bridge:
`J:/code/marengo-migration-backup-20260929/batch16/final-local-qualification.json`
SHA256 `03822ac5967dfda284fd6f77da4b6e57ff2ce211a5591b50e9d140bf61cf4298`. Independent final review
and exact-head GitHub delivery remain later receipts before merge. G15 remains
partial; all102IDs/eight maintenance tasks and hardware restrictions remain.

## Independent final review

Standards accepted complete19-file diff d660112...3a7ee096 and all1464 final
core bindings with no hard violation or blocker. One optional fixture-duplication
suggestion is deferred: shared fixture helpers can be revisited with unchanged
oracles and requalification. Spec accepted the full contract after the CLI P2
correction; its initial withholding remains recorded separately. The test author
disclosed ownership; root independently executed production red/green and gates.
The independent execution audit accepted the phase05 library chain and the
separate final CLI red/green proof. No acceptance infers remote CI or hardware.

Exact final review binding and separate reports are retained externally under
batch16. This subsequent reconciliation changes review documents only; all1464
checked core inputs remain unchanged. Exact final-head all-five-job GitHub CI,
equal-tree merged-main/fatal-ARM CI, verified all-refs Git backup and scoped branch
cleanup remain required delivery receipts. The immutable external merge receipt
will reconcile this frozen tracked checkpoint on the next iteration.
