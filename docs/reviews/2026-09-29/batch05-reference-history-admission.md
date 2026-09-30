# Batch05: historical calibration and current reference

Baseline: merged main `6ff2ed82191d9e602aa230cedb80ae10259cce71` (PR217).
Branch: `codex/reference-history-admission`. Active checkout/CAD:
`J:\code\marengo`. Design:
[ADR0022](../../decisions/0022-calibration-history-and-current-reference.md).

Status: **R1a software slice locally verified; exact-head GitHub delivery checks and safe merge pending**. This report is
evidence of the bounded startup slice, not complete CS05 reference authority or
a physical commissioning procedure. The full 101 finding IDs and eight
maintenance tasks remain in the [ledger](implementation-ledger.json).

## Observable contract

Loading calibration history cannot grant current-process `Verified` state. Every
configured joint starts `Unhomed`, even when all recorded fields appear to match.
History remains inspectable and construction does not rewrite bytes. Only a
missing resource means empty history; other read failures and YAML parse errors
must be visible before Supervisor startup reporting transmits CAN traffic.

The homing library binds a deterministic supplied path. Supervisor composition
preserves the legacy environment override and provides an explicit-path entry
point using the same config/policy initialization. A relative override retains
its old process-working-directory meaning. Read errors do not fall back to an
empty record, and an explicit path wins over environment selection.

CLI help/refusal and Pi startup messages no longer recommend the former
fresh-process home/Enable recipe. Their command ownership remains unchanged.

## Baseline evidence

Before active production edits, replayed the existing-public tests in the exact
6ff archive with:

```text
cargo test --locked --no-fail-fast -p marengo-homing -p davout --test reference_history_baseline -- --nocapture
```

The command compiled and reached four behavioral assertion failures:

| Group | Independent expected behavior | Actual unchanged baseline |
|---|---|---|
| Apparently matching historical row | Retained history; Unhomed and not ready | Verified; `require_ready` succeeds |
| Five separate mismatches | Wrong device/interface/sign/position/revision cannot grant readiness | Every case Verified and ready |
| Malformed existing YAML | Construction returns a parse error and preserves bytes | Construction succeeds with empty history |
| Actual Supervisor, five historical rows | Checked home and normal Enable refuse; no Enable traffic | Home/Enable succeed, mode Active, five literal Enable IDs `0x0300fd01` through `0x0300fd05` |

The missing-file control passes: empty history, Unhomed, refusal and no file
creation. History byte-preservation assertions pass before the readiness
failure. No synthetic grant, unchecked home or missing-feedback failure is used
to manufacture the Supervisor regression. Each mismatch child executes before
the aggregate assertion; one early failure cannot hide later cases.

These are four groups, not nine distinct defects. Eight child behavioral
failures and one positive child execute. The two ignored parent dispatch workers
are explicitly invoked in children. Compilation/API absence is not a red result.
All 56 archived relevant production/config/model/proto/doc files match the exact
6ff Git blobs and retain their SHA256 after replay. Native compile 3.22s, whole
command 3.75s; crate test bodies 0.04s and 0.10s. No cross-host speedup is claimed.

Exact source, hashes, logs and fixtures:
`J:\code\marengo-migration-backup-20260929\batch05\reference-baseline-preparation`.
The archive is baseline evidence, not an active development checkout.

## Test replacement and qualification

The original two baseline test sources and support source pass byte-for-byte
unchanged against a separate candidate snapshot. All four previously failing
groups and missing/history-preservation controls pass; all nine children execute.
Only frozen homing/Davout source is overlaid onto the exact baseline archive;
56 candidate files remain unchanged during execution and all 56 original
baseline production hashes remain preserved. See
`batch05/existing-public-green/README.md` under the evidence root. This replay
uses the old public APIs and is independent of the new explicit-path API tests.

Active candidate integration tests add eight registry and four Supervisor parent
contracts. Registry tests cover exact/mismatched history, typed parse/IO failures,
missing resources, explicit binding, real persistence followed by reconstruction
refusal, and library environment independence. Supervisor tests cover historical
home/normal Enable refusal, missing history, zero startup TX on corrupt/directory
errors, and five child path-precedence cases. Child-only environment tests protect
absolute/relative explicit and legacy paths plus configured root-relative default.
No parent environment mutation or ignored/empty dispatch worker is added.

Two PID-sharing registry tests are replaced by missing-history and real
persist/reconstruct public contracts. The legacy verifier tests retain their
assertions with uniquely owned resources. One Davout global `set_var` test is
replaced by the explicit missing/history admission tests. Review also removed an
intermediate empty dispatch worker, preserving all five real child cases without
inflating the test inventory. No Cargo dependency or lock change is needed.

| Check | Result and scope |
|---|---|
| Required primary | Pass: 639 Rust, one existing ignored placeholder; 355 frontend; 72 Pi-MCP; eight vcan-setup and four dependency-gate contracts; strict lint/fmt/proto/build/deny/audit/index coverage; fatal main aarch64 release smoke |
| Affected Linux feature | Pass: 338 homing/Davout/Berthier tests, zero ignored; strict all-targets clippy with SocketCAN compiled |
| Native affected | Homing 29 tests and strict clippy pass. Davout's intermediate full 137 passed, then the dispatcher-only replacement passes four focused tests/strict clippy; final Linux integrated inventory is 136 Davout tests |
| Simulation | Pass: five tests and minimal.xml engine smoke (`nq=2`, `nv=2`); production plant qualification remains open |
| Independent review | Frozen code/contracts/runbook reviewed; no introduced blocker for the bounded R1a scope; exact receipt under batch05 evidence |
| Exact-head GitHub | Pending; all applicable jobs must pass before safe merge |

Primary 98.79s, affected Linux 10.66s, simulation 2.71s. The original-test candidate
replay compiles in 6.85s and runs both test blocks in 0.11s (7.35s command wall
time). Compilation and test execution are distinct; no speedup is inferred from
different caches/hosts. One initial container launcher reset PATH before Cargo
could start; its log is preserved separately and does not count as a behavioral
test failure. The corrected non-login launcher passes. Primary keeps the known
Consul Router advisories (T31) and allowed unmaintained paste (M01) explicit;
software completion of those items is not claimed.

## Remaining scope and physical acceptance

R1a does not make `record_verification`, public state setters, synthetic bench
grants, mutable registry access, unchecked Ready or direct scoped Enable safe
production authority. Cached Set Zero, target-only calibration, stop-before-disk,
immutable/recoverable persistence, owner/device/config/model identity, correlated
client receipts and taught-reference invalidation remain follow-up findings.
CS05 is **partial**: the startup software slice is verified locally and awaits
exact-head delivery checks; the remaining reference authority is still open.

Separate CLI Set Zero/home/Pi processes cannot recover readiness from saved
history. That former procedure is withdrawn in [homing.md](../../homing.md).
Fresh `motor-repl disable` constructs the full Supervisor before dispatch, so
corrupt history adds a concrete stop-dispatch failure trigger. The already
running Pi owner's Disable and `Supervisor::disable_all` do not reload history.
The required reference-independent stop and installed-owner caller cutover remain
CS07/CS09/T05/T12; no competing raw-CAN fallback is introduced. The fresh CLI is
not certified as emergency stop. Physical E-stop and arm support remain separate
acceptance requirements.

No physical robot connection, Enable, motion, flashing or deployment is part of
this batch. No torque/velocity ceiling or Wave sign-off change is authorized.
