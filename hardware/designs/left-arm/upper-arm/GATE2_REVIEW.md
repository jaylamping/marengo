# Gate 2 summary, upper arm rev B

2026-11-06. Reviewers: Codex Sol, Codex Astra, Cursor Grok 4.7 High Fast, Muse Wonder. Antigravity not run (Mac only).

**Result: all four say no-go for the print and jig test as specified. Rev C is needed.**

## Accepted, must fix before printing

| # | Finding | Raised by | Checked | Fix |
| --- | --- | --- | --- | --- |
| 1 | M4 heads have no load path into the PETG. 1.39 kN per bolt at 50 N·m on about 10 mm² is about 136 MPa | All | Arithmetic and STEP agree | Steel ring under the heads, dimensioned sleeves, bolt grade, torque, engagement |
| 2 | Hub ligament is 1.5 to 2 mm to the Ø14/Ø15 centre bore | All | STEP | Delete the centre bore, about 4 g |
| 3 | M3 ligament is 1.0 mm, and the Ø9 flange seat runs 1 mm into the Ø66 bore | All | STEP | Steel ring on the inboard hoop face. **The bore cannot shrink**: the RS02 nose is Ø65 (r 32.52), so Muse's second option is out |
| 4 | The hoop cannot sit on the bed: the tube sticks out 16.46 mm past it | Sol, Astra | Correct. My wording was wrong | Name the real build orientation and supports |
| 5 | Support in the tube is trapped behind a 21 mm window | All | Agreed, not sliced | Make the crown self-supporting, then slice and prove it |
| 6 | Print article undefined. FEA assumes solid | Sol, Astra, Muse | Correct | Frozen profile, weigh and gauge the print |
| 7 | Jig does not apply the FEA wrench, and one root reading cannot remove rotation | All | Correct | FEA the exact jig loads, two-point root reading, stiffer bar |
| 8 | 0.20 mm fore-aft prediction was scaled from rev A, not solved | Grok, Astra, Muse | My error | Replace with a solved number |
| 9 | "Can go to 25 kg" is not supported | Sol, Astra, Grok | Correct | Withdrawn. Cap at 10 kg |
| 10 | "Upper bound" claim for the peaks is not supported, and fixing the full M4 bore is not a limiter | All | Correct | Withdraw the claim. Add a contact model of each joint |
| 11 | Carrier floors 1 mm under Ø9.5 × 3 pockets, zero head clearance; BASELINE still says Ø6.5 | Sol, Astra, Muse | STEP | Thicker plate, correct doc |
| 12 | M4 access through the window is unproven | All | Geometry plausible with a short-arm key, unproven | Dry build on dummy discs |
| 13 | Collar root is thin, 1.25 mm where r 41 meets r 42.25, with a 1.5 mm fillet | Muse | Outer r 42.27 present near X 185 to 190 | Thicken locally, 3 mm fillet |
| 14 | No sleeve or insert part numbers; M4 length unbounded | Sol, Astra | Correct | Hardware schedule |
| 15 | Load cases imply 81.5 N·m at the shoulder and yaw is never combined | Sol, Astra, Muse | Arithmetic correct | Keep as a benchmark, add a combined case |

## Rejected

| Finding | Why |
| --- | --- |
| Grok: collar end cuts the upper M3 hole | The hole is r 36.5 ± 2.5, so it ends at r 39. The collar starts at r 41 and at X 161.19, the hoop's outer face. They never meet |
| Muse: holes cannot be reamed through the window | They open on the hub top face, so drill from outside |
| Muse: 1.27 mm deflection closes the 1.75 mm collar gap | The RS02 is bolted to the hoop and moves with it. Deflection is not relative to the collar. Print tolerance remains a check |

## Superseded

Muse M5 and Sol's shell note use packet-time `shell_gaps.json`. Later sweeps cover this: `shell_gaps_sock.json`, r 76 minimum.
The scripts still rely on sampled points, so the r 76 and r 78 margins of 0.2 to 0.8 mm above 3 mm are not firm until checked on the solids.

## Can wait until after a bench print

Bearing retention, fork, stop lug case, clamps, coupons, creep, fatigue, lightweighting.
