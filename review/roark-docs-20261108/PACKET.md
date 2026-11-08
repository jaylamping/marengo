# Sharpening packet: Roark TENETS.md and PLAN.md

## What this is

Roark is a full-size hobby humanoid robot (23 joints, RobStride RS00/RS02/RS03/RS04 actuators on five CAN buses, Raspberry Pi 5
realtime host, Jetson later). Its software is being rebuilt from scratch in a new repo. Two documents are the **entire handoff**
to the agents who will build it, who will have no other context: `roark/TENETS.md` (the rules every change is judged by) and
`roark/PLAN.md` (what to build, in order). `roark/CLAUDE.md` and `roark/README.md` are the entry files.

This is **not** a pass/fail review. The goal is to make these documents so clear and complete that a capable agent with only
these files (plus the reference repo) builds the right thing, in the right order, without guessing.

## Files

- `roark/` : the four documents to sharpen.
- `marengo/` : the reference repo snapshot (hardware repo + legacy software). Use it to check facts and find what the documents
  leave out. Key files: `marengo/hardware/STATUS.md`, `marengo/review/sw-foundation-20261108/RECONCILIATION.md` (and its
  `reviews/`), `marengo/hardware/docs/research/can-stack-for-marengo.md`, `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md`,
  `marengo/hardware/docs/sim-to-real-loop.md`, `marengo/hardware/electrical/pdb/REV_B_PARTS.md`, `marengo/hardware/docs/robstride/`
  (vendor manuals), and the legacy code under `marengo/crates`, `marengo/bins`, `marengo/config`.

## Owner's intent (decided; sharpen within it, don't relitigate)

- Every change is for the finished robot, never for the limb on the bench; the left arm is milestone 1 only.
- Fixed robot, one skeleton YAML, layered safety, build only what's needed, CAN layer rewritten from scratch as the trusted
  foundation, measure/automate/no pointless tests, any agent can learn it fast, off-the-shelf observability.
- New repo; legacy code is reference only and is deleted from the old repo after milestone 2.
- CAN stack per the research report (SocketCAN, gs_usb adapters, socketcan 4.0 crate, own sans-IO codec, one thread per bus,
  250 Hz, MCAP + candump, vcan sim drive).

## Questions

1. **Vagueness.** Quote every sentence an agent could read two ways, or that names a goal without a testable "done". Give the
   sharper replacement text.
2. **Gaps.** What does an agent need to decide or know that neither document says? (Examples to consider, not a limit: skeleton
   schema and its fields and units; zero/offset convention; the realtime process's interface (transport, message set, who may
   command motion); state machine for a drive and a limb; exact enable/disable/stop sequences; repo layout, toolchain, CI and
   deploy to the Pi; logging schema and file layout; how the sim drive is driven; acceptance criteria per milestone; how the
   right arm migrates; what "done" means for each plan item.) Propose concrete text for each.
3. **Contradictions.** Between TENETS, PLAN, CLAUDE.md, README and the reference docs (numbers, names, paths, ownership).
   Say which should win and the fix.
4. **Facts.** Check every number and fact in PLAN against the reference repo and the RobStride manuals. List anything wrong
   or unsupported.
5. **Order.** Is the "Start here" and milestone order right for a first agent? Propose a better order if not, with reasons.
6. **Over-specification.** Anything in the documents that pre-decides something better left to the builder, or that adds
   process the tenets would call unneeded (tenet 4).
7. **Questions only the owner can answer.** List them, each with a recommended default so work isn't blocked.
