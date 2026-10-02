# Disabled bench-home qualification

This command qualifies drive behavior before current-reference acquisition. It
changes hardware encoder zero and volatile CAN timeout settings, but never
enables a drive or grants Ready. See [ADR0037](../decisions/0037-disabled-bench-home-qualification.md).

Require a stopped main runtime, no competing CAN owner, the unchanged/sign-tested
five-joint assembly supported at mechanical home, and the operator beside the
reachable physical motor-power E-stop. Obtain approval for this exact operation
before sending it. Use the installed configuration and URDF, preserving taught
limits and calibration history.

```bash
MARENGO_ROOT=/opt/marengo MARENGO_CONFIG_DIR=/opt/marengo/config RUST_LOG=error \
  /path/to/reviewed/motor-repl bench-home-qualify joseph \
  --confirm-home --sign-attested > home-qualification.json
```

The owned operation obtains fresh firmware/identity/mode reads, restricts the
observed profiles to RS03 0.3.1.42, RS02 0.2.3.34 and RS00 0.0.3.32, then writes
CAN timeout 600 raw counts and verifies each readback. It sends one Set Zero per
joint, using five distinct reply hosts. Each exact status reply must remain
fault-free Reset; each fresh mechanical-position read must be finite and within
the installed zero tolerance (currently 0.05 rad). It rechecks timeout readbacks
after all zeros and ends with another canonical all-address stop. No Enable,
nonneutral control, limit changes or firmware updates are sent.

Save the receipt and passive CAN capture with source revision, executable hash,
installed YAML/URDF hashes and the operator's approval. `grants_reference=false`
is deliberate; old history is preserved and current homing remains Unhomed.

The next physical step is a deliberate motor-power cut/restoration using the
E-stop while the arm stays supported. A new disabled protocol inspection must
show all five timeout values returned to zero before volatility is treated as a
qualified continuity witness. Observe post-reset pose/wrapping as well. If any
timeout remains 600, do not claim reset detection or proceed to motor enable.
Actual timeout duration/output-stop behavior remains a separate qualification.
