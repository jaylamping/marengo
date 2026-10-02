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
physical CAN transport or motor movement is performed. Final primary, broad
Pi workspace, independent review and hosted delivery remain pending.
