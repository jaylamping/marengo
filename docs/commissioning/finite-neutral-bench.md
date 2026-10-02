# Finite physical neutral qualification

The first enabled check is a standalone 500 ms sequence at 200 Hz with all five
MIT commands at zero position, velocity, torque, stiffness and damping. It uses
the installed arm configuration/URDF and Davout's ordinary Ready/Active gates.
See [ADR0038](../decisions/0038-finite-neutral-physical-bench-owner.md).

```bash
MARENGO_ROOT=/opt/marengo MARENGO_CONFIG_DIR=/opt/marengo/config RUST_LOG=error \
  /path/to/reviewed/motor-repl bench-neutral joseph \
  --confirm-home --sign-attested --confirm-neutral-enable > neutral.json
```

Stop the main runtime and competing CAN writers first. The supported unchanged
five-joint assembly must be at mechanical home, workspace clear, and an actual
motor-power E-stop functional. Explicit operator approval applies to this exact
neutral sequence. The operator approved remote execution while the arm was at
home and clear of obstacles during the October 2 session. Leaving power on does
not establish timeout volatility or reset qualification.

The closed owner opens real CAN itself; it cannot accept a supplied bus, history
row, receipt or virtual device epoch. It performs fresh disabled qualification,
checks the observed firmware/MCU profile, durably syncs actual audit in
`/opt/marengo/var/calibration/neutral-bench`, and obtains fresh home/timeout reads
before selecting owner-local reference. Audit files are history, with no grant
import or recovery API. Existing calibration history and taught limits remain.

The five-second permission allows one enable session. Immediate neutral commands
and addressed active home/timeout checks precede ticks. No nonneutral MIT field,
incomplete batch, re-arm or unlimited run is admitted. Ordinary faults, limits,
feedback watchdog and an additional 0.05-rad home-drift guard remain effective.
Every finish/error attempts all-address stop, and reporting stays Off.

Save receipt, passive capture, source/executable/config/URDF hashes and approval.
The stop receipt describes accepted writes, not physical stop acknowledgement.
Process kill and hardware behavior are external observations. This tool cannot
run torque pulses or trajectories; those require a separate finite motion profile.
