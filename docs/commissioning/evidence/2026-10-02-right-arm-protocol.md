# Right-arm disabled protocol inspection — October 2, 2026

The actual five-drive inspection succeeded at approximately 20:14 UTC. Main Pi
runtime PID was zero/inactive before execution. The operator confirmed the arm
remained supported at mechanical home. No Enable or Set Zero was sent.

Probe source: `e5599c6083a1338f9aade020bd3090ce007dfc3a`.
Linux motor-repl SHA256:
`610401160a98ba6fa380211b6cf6270bc96477fc4bbad3bcca054e0e0d5c31d2`.
Installed main runtime SHA256 remained
`dd2ec386471224c43f3d0cde6c37cc9f95e1773eb3cf714cabd85ff57a2defa9`.

| Joint / can0 ID | MCU identifier, wire-order hex | Actual firmware | mechPos (motor rad) | CAN timeout |
|---|---|---|---:|---:|
| shoulder pitch / 1 | 457b30020c323817 | 0.3.1.42 | 0.0258220 | 0 |
| shoulder roll / 2 | 785630020c343701 | 0.3.1.42 | 0.0158937 | 0 |
| upper-arm yaw / 3 | 9d6a3b859c023019 | 0.2.3.34 | -0.00643281 | 0 |
| elbow pitch / 4 | 8a063b8f2450b00d | 0.2.3.34 | -0.0133605 | 0 |
| lower-arm yaw / 5 | 622c30020c343701 | 0.0.3.32 | 4.1749973 | 0 |

All version reply headers reported Reset. All run-mode reads were MIT (0),
zero-wrapping reads were 0 and additive offsets were 0. Forty addressed queries
completed with the observed host/index echoes; MCU replies used fixed FE.

Raw receipt and passive 255-frame capture are committed beside this summary.
The operation's canonical stops and reporting Off account for additional frames.
MCU identity is not a boot epoch; no reset or Set Zero behavior was qualified by
this inspection. Lower-arm yaw's negative joint-space position follows the
installed direction -1. At mechanical home it needs a fresh zero.

Installed configuration and URDF were preserved. Their hashes are in the
associated `sha256.txt`. Actual RS02/RS00 firmware differed from configured
metadata; source metadata is corrected separately, without deploying limits.
