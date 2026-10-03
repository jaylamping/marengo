# Intent card — `motor-repl`

## 1. Header

| Field | Value |
|---|---|
| Crate | `motor-repl` |
| Path | `bins/motor-repl` |
| Kind | bin (`src/main.rs`); features `socketcan` (→ `robstride/socketcan`), `vcan` (→ `socketcan`) — `Cargo.toml:13-16` |
| Baseline | `a2b55b3` |
| LOC | src 465 (`main.rs`); tests 0 |
| Sources | `bins/AGENTS.md`, `bins/codemap.md`, `bins/motor-repl/codemap.md`, `src/codemap.md`, `codemap.md:12`, `AGENTS.md:224`, ADR 0006/0022/0023/0036, `docs/homing.md:99-170`, `docs/safety.md:225-237`, `docs/pi-commissioning.md`, `scripts/homing-preflight.sh`, `scripts/install-pi.sh:375-391`, `scripts/profile-pi-loop.sh:28`, `tools/marengo-pi-mcp/src/{tools/motion.ts,tools/readonly.ts,homing-preflight.ts,gravity-gate.ts,harness/index.ts}`, prior `control.md` CS07/CS20, `tooling.md` T05/T07/T30, ledger, `git log -- bins/motor-repl` (20 commits; first `61fe36d` 2026-05-19; turns `fc07165` verified homing, `8c3f62e` history≠readiness, `7951f10` private reference admission, `e1a1771` physical owner for set-zero) |

## 2. Intent

Originally a one-shot **bench bring-up CLI** that exercised motors through Davout over SocketCAN without Motor Studio (`main.rs:1`, "all motion goes through Davout"; `bins/AGENTS.md` "motor-repl uses SocketCAN only"; commit `84718fc` "Dual-shoulder CAN bring-up … motor-repl"). Each invocation constructs a fresh `ControlLoop`/`Supervisor`, runs one subcommand, and exits (`main.rs:127-465`); it is not a REPL (prior review `control.md:17`, `2026-09-29-repository-review.md:93`). Since ADR 0022/0023/0036 a fresh process starts Unhomed and **current reference grants are private to the owning process**, so motion subcommands can no longer be admitted here; ADR 0036 §"Process lifetime" and the usage text (`main.rs:85-89`) redirect home+enable to one `marengo-pi`. What remains intended: `set-zero` (qualified physical reference + journal row; grant dies with the process, `main.rs:383-403`), `disable` (fresh-process stop/fault clear used by MCP; `docs/safety.md:233`), read-only `status`/`homing-status`/`gravity-preview` used by MCP/scripts.

Conflicting statements: `bins/AGENTS.md` "Interactive motor exercise: status/enable/jog/set-zero/gravity" and codemap "interactive and one-shot … hold-on, hold-at" vs. code (one-shot, no hold commands, enable/jog unadmittable). Cargo description "Interactive motor exercise REPL" (`Cargo.toml:4`).

## 3. Owns / Must not

| Owns | Must not |
|---|---|
| Argument parsing and one Davout call per run (`main.rs:95-465`) | Motion outside Davout (`main.rs:1`; `bins/AGENTS.md` anti-pattern "Direct CAN access from bins") — upheld; all frames via Supervisor |
| Choosing ordinary vs physical-reference owner: only `set-zero` builds `from_repo_with_physical_reference` so a journal fault cannot block `disable` (`main.rs:189-213`) | Be a qualified emergency stop: full constructor runs before dispatch, so corrupt config/history blocks `disable` (`bins/motor-repl/codemap.md`; `docs/safety.md:233-236`) |
| Gravity-preview model evaluation (`main.rs:432-459`) | Co-own CAN with `marengo-pi` (MCP `canOwnerBranch`/`unlessCanOwned` gate it, `homing-preflight.ts:11-16`) — enforced only by callers, not by the bin |
| — | "Logic in bins": `preflight_gravity_saturation` duplicated with marengo-pi (`main.rs:23-66` vs `marengo-pi/src/main.rs:808-855`) |

## 4. Interface

Global args: `--can-interface IFACE` (else `MARENGO_CAN_INTERFACE`, else all interfaces from motors.yaml), `--config-dir PATH` (else `MARENGO_CONFIG_DIR`) (`main.rs:95-125,159-174`). Every subcommand: load control+motors YAML, open SocketCAN, construct Supervisor; construction calls `sync_active_reporting()` which transmits **type-24 active-reporting enables** because `config/control.yaml:11` sets `bench.active_reporting_diagnostics: true` (`davout/src/lib.rs:524,543-545`); nothing turns them off at exit.

### Subcommand matrix vs ADR 0036 (process-local grants)

