# bins/marengo-log-cli/src/

## Responsibility
`main.rs` parses and dispatches session, archive, purge (enforces the stored retention settings), legacy/journal import
commands to `marengo-store` (`StoreCommand`; the other subcommands never open the Store). Candump inspection and explicit
`recover-known-v2` dispatch before normal Store open. Recovery prints the library's
completed JSON receipt; schema recognition and backup/publication belong to the library.
`gravity_fit.rs` (`gravity-fit`, no DB) reads `pi_joint_calibrate` / `pi_gravity_calibrate`
session directories as immutable evidence, validates fused sessions have byte-identical
captured Pi URDFs and identical effective calibration windows, groups rows by tick, rejects
timestamp regression, and writes a dated record, patches and accepted proposed URDF only
under the required explicit `--out-dir`. Static method: fits link inertials to holds with
`armee_dynamics::calibration`. Wave method (`gravity_fit/wave.rs`, plan `method: "wave"` or
`--method wave`): up/down bin means of local waves, lumped `A·sin q + B·cos q` per swept
joint (`armee_dynamics::lumped`), gates from the cross-session spread, a minimal-COM URDF
patch and an fc/fv/fs `control.yaml` patch. The required `--repo-urdf` is read (missing
paths are errors) and compared to the Pi URDF. Exit 0 = URDF patch proposed, 2 = refusal,
1 = error; it never applies anything.
`firmware_timing.rs` (`firmware-timing [--json] <candump>...`, no DB) streams `candump -L`
or `-t z|a` captures through `marengo_candump::Candump::visit_path`, classifies each frame
with `robstride::classify_frame` (host 0xFD command vs drive frame) and measures per drive
Enable→Run, post-SetZero silence, identity/parameter/MIT reply latency, report period,
reporting-Off tail and Disable→Reset, plus non-neutral MIT frames and bus density. Every
measurement is defined in the module doc; the committed profile is
`docs/commissioning/firmware/robstride-timing-profile.json`.
