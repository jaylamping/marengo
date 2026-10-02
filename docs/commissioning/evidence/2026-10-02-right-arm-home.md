# Right-arm disabled home qualification — October 2, 2026

The approved operation succeeded at 21:02:55 UTC. The five-joint arm was
supported at mechanical home, with unchanged sign-attested mapping and the main
runtime stopped. The operator approved the exact disabled Set Zero and timeout
readback operation in the commissioning chat before execution.

Source: `cd03380853481fe3f659ebaaac0a34ef86241937`.
Linux motor-repl SHA256:
`58094d831f4c5ac318306c8a7bc9066a1254ea27e328be541125e0499466cc46`.
All five CI jobs passed at that exact source revision, including the full Linux
check job, simulation and virtual CAN.

| Joint / can0 ID | Fresh post-zero mechPos (motor rad) | CAN timeout, before and after zero |
|---|---:|---:|
| right_shoulder_pitch / 1 | 0.000042611 | 600 |
| right_shoulder_roll / 2 | -0.000170443 | 600 |
| right_upper_arm_yaw / 3 | 0.000098998 | 600 |
| right_elbow_pitch / 4 | -0.000049530 | 600 |
| right_lower_arm_yaw / 5 | 0.000000000 | 600 |

All five exact addressed Set Zero replies were fault-free Reset. Ten timeout
readbacks were exactly 600 raw counts. All five position readbacks were within
0.00018 rad of zero, below the installed 0.05-rad tolerance. Forty baseline
protocol queries and twenty qualification reads completed successfully.

The 230-frame passive capture contains five Set Zero requests, no Enable, and
only fault-free Reset status headers. Final canonical all-address stop completed
successfully. `grants_reference=false`: this is external protocol qualification,
not current-reference permission or motion approval.

The installed runtime binary, YAML, taught limits, URDF and calibration history
were preserved. Their hashes and the source revision accompany the raw receipt
and capture. This revision supplied no physical current-reference owner. The
subsequent [neutral](2026-10-02-right-arm-neutral.md) and
[lower-yaw](2026-10-02-right-arm-lower-yaw.md) evidence records the closed owner
implementation and actual enabled tests. Motor-power reset/timeout behavior
remains unmeasured.
