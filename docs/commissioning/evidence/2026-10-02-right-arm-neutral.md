# Right-arm neutral enable — October 2, 2026

The approved remote neutral check passed on the unchanged supported five-joint
arm. Source `af28b864741938d3f9dfa0c9aef0c19b105fd9af`; Linux SocketCAN executable
SHA256 `da8e58d345bac5c9cc391ed8bd13c046a331d4893c80299aa4361c183bc8fe45`.
Raw reports, audit, capture, operator authorization and file hashes are in the
[evidence directory](2026-10-02-right-arm-neutral/).

One Enable was sent to each of can0 IDs 1–5. The enabled command window was
500,061 us, including active home/timeout checks, followed by 15 accepted stop
writes with no delivery failures. Berthier recorded 95 complete five-drive ticks;
maximum tick delay was 83 us. The passive capture puts the last Disable write
504.7 ms after the first Enable. Accepted writes are not a physical stop proof.

| Joint | Maximum absolute reported position (rad) |
|---|---:|
| right_shoulder_pitch | 0.0003835 |
| right_shoulder_roll | 0.0003835 |
| right_upper_arm_yaw | 0.0007670 |
| right_elbow_pitch | 0.0003835 |
| right_lower_arm_yaw | 0.0095874 |

All 555 MIT requests used zero position/velocity/torque/gains. All 620 decoded
ordinary status headers were fault-free Reset or Run; firmware-version replies
are separate. The lower-arm-yaw drift was about 0.55 degrees, inside the 0.05-rad
guard. This check establishes the enabled neutral path and reports drift; it
does not establish intentional tracking, gravity compensation or stop distance.

The first executable was accidentally built without SocketCAN. That attempt
returned before bus construction and its capture contains no CAN frames. It is
retained alongside the successful retry, which used `--features socketcan`.

Installed YAML, URDF, taught limits and main runtime binary hashes match before
and after. The runtime remains inactive with MainPID=0. Audit is history only;
the owner reference was revoked at finish. Timeout volatility and output-stop
behavior remain unmeasured.
