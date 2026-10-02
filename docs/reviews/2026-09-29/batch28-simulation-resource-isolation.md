# Batch28 — isolate simulation resources from installed Pi configuration

Baseline: `ca13eb810406539cc2394d6130881176fb76665c`. Branch:
`codex/simulation-config-isolation`. This is a newly observed platform-validation
defect related to T26/test quality; the original102 finding IDs and8 maintenance
tasks remain intact. No original control finding is reopened solely on this test
setup failure, and T26 remains partial.

The Pi workspace stopped at cs24_constructor_period because its positive control
emitted diagnostics despite disabling them in its copied fixture. A targeted
repeat reproduced it. The configuration resolver preferred /opt/marengo/config
over the constructor's supplied root. A separate child-process regression with
a competing configuration directory fails eight valid/invalid constructor cases
at unchanged Rust baseline source. The ordinary runtime constructor passes its
positive precedence control. Frozen v1 and the original constructor test replay
green after the resource selection repair.

Both specialized Supervisor and ControlLoop simulation constructor variants now
select the supplied root's config directory and share the same initializer,
validation, dynamics, admission, stop and output implementations. Ordinary
runtime configuration precedence is retained. ADR0031 records the choice.
The final test adds a narrowly scoped clippy allowance for an impossible harness
selector; the intermediate lifetime compile and clippy preparation failures are
preserved in the recovery directory and excluded from behavioral evidence.

The broader isolated Pi workspace uses MARENGO_CONFIG_DIR/MARENGO_ROOT pinned to
its staged source tree. This avoids installed configuration for ordinary tests.
Two numeric-grid cases were also assuming an ordinary constructor selected
their copied gear/direction. Their setup now uses the closed Unreferenced
constructor; all independent numeric, ordering, no-motion and Unhomed assertions
remain. The ordinary unsupported-reference test separately rejects any startup
traffic except diagnostics, clears that initialization trace, and retains every
preflight write and all no-grant/no-stamp-consumption oracles. It still invokes
the ordinary constructor and cannot receive virtual acquisition capability.

The Pi source is staged only under
`/home/joey/marengo-validation/batch27-20261001/stack-3cde431`; its historical name
identifies the original staging receipt, not the final candidate. Source hashes
bind the overlaid repair. No runtime/configuration deployment, service restart,
physical CAN transport or motor movement is performed. Final combined Linux
primary passes814 Rust/1existing ignored, Consul361, Pi MCP72, research83,
daily-audit15 and the fatal aarch64 release build. Pi workspace passes814
Rust/1ignored; its six final source/test hashes match the canonical inputs.
Both independent reviews are clear at8bcd9a. PR242 merged at
`4b0d287004260c5286c1a240e300c796a6a9ccfa`; exact PR-head CI36958751468 and
merged-main CI36959437295 passed all five jobs. Delivery is complete. T26 remains
partial and no original finding ID is closed by this platform observation.

A Windows worktree needs a Linux .git pointer overlay in Docker. An initial
GIT_DIR/GIT_WORK_TREE environment workaround leaked into daily-audit temporary
Git repositories and failed preparation. The corrected container mapping mounts
only read-only metadata and does not export those variables. That setup failure
is preserved/excluded from product behavior evidence.
