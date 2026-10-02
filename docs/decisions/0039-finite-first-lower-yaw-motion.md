# ADR 0039: finite first lower-arm-yaw motion

Status: accepted for implementation, October 2, 2026.

## Context

The five-joint neutral enable passed using actual owner-local physical reference.
Joseph authorized testing and tuning the unchanged arm over SSH, with the arm
supported near mechanical home, workspace clear and physical E-stop functional.
The next diagnostic should establish a small deliberate response without an
unrestricted motion interface. The observed neutral lower-yaw drift was 9.6 mrad.

## Decision

Add a separately named closed `PhysicalLowerYawBench` using the same real-bus
acquisition, profile checks, durable history and private current-reference path
as the neutral owner. Before coordinate writes, fresh mechanical-position reads
must already be inside the installed home tolerance, capped at 0.05 rad. Set Zero
cannot mask a large starting displacement. Neither constructor accepts a bus,
receipt, imported grant or virtual epoch; the neutral wrapper stays neutral-only.

The first motion profile has one second of enabled budget, including the initial
neutral active readbacks. Berthier ramps lower-arm yaw from 0 to +0.02 rad over
400 ms, returns over 400 ms and commands zero for the remaining window. Position
gains are fixed at kp=10, kd=0.4, with zero commanded velocity and torque feedforward.
Every other joint remains neutral. This is a supported near-home diagnostic;
it supplies no elevated position-hold capability.

Davout independently refuses any other joint's gains, nonzero velocity/torque,
changed gains, incomplete batches or target outside [0, 0.02]. The physical
home-band guards inspect each raw pose, including the Ready enable flush, with
absolute position <=0.05 rad and measured velocity <=0.25 rad/s. Ordinary limits,
faults and feedback watchdog remain effective. Finish/error/drop stops all five
addresses and revokes the reference. There is one enable, no automatic re-arm.

## Consequences

The profile is sufficient to measure initial lower-yaw direction and tracking.
Its receipt records samples and stop delivery; passing execution is not full limb
commissioning or proof of physical stop. Tuning and wider trajectories require
profiles with explicit numerical envelopes, exercised under the already approved
development session where applicable. Caller application identity is irrelevant.
