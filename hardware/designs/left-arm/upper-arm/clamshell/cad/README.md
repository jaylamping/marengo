# Upper arm clamshell: detailed CAD

Draft 2, 2026-11-08.

Status: **the mesh gate passes, the support check and seam FEA are done.** The two-reviewer gate is still to come, so don't print yet.

## Source

`ua_master.py` copies the db71b23b part (`cad/parts/left_arm/marengo_upper_arm_left.SLDPRT`) and suppresses the cable window and cover. The outer skin is then continuous to ±0.03 mm.

`ua_parts.py` cuts the parts from that master. The helpers in `swg.py` work in arm coordinates.

```
python ua_master.py build COVER_RECESS_B,WINDOW_ROUND,COVER_INSERTS_B,FLOOR_EXIT,CABLE_COVER_B,COVER_CLEAR_B,COVER_ENTRY,COVER_BORE,COVER_BORE2
python ua_parts.py ring hub upper lower clip      # parts
python ua_parts.py fea                            # one-body FEA model
```

- Exports (STEP and STL, arm coordinates): `hardware/designs/left-arm/prints/upper-arm-clamshell/`.
- Gate outputs: `gate/`.

## Parts and printed mass

Sliced in Bambu Studio with the P2 process: PETG, 0.2 mm, `ua_slice.py`.

| Part | What it is | Print | Sliced |
| --- | --- | --- | --- |
| `ua_upper` | Half above the seam (Z > 6). Four Ø7.5 seam posts and two hub-lug posts, with Ruthex inserts, M3 × 30 pilots and Ø5.9 pin sockets. Fuse pockets from the master | Seam face down. 4 walls, 50%, with 6 walls and 100% on the posts, the hub band (Y 286–303) and the ring capture | **55.3 g** |
| `ua_lower` | Half below the seam. Matching posts with Ø5.5 × 1.2 pins and counterbores. Ø10 trunk entry through the back wall below the hub, with a tie boss. Ø6.8 RS00-branch exit at Y 270 through the back-inboard wall, with a tie boss | Seam face down. Same settings | **53.5 g** |
| `ua_ring` | RS02 seat ring, X 150.8–166.2, keeping the master hoop OD (cut at r 44.25). It takes the skin flare below Y 242 and has a top slot widened to Z −9.5..18. Two M3 inserts on the OD at Y 246.5 | 6 walls, 100% | **32.2 g** |
| `ua_hub` | Hub disc: a 4 mm plate at the yaw RS02 face with six windows, the old tube floor opened, a spigot r 28–35.8, and two seam lugs rooted in the spigot | RS02 face down. 6 walls, 50%, with 100% on the plate, the bosses and the spigot | **63.4 g** |
| `ua_loop_clip` | Elbow back-loop saddle for the **unchanged** carrier: a 3 mm foot, and two crush-ribbed spigots in the carrier's Ø9.4 screw-access bores at 44° and 84° | Standing on its flat end. **No support** | **7.7 g** |

**Total: 204.4 g sliced for the four arm parts.** The one-piece db71b23b arm sliced at 229.0 g on the same process, and weighed **202 g** on the scale. That is 10.7% lighter, so expect **about 180 g**, plus the 7.7 g clip.

The parts are small and thin, so 6 walls plus shells make them nearly solid; dropping the halves to 4 walls saved little. The real savings came from the hub plate (6 → 4 mm), the windows and the floor opening.

## Seam and fasteners

- **Seam:** Z split at Z 6.
- **Seam posts:**
  - At X 147.5 and 193.5 (23 mm off the tube axis), Y 262 and 278.
  - Each lower post ends 0.25 mm into the inner skin at its centre. Nearer the wall, a vertical post pokes out through the curved outer skin, which the first draft did.
  - Upper posts run up to the upper skin.
  - M3 × 30 from below, through a counterbore whose floor is the post end.
- **Hub lugs:** posts at X 147.5 / 193.5, Y 292, with M3 × 40.
- **Ring pins:** one M3 × 12 per half along Z, at Y 246.5, into the ring inserts.
  - These replace draft 1's lug. That lug sat inside the master's port slot and hung on 0.2 mm skins.
- **Pins:** Ø5.5 × 1.2 pins on the lower posts locate the halves.

## Checks