| Subcommand | Code | Effect now | Callers (file) | Still makes sense? |
|---|---|---|---|---|
| `status` | `main.rs:223-246` | Prints `operational: Disabled, control: Disabled` and `homing … Unhomed` for every joint — constant for a fresh owner (`davout/src/lib.rs:646-655`) | MCP `pi_motor_repl_status` (`readonly.ts:76-83`), harness `motor_repl_status`/`final_status` (`harness/index.ts:329,395`), `install-pi.sh:391` (hint text) | Only as "config loads + SocketCAN opens" smoke; information content zero. Side effect: type-24 enables. |
| `homing-status` | `main.rs:247-264` | Always `homing=Unhomed`; `pos` from `joint_feedback` cache that this process never refreshes → likely `n/a` [INFERENCE: no drain/tick call] | `scripts/homing-preflight.sh:67` (via MCP `homingReportShell` in `pi_health`, `pi_sync_bench_config`), MCP `pi_homing_status` (`motion.ts:703-714`), `pi_set_zero` readback (`zeroActuatorRemoteBody`, `motion.ts:579-590`) | **No.** Always reports not-Verified, so `homing-preflight.sh:70-74` always warns and `pi_set_zero`'s "verify" readback always shows Unhomed right after a successful set-zero. Live homing must come from marengo-pi RobotState (already the MCP fallback, `homing-preflight.ts:11-16`). |
| `home` | `main.rs:265-271` | `set_homing_complete` → `ensure_reference_binding` fails ("no qualified acquisition capability", `davout/src/lib.rs:1048-1063`) → exit 1 | docs only (`docs/pi-commissioning.md:134,173`, ADR 0006:53) | **No — unreachable success.** |
| `enable [op] [--force]` | `main.rs:272-299` | Same `set_homing_complete` failure (`:280-287`) → exit 1; gravity preflight never reached | MCP `pi_motor_enable` (`motion.ts:606-627`) | **No.** `pi_motor_enable` is a broken tool. |
| `disable` | `main.rs:300-307` | `disable_all` + mode Disabled; real CAN stop/fault-clear frames | MCP `pi_motor_disable`, `pi_motor_recover` (`motion.ts:557-563`), `pi_hold_off`, post-session cleanup (`motion.ts:504-521`), harness `final_disable`, `scripts/profile-pi-loop.sh:28`, `docs/safety.md:233` | **Yes** (only independent software stop when no owner runs). |
| `jog <j> <rad>` | `main.rs:308-335` | `set_homing_complete` fails first → exit 1. Even if admitted, `send_joint_command` is the legacy kp=kd=0 path (CS20) | MCP `pi_jog` (`motion.ts:774-795`) | **No.** `pi_jog` is a broken tool. |
| `speed <j> <rad_s>` | `main.rs:336-371` | Refused by `control.bench.allow_firmware_speed_mode: false` (`config/control.yaml:9`), then `set_homing_complete` would fail | none | **No.** |
| `speed-stop <j>` | `main.rs:372-382` | Sends firmware speed 0 (`davout/src/lib.rs:2288-2292`); no producer of speed mode exists | none | **No** (only counterpart of dead `speed`). |
| `set-zero <j> [--sign-tested]` | `main.rs:383-403` | Physical owner + journal; `calibrate_joint_zero` runs qualified workflow, stops drives, commits history row; grant ends at exit (ADR 0036 §Process lifetime) | MCP `pi_set_zero` (`zeroActuatorRemoteBody`, `motion.ts:579-590`), docs `homing.md:117`, `tuning.md:13` | **Partially.** Writes drive zero + history, but the next marengo-pi starts Unreferenced and must run `home <j> sign-tested` (another SetZero) before Enable; redundant for the enable workflow [INFERENCE]. |
| `gravity-on` / `gravity-off` | `main.rs:404-411` | Sets mode in a loop that never ticks; zero bus effect (CS20) | none (MCP `gravity-on` targets marengo-pi stdin) | **No.** |
| `torque-cmd <j> <nm>` | `main.rs:412-431` | Latches τ_cmd in a never-ticked loop; zero bus effect | none | **No.** |
| `gravity-preview [q…]` | `main.rs:432-459` | Pure model τ_g in robot.yaml order; still opens SocketCAN and sends type-24 | MCP `pi_gravity_preview` (`readonly.ts:86-101`), gravity gate (`gravity-gate.ts:124-138`), harness weighted profile (`harness/index.ts:347-357`), `pi_gravity_calibrate` | **Yes**, but does not need CAN; belongs in a non-CAN tool (e.g. `marengo-log-cli` or a model CLI). |

Depth: shallow pass-through to Davout/Berthier; no seams of its own (concrete `ControlLoop<RuntimeBus>`, `main.rs:23`). Consumers listed above; no Rust dependents. Coverage 0.0 % (0/382 lines, `metrics/coverage-by-file.md:13`; `metrics/coverage-by-crate.md:17`); 0 tests. `cargo machete` reports no unused deps (`metrics/unused-deps.md`).

