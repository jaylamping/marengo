# ADR 0022: Calibration history and current reference

Status: **Accepted for implementation**. September 30, 2026. The first slice is
startup history admission (R1a); it does not complete the reference transactions
in [ADR 0019](0019-runtime-authority-and-verification.md).

## Problem

`HomingRegistry::new` treats any historical row with a configured joint name as
current `Verified` state. A motor swap, changed interface, failed sign test,
out-of-tolerance position or old config revision therefore still permits checked
home and normal Enable in a fresh Supervisor. Even an apparently matching row
does not establish the current physical reference. The constructor also hides
malformed and unreadable resources through `is_file` and discarded load errors.

## Decision

Every newly constructed registry starts all configured joints `Unhomed`.
Historical rows remain available through `calibration()` and construction never
rewrites the resource. An absent resource means empty history; every other read
error and YAML parse error is returned. Read directly and classify `NotFound`
rather than prechecking metadata and treating every failure as absence. Expose
the existing typed `RegistryError` through the crate's public API.

The homing library takes an explicit resource binding. Its existing `new`
constructor deterministically uses `repo_root.join(record_rel_path)`; an explicit
`with_record_path` constructor accepts a path as supplied. Environment selection
belongs to Supervisor composition: existing `from_repo` continues to honor
`MARENGO_CALIBRATION_RECORD`, using `var_os` to preserve OS paths, and its default
remains the configured path under the repository root. An override retains its
existing path interpretation (a relative override is relative to the process
working directory). `from_repo_with_calibration_record_path` uses the supplied
path, regardless of that environment variable. Both Supervisor entry points use
one implementation of policy validation and initialization. History errors
return before startup reporting transmits any CAN frames.

These constructors bind a resource; they do not grant reference authority. R1a
adds no successful calibration or persistent recovery path. Existing public
state setters, synthetic bench grants, mutable registry access, unchecked Ready
and direct scoped Enable remain explicit follow-up work under CS05/CS06/CS15.

## Verification

Before changing production, replay the four existing-public behavioral groups
against exact merged main: apparently matching history, all five independent
mismatch cases, corrupt YAML, and actual Supervisor home/Enable admission. Keep
the missing-file and byte-preservation controls. Record actual source hashes,
assertion failures and exact commit identity. A missing proposed API or compiler
error is not a behavioral regression.

Replace shared PID filenames and parent-process environment mutations with
independent exclusively created test directories and explicit bindings. Where
legacy environment precedence itself is the contract, set or remove variables
only in a child process. Public Supervisor tests distinguish startup diagnostic
traffic from Enable/motion traffic, and observe a shared recording bus after
constructor failure. Add typed-error and explicit-binding conformance tests;
keep their new-API scope distinct from the unchanged baseline regressions.

## Consequences and remaining work

A separate `motor-repl set-zero` process no longer conveys readiness to a later
`home`, `enable` or Pi process through a YAML row. This is an intentional refusal
until a qualified current-reference or separately accepted persistent recovery
transaction exists. Existing history is preserved for inspection. Operator docs
must not present the former multi-process procedure as a working commissioning
path, or recommend unchecked grants as a workaround.

*Update 2026-10-03: `motor-repl disable` no longer constructs a Supervisor; it
reads only the drive addresses from `motors.yaml` and sends one Disable per
drive (see docs/safety.md, Reference and stop callers). The paragraph below
describes the earlier behavior.*

The current motor-repl constructs a full Supervisor before dispatching `disable`.
Consequently a corrupt history resource can prevent that fresh process from
reaching stop, as other config/model construction failures already can. This is
part of the unresolved stop/owner cutover: the authoritative stop must not depend
on reference loading or create a competing CAN owner. R1a does not certify that
CLI as an emergency-stop mechanism; review this effect before delivery and keep
the existing physical stop procedure explicit. The already running Pi owner's
Chappe/stdin Disable and `Supervisor::disable_all` do not reload history or
require Verified, so this constructor change does not inhibit those stop paths.

Later slices must remove naked production `Verified` grants, bind a private
reference receipt to owner/boot/device/config/model, preflight target-only Set
Zero, qualify postcommand protocol evidence, and stop before persistence. Host
dequeue time or cached feedback does not prove command causality. Unsupported
firmware/reference capabilities must refuse success. Migrate Pi, gateway and
clients through correlated request/receipt contracts before admitting recovery.
No software test here establishes physical calibration, Hall wiring, accepted
firmware evidence or a stopped drive; hardware acceptance remains separate.

## Amendment (WP-T, 2026-10-03 — D-3)

The startup history admission decided here is retired. Supervisors no longer
read any history resource: `HomingRegistry::with_record_path`,
`load_calibration`, the `MARENGO_CALIBRATION_RECORD` override in Davout
composition, and the `from_repo_with_calibration_record_path` /
`from_simulation_with_calibration_record_path` /
`from_repo_with_physical_reference_and_record_path` constructors are deleted,
along with the `CalibrationRecord` history type and its writer. Corrupt,
directory-shaped or missing legacy files can no longer fail construction
(`crates/davout/tests/reference_history.rs` pins the new contract), and no
constructor creates a history resource. The "preserved for inspection" rows
above are now inert bytes: Davout never inspects them either. The reserved
history location (`homing.yaml calibration_record_path`) is still computed so
the reference journal keeps a distinct path beside it, and it stays in the
reference policy binding. The remaining R1a consequences (private receipt,
target-only SetZero, stop before persistence) stand and are owned by ADR 0036.
