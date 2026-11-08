# Marengo software tenets

**Agreed by Joey, 2026-11-08.** These are the rules the marengo software is judged against. The left arm is brought up on a
fresh, lean stack that follows them. Existing code that doesn't honour them gets rewritten or deleted, and once the left arm
passes its tests, the right-arm-era code that wasn't carried over is deleted. A reviewer, human or agent, can cite a tenet by
number as the reason for a change.

## 1. A fixed robot, not a kit

The robot has a fixed set of limbs: two arms, two legs and the waist. Each has pre-assigned RobStride actuators on a fixed bus
with fixed IDs. Our design process is built around the actuators, so once one is in place, it stays. When a drive answers it is
registered and addressed; when it doesn't, it shows as offline. Nothing else.

- No mix-and-match of actuator types per limb, no plugin layers, no generic "any limb, any motor" abstractions.
- A limb that isn't built yet is just a limb whose drives are all offline. It needs no separate code path.

## 2. One skeleton file is the contract

One hand-agreed file (YAML) describes the whole robot: every limb, every joint in it, its actuator type, bus, device ID,
direction, zero, and soft and hard limits. The runtime reads that file and nothing else about the hardware. **The software never
reads CAD or design files.**

- When the hardware changes, the hardware side edits the skeleton, in one commit, and says so in [STATUS.md](STATUS.md).
- A URDF or MJCF for sim and visualisation may be generated from CAD or from the skeleton, but the runtime doesn't depend on it.
- One fact lives in one place. If a joint name, limit or ID appears in two files, one of them is wrong.

## 3. Layered safety, each layer with a job

We will never catch everything, so safety is layered, and each layer covers a failure the others can't.

1. **Hardware:** physical E-stop in motor power; power board heartbeat watchdog; fuses and port breakers.
2. **Drives** (holds even if the Pi crashes): CAN host-loss timeout of 50 ms or less, torque, current and speed caps, the
   anti-backdrive damper left on.
3. **Host, a short fixed list:** check the drive settings above before every enable and refuse on any mismatch; soft limits
   inside every hard stop; a bound on *total* commanded torque (not only feed-forward); stop on any fault and never auto-clear
   it; stop when a drive goes silent.

Every safety rule names the hardware it protects and the failure it covers. A rule that duplicates another layer, or protects
nothing nameable, goes. There are no spare actuators, so motors come first, then printed parts, then the frame.

## 4. Build only what's needed now

Code exists only if a test we're actually running needs it. No scaffolding for features that may come later.

- ADRs, docs and version files describe what exists and what was decided, not what might be built. Speculative code, ADRs and
  docs are deleted, so no agent is afraid to touch anything.
- If something is needed later, it gets built later, against the real need.

## 5. The CAN layer is the foundation

CAN is the most important data stream in the robot, so it is the most trusted code, not the most feared. **It is rewritten from
scratch**, small and readable:

- one realtime process on the Pi owns every CAN bus;
- the RobStride protocol is implemented from the vendor manuals ([hardware/docs/robstride/](docs/robstride/)) and checked
  against real drives;
- every frame in and out can be logged, with timestamps;
- it runs unchanged against a simulated drive on `vcan`, so faults can be injected without risking a motor;
- bus names come from the skeleton, so moving a limb from the HAT to a USB-CAN adapter is a config change.

Everything else (UI, logging, storage, agent tools) talks to the realtime process through one interface, and can crash without
affecting motion.

## 6. Measure, automate, close the loop

- Every tuned number comes from a logged test, and every session is logged so sim can replay it.
- Anything we'd do more than twice becomes a script or loop an agent can run and judge on its own. A feedback loop beats
  debugging by hand every time.
- **No pointless tests.** A test exists only if it guards a real failure: a bug we actually hit, a safety rule, a protocol
  contract, or a number a loop depends on. No tests written for coverage or just because a file was touched.

## 7. Any agent can learn it fast

An agent (or a person) new to the repo should know within minutes what every layer does, what it owns, who it talks to, and
where the seams are: between crates, and between the big pieces (Pi runtime, CAN, UI front end, back end, storage, agent tools).

- One short architecture map, kept current with every change that moves a seam.
- Each crate or app starts with a few lines: what it is for, what it must never do, what it talks to.
- Names say what things do.

## 8. Observability everywhere

Detailed, concise, timely logs at every step, because the real-to-sim loop runs on them.

- Structured, timestamped events and metrics from every layer, on one shared clock (Pi and Jetson synced).
- Every motion command, drive reply, fault, limit hit, state change and operator action is recorded.
- Use a proven off-the-shelf stack instead of home-grown logging. Candidates to evaluate: OpenTelemetry with Grafana (Loki,
  Prometheus or Tempo), Sentry for crashes, and robotics tools such as Foxglove/MCAP or Rerun for time-series replay.