## 5. Invariants owned

| Invariant | Enforcing code | Test |
|---|---|---|
| Only `set-zero` gets the physical-reference owner/journal; journal faults cannot block `disable` | `main.rs:189-213` | **untested** (0 tests in crate) |
| Set-zero requires explicit `--sign-tested`; Davout validates target before arming and stops drives before return | `main.rs:388-393` → `calibrate_joint_zero` (`davout/src/lib.rs:882-950`) | Davout tests (`calibrate_joint_zero_refuses_while_active`, `…unknown_joint_before_enable`, `davout/src/lib.rs:3412,3430`); none in motor-repl |
| Firmware speed mode refused unless `allow_firmware_speed_mode` | `main.rs:345-348` | **untested** |
| Gravity saturation refuses enable without `--force` | `main.rs:289-293` | **untested**, and unreachable (L1) |
| `disable` sends Disable to every configured drive and exits non-zero if any write fails | `main.rs:300-307` → `disable_all` (Davout aggregates failed writes, CS08) | Davout tests only; motor-repl path **untested** (0 % coverage) — safety gap, not prune signal |

## 6. Inputs / outputs

- Env: `MARENGO_ROOT`, `MARENGO_CONFIG_DIR`, `MARENGO_CAN_INTERFACE` (`main.rs:97-98`), transitively `MARENGO_JOINT_SUBSET`, `MARENGO_CALIBRATION_RECORD`, `RUST_LOG` (`marengo_support::init_tracing`, `main.rs:128`).
- Config: control.yaml (`loop_hz`, `chappe_state_hz`, `bench.allow_firmware_speed_mode`, `bench.active_reporting_diagnostics`), motors.yaml, robot.yaml (dynamics order), homing.yaml/reference journal path (`resolve_reference_journal_path`, `main.rs:193`).
- CAN out: type-24 active-reporting enables on every run; Disable frames (`disable`); SetZero/type-0/type-17 reference exchange (`set-zero`); firmware speed 0 (`speed-stop`).
- Files: reference journal + calibration history (set-zero only).
- Stdout lines parsed by MCP: `<joint>: tau_g = <Nm> Nm` (`main.rs:457`; regex in `gravity-gate.ts`), `<joint>: homing=<State> pos=…` (`main.rs:258-262`; grep in `homing-preflight.sh:70`). Exit code 1 on any refusal.

## 7. Prior review reconciliation

| ID | Prior | Current | Evidence |
|---|---|---|---|
| CS07 | set-zero enables all drives, no cleanup on exit paths | **partial → largely fixed for this bin**: no `request_enable_for_calibration`; `calibrate_joint_zero` validates first and stops drives before returning (`main.rs:389-393`); constructor-before-disable dependency remains (codemap; `docs/safety.md:233-236`) | ledger "partial"; commit `e1a1771`; implementation-roadmap "CS05/CS06/CS07 software implemented; bench qualification pending" |
| CS20 | one-shot commands only mutate exiting process; jog legacy kp=0 | **open**: `gravity-on/off`, `torque-cmd` unchanged (`main.rs:404-431`); `jog` still `send_joint_command` (`:325`) but now unreachable behind `set_homing_complete`; `status` still local | ledger open |
| T30 | partial gravity-preview vector silently becomes zeros | **open**: `main.rs:436-448` (`args.len() >= 2 + joint_count` else all zeros, excess truncated). robot.yaml has 5 joints (`config/robot.yaml:10-15`); `pi_gravity_preview` default `[0,0]` (`readonly.ts:96`) and harness weighted profile passes 2 angles (`harness/index.ts:353`). `gravity-gate.ts:118-138` works around it. | ledger open |
| T05 / T07 | MCP competes with owner; exit status lost | out of crate. T07 still visible: `auditMotion("pi_motor_enable", args, out, 0)` hard-codes exit 0 (`motion.ts:626`), so the always-failing enable is audited as success | ledger open |

## 8. Drift

