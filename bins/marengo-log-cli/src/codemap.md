# bins/marengo-log-cli/src/

## Responsibility
`main.rs` parses and dispatches session, archive, purge (enforces the stored retention settings), legacy/journal import and
disk-usage commands to `marengo-store`. Candump inspection and explicit
`recover-known-v2` dispatch before normal Store open. Recovery prints the library's
completed JSON receipt; schema recognition and backup/publication belong to the library.
`gravity_fit.rs` (`gravity-fit`, no DB) reads `pi_gravity_calibrate` session directories,
extracts friction-cancelled steady-state torques from the position trace, fits link
inertials with `armee_dynamics::calibration`, and writes a dated record plus a proposed
URDF inertial patch (exit 0 proposed, 2 refused, 1 error). It never applies anything.
`firmware_timing.rs` (`firmware-timing [--json] <candump>...`, no DB) streams `candump -L`
or `-t z|a` captures through `marengo_candump::Candump::visit_path`, classifies each frame
with `robstride::classify_frame` (host 0xFD command vs drive frame) and measures per drive
Enable→Run, post-SetZero silence, identity/parameter/MIT reply latency, report period,
reporting-Off tail and Disable→Reset, plus non-neutral MIT frames and bus density. Every
measurement is defined in the module doc; the committed profile is
`docs/commissioning/firmware/robstride-timing-profile.json`.