**Mesh gate:** `ua_gate_run.py`, with `hardware/tools/mesh_check.py` and the whitelist in `prints/mesh_whitelist.py`. All five parts pass.

The whitelist holds:
- the master's own reviewed spots;
- four ring parting-line feathers, where the master hoop OD runs tangentially into the hoop-to-tube fillet;
- the oblique exits of the counterbores and the RS00 hole on the curved underside.

Each entry carries its reason.

**Support on visible faces** (`support_check.py`, judged against the assembled arm):

| Part | Visible faces needing support |
| --- | --- |
| Upper | None. The 84 mm² it flags are insert-hole ceilings and fuse-pocket faces inside the shell |
| Lower | 0.4 mm² |
| Ring | 0 |
| Hub | None visible. The 183 mm² it flags are the lug faces under the spigot, inside the closed arm |
| Clip | Prints with support off. Its overhangs are the short spigot cantilevers, hidden in the bores |

The RS02-side skirt is trimmed above Y 240; its tip was the one visible down-facing band.

**Seam FEA** (`ua_fea.py`, results in `gate/fea_seam.json`, `gate/seam_tractions.json`):

- **Model:** the db71b23b cases on a one-body model of the clamshell geometry: hub lightening, posts, skirt and ring trims, widened slot.
  - Load: remote load on the six carrier-screw holes in the ring.
  - Fixed: the hub face that clamps on the yaw RS02.
  - Bonded at the seam. The seam tractions are then read off to check the bolted joint.

| Case | Part SF away from load and fixture (yield 45 MPa) | Layer SF | Seam σz peak / mean tension | Seam shear mean / p95 |
| --- | --- | --- | --- | --- |
| LC1A (300 N, 17 N·m) | 3.49 | 3.47 | 0.42 / 0.07 MPa | 0.31 / 1.01 MPa |
| LC1B | 3.49 | 5.01 | 0.15 / 0.03 | 0.31 / 1.01 |
| LC1C (lateral) | 2.54 | 2.18 | 0.64 / 0.16 | 0.06 / 0.19 |
| LC2 (17 N·m twist) | **1.98** | 2.83 | 0.44 / 0.04 | 0.36 / 0.93 |

- **Joint load:** at most about 65 N of shear per side and under 30 N of opening.
- **Joint capacity:** two M3 posts per side at roughly 300 N preload after PETG creep, plus two Ø5.5 pins. The joint stays closed with margin.
- **Weakest spot:** LC2's 1.98 sits where the Y 262 upper post meets the upper skin, a square junction at 3 mm mesh. A 2 mm fillet there should lift it above 2. It is the next CAD change.
- One-piece db71b23b on its own fixture, for reference:

| Case | SF |
| --- | --- |
| LC1A | 2.23 |
| LC1C | 2.56 |
| LC2 | 4.24 |

## Elbow back loop

- **Clip:**
  - Saddle at 60°, r 48, in the loop plane X 132.
  - 8.4 mm U-groove, 8 mm flat, tilted 15° outward, R25 floor run-outs over 45°.
  - 4.5 × 2 mm cable-tie tunnel.
  - The block extends to w −12.5 so the run-out leaves no sliver.
- **Foot:** kept at r ≥ 31.5 to clear the forearm hub (draft 1 hit it by 1.2 mm), with cut-outs around the 6808 retainer heads.
- **Fit:**
  - Carrier penetration 0.2 mm (the crush ribs).
  - 1.35 mm or more from every screw head and washer.
  - Clear of the forearm by 2.5 mm or more through 0–124°.
- **Sizing** (`loop_sweep.py`, `loop_sweep_190.json`):
  - 190 mm free, bend radius at least 27.9 mm.
  - Bow about 70 mm at 0°.
  - Clearances: forearm 8.5 mm, RS02 26, carrier screws 16.
  - The nearest saddle edge is 1.7 mm away at 120–124°.
- **Retorque:** the clip pulls off by hand for the carrier-screw retorque. See `hardware/docs/assembly.md`.
- **Forearm saddle:** on the cheek rim at 140°. It comes with the forearm clamshell.

## Not in this draft

- Splice cradle: the splice sits loose in the tube, tied to the trunk relief.
- The 2 mm fillet at the upper post roots.
- Two-reviewer gate.