| Doc | Says | Code |
|---|---|---|
| `bins/motor-repl/codemap.md` Design | subcommands incl. `hold-on`, `hold-at`; "interactive REPL"; arg `--can` | no hold commands; one-shot; arg is `--can-interface` (`main.rs:102`); omits `home`, `speed`, `speed-stop`, `gravity-off`, `torque-cmd` |
| `bins/motor-repl/codemap.md` Integration | "Primary bench tool for MCP `pi_hold_on`, `pi_motor_recover`, `pi_set_zero`" | `pi_hold_on` runs marengo-pi; motor-repl only does pre/post `disable` |
| `bins/AGENTS.md`, `bins/codemap.md`, `codemap.md:12`, Cargo description | "Interactive", "enable/jog" | enable/jog/home cannot succeed (§4) |
| `docs/pi-commissioning.md:134,150-190,264` | `motor-repl home`, `gravity-on` workflow | `home` always fails; `gravity-on` no-op |
| ADR 0006:53-54 | `motor-repl home` / `enable` | superseded by ADR 0022/0023/0036 |
| `bins/AGENTS.md` table "Host: Dev (bench)" | dev host | deployed to Pi `/opt/marengo/bin` (`scripts/deploy-pi.sh:193`, `install-pi.sh`) and run there by MCP |

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|
| `home`, `enable`, `jog`, `speed` subcommands (`main.rs:265-299,308-371`) | superseded by ADR 0036 / commit `7951f10` (fresh ordinary owner has no acquisition capability, `davout/src/lib.rs:1055-1058`) — cannot succeed; **0%** coverage does not distinguish dead from safety-critical (`coverage-by-file.md`) | high | MCP `pi_motor_enable`, `pi_jog` (`motion.ts:606-627,774-795`) + their tests in `tools/marengo-pi-mcp/test/motion.test.ts`; docs `pi-commissioning.md`, ADR 0006 notes; `preflight_gravity_saturation` (`main.rs:20-66`) becomes dead |
| `speed-stop` (`main.rs:372-382`) | scaffold with no consumer (speed mode disabled in config, no caller) | med | Davout `stop_speed_command`/`send_speed_command` may become dead (check other users) |
| `gravity-on`, `gravity-off`, `torque-cmd` (`main.rs:404-431`) | zero bus effect (CS20); zero callers in tools/scripts | high | usage text, docs |
| `homing-status` (`main.rs:247-264`) | superseded by ADR 0036 (always Unhomed in a fresh process) | med | `scripts/homing-preflight.sh`, MCP `homingStatusShell`/`homingReportShell`, `pi_homing_status`, `pi_set_zero` readback, tests `homing-preflight.test.ts`, `motion.test.ts:459`; replace with gateway RobotState path already present |
| `status` (`main.rs:223-246`) | constant output for fresh owner | low (used as CAN/config smoke by harness) | MCP `pi_motor_repl_status`, harness steps, `install-pi.sh:391` |
| `vcan` feature (`Cargo.toml:16`) | feature never enabled (no reference in scripts/justfiles) | high | Cargo.toml |
| Duplicate `preflight_gravity_saturation` (`main.rs:23-66`) | duplicate implementation of marengo-pi `main.rs:808-855` | high | dead once enable is removed |

## 10. Phase-B leads

| # | Location | Suspicion |
|---|---|---|
| L1 | `main.rs:280-287,317-320,349-352` | `enable`/`jog`/`speed` call `set_homing_complete` on an ordinary owner that can never hold a grant → always exit 1. MCP `pi_motor_enable`/`pi_jog` are advertised but cannot work; motion audit records exit 0 (`motion.ts:626,793`). |
| L2 | `davout/src/lib.rs:543-545` + `config/control.yaml:11` | Every motor-repl run (incl. read-only `status`, `gravity-preview`, `homing-status`) transmits type-24 enables and never disables them on exit; drives keep streaming into a bus marengo-pi later owns (cf. `6a1bb9d` "stop mcp251x RX overruns"). |
| L3 | `main.rs:436-448` | T30: partial angle vector silently → zero pose; excess silently truncated. |
| L4 | `motion.ts:579-590` + `main.rs:247-264` | `pi_set_zero` "verify" readback uses a fresh-process `homing-status`, which always reports Unhomed — readback cannot confirm the set-zero it follows. |
| L5 | `main.rs:300-307` | `disable` depends on full constructor success (config + history + SocketCAN for every interface unless `--can-interface`); a single missing interface aborts the stop. Gap vs "independent stop" (`docs/safety.md:233-236`). |
| L6 | `main.rs:136-138` | `--config-dir` mutates process env via `env::set_var` after `init_tracing` (fine single-threaded now; unsafe in Rust 2024 / with threads). |
| L7 | `main.rs:189-213` | Physical owner chosen by `args[1] == "set-zero"` string compare; any future alias/flag ordering (`--config-dir` after subcommand is treated as command args, `:118-121`) silently falls back to ordinary owner. |
| L8 | `main.rs:96` | `args[0].clone()` indexing; `parse_bus_args` never validates that flags precede the subcommand (flags after it are passed as subcommand args). |
| L9 | `metrics/coverage-by-file.md` | **0%** on entire `main.rs` — `disable` and `set-zero` (§5 safety invariants) have **no** llvm-cov or crate tests. Phase-B should add smoke/integration tests for those subcommands before pruning others; absence of coverage is a **gap**, not prune evidence for `disable`. |
