# PC resume checkpoint — repair loop paused

The owner requested a pause for transfer to their PC. This checkpoint supersedes the historical status sections in HANDOFF.md. Do not restart the loop or Windows automation until the owner resumes it.

The ledger retains all 102 findings and eight maintenance tasks: **24 verified, 11 partial, 67 open**. T21 and T17 remain open; G15 remains partial. No physical acceptance, robot motion, deployment, limits or Wave changes were performed.

## Fetch the saved work

Use the existing Windows checkout `J:\code\marengo`. Fetch origin and check out `codex/daily-audit-production-evidence`. Read this file, HANDOFF.md, implementation-ledger.json and batch27-daily-audit-production-evidence.md before continuing. The implementation stopping point is `788afbffc842625ee2649211027e7e7f0ad3bd28`; the branch also contains this documentation checkpoint. Latest integrated main is `ca13eb810406539cc2394d6130881176fb76665c`.

Use Docker for the full Linux gate (`just check`); the complete workspace cannot be qualified natively on Windows because Chappe uses Unix IPC. Keep the existing host checkout rather than relocating into WSL. Ignored node_modules and virtual environments are not portable artifacts: use Node 24/npm and the research project's locked uv environment (qualified with Python 3.12). Mac Docker was blocked by owner onboarding; do not accept setup terms on their behalf.

## T21 / batch27: implemented, not delivered

The branch contains conservative Rust production scanning with lexical masking, explicit unknown states for unsupported syntax, exclusion of integration tests and file-level cfg(test), semantic Cargo dependency checks, pinned generated-output regeneration and comparison, report confidence states, and a persistent unresolved-finding ledger with explicit bound acceptance receipts. An empty later scan does not close an unresolved finding.

Last observed daily-audit suite: **41 tests passed** at implementation head 788afbf. The stored current-native-tests.txt predates the final three tests and reports 38; do not mistake it for a fresh 41-test receipt. Frozen public probes, original bindings, negative fixtures, actual pinned generation and actual Cargo metadata qualification are retained under evidence/batch27. The generated frozen probe uses a deterministic generator fixture; actual pinned generation is separate evidence. Initial fixture-precondition failures are retained and are not claimed as behavioral reds.

Two independent review findings remain open:

1. Bind production dependency/call findings to an inspected base and added diff. The current scanner can relabel an unchanged reference as newly introduced after an unrelated file edit. Preserve visibility of existing violations while distinguishing new ones.
2. Validate external/ledger schemas and contain expected KeyError/TypeError/AttributeError failures. Malformed input can escape before writing the current report and leave an older same-day clean report available. Prove public main replaces that stale report with a non-clean failure report.

The review's generated-deletion and test-only-source false positives were fixed at 788afbf, with tests passing, but have not received a final independent re-review. Next, when resumed: resolve the two open findings, replay frozen probes and meaningful negative cases, repeat both scoped reviews, run the primary Linux gate, then create/deliver a PR and verify merged-main CI before closing T21. No batch27 PR or hosted primary gate exists yet.

Ledger writes fsync the file and atomically replace it, but do not fsync the parent directory. This establishes ordinary persistence, not power-loss durability. Resolution receipts bind caller-supplied acceptance evidence; their presence alone does not prove a validation command ran.

## T17 / batch19: portable, workflow authorization still pending

Local branch `codex/research-cache-await` is clean at `4643d3baffbf61a951e882059bd3f6afba7e2f01`. Its source and 34 offline tests were accepted; delivery was rejected because the token cannot update .github/workflows without workflow scope. Do not drop the workflow or infer authorization to broaden token permissions.

The exact branch, including its workflow, review documents and evidence, is preserved in [resume/batch19-research-cache-await.bundle](resume/batch19-research-cache-await.bundle). After fetching main on the PC, verify the bundle and fetch its branch into a separate local branch. The bundle requires ancestor `37a4dd1675a22f064a73ab40426e02937ccea194`, already in main history. Reconcile its older documentation with this checkpoint when integrating; T20 later changed the same research area, so inspect conflicts and rerun the full research suite. The bundle preserves work; it does not deliver the workflow or close T17.

Bundle SHA-256: `554d1e0f1e9fa40df11c6b1cab4c19eb8c9bbd2a8206d9af6e57f879be7f364f`.

## Delivered repairs since the earlier handoff

| Batch / finding | PR | Merged main | Exact merged-main CI |
|---|---|---|---|
| 17 / G15 partial | 232 | 7897c33 | 36886085584 |
| 18 / G17 verified | 233 | d03830d | 36888584718 |
| 20 / T18 verified | 234 | a65ae3f | 36889672275 |
| 21 / T19 verified | 235 | 37a4dd1 | 36890121385 |
| 22 / G01 verified | 236 | 0166970 | 36898895683 |
| 23 / G18 verified | 237 | ad09ee4 | 36900877954 |
| 24 / G19 verified | 238 | f831e58 | 36903696586 |
| 25 / T20 verified | 239 | 5c25a8f | 36909443242 |
| 26 / T22 verified | 240 | ca13eb8 | 36938244248 |

These merged-main runs passed all five jobs. Per-batch reports and evidence directories hold the exact heads and receipts. T22's latest main receipt is evidence/batch26/main-ci.json. It classifies scanner errors/unknowns, preserves timeout diagnostics, and checks offline advisory snapshot provenance. The paste unmaintained advisory remains maintenance task M01; scanner qualification does not close it.

## Local preservation and cleanup

Mac worktrees remain intact, including batch19 and batch27. Completed batch22–26 worktrees are also retained; cleanup is deferred. Verified all-ref backups live outside the checkout under `/Users/joseph/.codex/review-backups/`; those Mac paths will not exist automatically on the PC. The tracked T17 bundle and pushed batch27 branch are the portable resume artifacts. Preserve the ledger's historical batch16 Windows checkpoint and the paused automation state.
