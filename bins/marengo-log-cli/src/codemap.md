# bins/marengo-log-cli/src/

## Responsibility
`main.rs` parses and dispatches session, archive, purge, legacy/journal import and
disk-usage commands to `marengo-store`. Candump inspection and explicit
`recover-known-v2` dispatch before normal Store open. Recovery prints the library's
completed JSON receipt; schema recognition and backup/publication belong to the library.
`gravity_fit.rs` (`gravity-fit`, no DB) reads `pi_gravity_calibrate` session directories,
extracts friction-cancelled steady-state torques from the position trace, fits link
inertials with `armee_dynamics::calibration`, and writes a dated record plus a proposed
URDF inertial patch (exit 0 proposed, 2 refused, 1 error). It never applies anything.
