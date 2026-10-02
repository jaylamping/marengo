# Right-arm protocol inspection

This is the first dependency for physical reference and motion testing. It does
not make the arm Ready or qualify motion. See [ADR 0036](../decisions/0036-disabled-drive-protocol-inspection.md).

Use the installed five-joint YAML and URDF, including taught limits. Keep the arm
at supported mechanical home, with the physical motor-power E-stop reachable.
Check current telemetry is Disabled before stopping `marengo-pi`. Stop competing
CAN owners; do not run this standalone command beside the control runtime.

```bash
sudo systemctl stop marengo-pi
MARENGO_ROOT=/opt/marengo MARENGO_CONFIG_DIR=/opt/marengo/config RUST_LOG=error \
  /path/to/qualified/motor-repl protocol-inspect > protocol-inspection.json
```

The operation uses the canonical all-address stop sequence, then reads firmware,
MCU identity, run mode, mechanical position/velocity, CAN timeout, zero wrapping
and additive offset. It stops all addresses again on success or failure. It sends
no Enable, Set Zero, configuration changes or nonneutral output. A successful
receipt contains forty actual addressed query/reply pairs. Preserve the receipt
with the source revision and binary hash.

Every accepted version reply must report Reset/Disabled. Missing replies,
parameter errors, conflicting replies, receive failures and peer faults refuse
the inspection. A reply host match is an observed protocol fact, not proof of a
drive boot epoch. MCU bytes remain in wire order; `zero_sta` describes wrapping.

After inspection, verify Disabled telemetry when the qualified main runtime is
restarted. Restarting neither recovers fault authority nor grants reference.
Physical Set Zero and motion still require a qualified owner workflow and a
concrete approved test envelope.
