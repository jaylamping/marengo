# Review packet: marengo software foundation (keep / rebuild / delete)

**Repo snapshot:** jaylamping/marengo main at the commit named in `repo/SNAPSHOT.txt` (2026-11-08), copied into `repo/`.
Software lives in `repo/crates/`, `repo/bins/`, `repo/tools/`, `repo/consul/`, `repo/config/`, `repo/proto/`, `repo/sim/`,
`repo/docs/`. Hardware docs are in `repo/hardware/` (read only the ones listed below).

## What this is

A personal humanoid robot. Today one right arm (5 RobStride drives on one CAN bus) runs on this software on a Raspberry Pi 5.
A new left arm (same five joint types, new mechanics, its own bus `can1`, IDs 11-15, URDF generated from CAD) is about to be
built and tested. The owner plans to bring the left arm up on a fresh, lean stack, keep only the parts of the existing software
that meet the tenets below, and delete the rest once the left arm passes its tests. The owner believes the current stack is
over-built (topology, safety layers, other things). Verify or refute that against the code; do not assume it.

## The tenets (the yardstick)

The eight tenets are in `repo/hardware/SOFTWARE_TENETS.md`. Read that file in full before anything else. In short:
1 a fixed robot, not a kit (fixed limbs, pre-assigned drives, online or offline, no mix-and-match abstractions);
2 one skeleton YAML is the contract, runtime never reads CAD/design files, one fact in one place;
3 layered safety, each layer with a named job (hardware, drives, a short fixed host list);
4 build only what's needed now, no speculative scaffolding in code, ADRs or docs;
5 the CAN layer is the foundation and will be rewritten from scratch;
6 measure, automate, close the loop, no pointless tests;
7 any agent can learn the codebase fast (map of layers and seams);
8 observability everywhere, off-the-shelf stack rather than home-grown logging.

## What the left arm tests need from software

- `repo/hardware/designs/left-arm/testing/SOFTWARE_PREP.md` (ranked list of the software the tests need)
- `repo/hardware/STATUS.md` (hardware state, joint/CAN table, power board software requirements)
- `repo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` (camera calibration loop; section 9 safety rules, section 11 spec)
- `repo/hardware/docs/sim-to-real-loop.md` (actuator test stand T1-T6, sim/HIL spec)
- `repo/hardware/electrical/pdb/REV_B_PARTS.md`, section "Accepted risks and software requirements" (what the power board needs from software)
- RobStride vendor manuals: `repo/hardware/docs/robstride/`

## Size, to plan your reading (lines of .rs/.ts/.tsx/.py/.proto/.yaml/.md)

| Area | Lines | Area | Lines |
|---|---|---|---|
| crates/davout | 36,251 | bins/marengo-pi | 14,451 |
| crates/berthier | 21,998 | bins/marengo-gateway | 9,753 |
| crates/marengo-store | 10,305 | bins/marengo-log-cli | 8,666 |
| crates/robstride | 7,278 | bins/motor-repl | 1,770 |
| crates/marengo-config | 6,844 | tools/marengo-pi-mcp | 16,090 |
| crates/armee-dynamics | 3,976 | tools/marengo-research-mcp | 2,384 |
| crates/chappe | 2,983 | tools/compound-auto-learn | 1,475 |
| crates/marengo-candump | 1,913 | consul/src | 35,567 |
| crates/marengo-deploy | 1,798 | docs/decisions (ADRs) | 4,280 |
| crates/marengo-host-metrics | 1,658 | config | 882 |
| crates/armee-kinematics | 1,246 | proto | 783 |
| crates/marengo-imu | 1,180 | other small crates/bins | < 600 each |

Start with `repo/README.md`, `repo/docs/architecture.md`, `repo/docs/safety.md`, `repo/docs/status.md`, `repo/docs/decisions/`,
and each crate's README / `codemap.md`.

## Checks done

None on the code. This packet is the first review against the tenets.

## Known open items (from docs/status.md, not verified)

MCP2515 RX overrun cause B; CS10 (bench caps bound feed-forward only); CS14; M02/M03 drive timeout and fault-clear policy;
left-arm naming and bus mismatches in config.

## Questions

Answer 1 to 14. Question 1 is a table; keep the rest tight.

1. **Per component verdict.** For every crate, bin and tool: keep as-is, keep but slim, rebuild, or delete, for the left-arm
   stack. Name the tenet each verdict rests on, with file paths as evidence. Give a table.
2. **Topology.** Describe the actual process and bus layout (marengo-pi, Chappe, gateway, Consul, Davout, MCP). Where does it
   break tenet 5 or tenet 4? Propose the smallest topology that meets the tenets for one arm and still scales to 23 joints on
   five buses.
3. **Safety inventory.** List every safety mechanism in the code (supervisors, grants, leases, gates, preflights, caps, danger
   zones, degraded modes). For each: (a) required by the hardware (see tenet 3 and the hardware docs), (b) cheap and useful,
   or (c) breaks a tenet. Flag any hardware-required protection that is missing or wrong today (for example drive CAN timeout
   read-back before enable, total-torque bound vs feed-forward only).
4. **Control.** Which modes and control laws are needed for: sign/zero, the test stand T1-T6, and the calibration loop C0-C8?
   Which are not needed?
5. **Config and URDF.** Trace how joint names, limits and masses flow from config/URDF into the runtime. Where is the same
   fact stored in more than one place (tenet 2)?
6. **Hard-won knowledge.** What in the code encodes real bench lessons that a rewrite must keep (RobStride protocol quirks,
   MCP2515 pacing, enable/disable sequencing, timing)? Point to the exact code so it can be lifted out before anything is deleted.
7. **Deletion plan.** If the left arm passes on the lean stack, what can be deleted outright, and what must be migrated first?
8. **CAN layer.** The owner has decided to rewrite it (tenet 5). Describe the current CAN path end to end (robstride crate,
   transport, pacing, Davout, marengo-pi) and its worst failure modes. What exact behaviour must the rewrite reproduce, and what
   should it drop?
9. **Speculative scaffolding (tenet 4).** Which code, ADRs, proto messages, config files and docs describe or implement things
   that don't exist or aren't used? List them for deletion.
10. **Tests (tenet 6).** Which tests guard a real failure, and which exist for coverage only?
11. **Logging and observability (tenet 8).** What does the current logging/telemetry/storage stack do (marengo-store,
   log-cli, gateway, Consul telemetry)? Which parts would an off-the-shelf stack replace?
12. **Agent onboarding (tenet 7).** Could a new agent learn the layers and seams quickly from the current docs (AGENTS.md,
   codemap.md files, architecture.md)? What is wrong or stale?
13. **What are we missing?** List proven off-the-shelf tools, libraries, protocols or cheap hardware that would replace
   home-grown code here or make the loop faster (for example: USB-CAN adapters with gs_usb/SocketCAN instead of an SPI CAN
   HAT, can-utils, existing RobStride driver libraries, ros2_control-style patterns, MCAP/Foxglove/Rerun, OpenTelemetry).
   For each: what it replaces, why it's better, and the catch. Only real, named projects you are confident exist.
14. **Anything else** that would make the left-arm bring-up fail or waste time.
