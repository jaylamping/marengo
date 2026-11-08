# Upper arm clamshell, detailed CAD (draft 1, 2026-11-08)

Status: **parts built, not gated.** Still to come: the mesh gate, the support-on-visible-face check, seam FEA at the upper-arm load cases, picture sheets and the two-reviewer gate. Do not print these yet.

## Source

`ua_master.py` copies the db71b23b part (`cad/parts/left_arm/marengo_upper_arm_left.SLDPRT`). It suppresses the cable window and cover features, so the outer skin is continuous to ±0.03 mm. `ua_parts.py` then cuts the parts from that master. The helpers in `swg.py` work in arm coordinates, with no sketch-plane bookkeeping.

```
python ua_master.py build COVER_RECESS_B,WINDOW_ROUND,COVER_INSERTS_B,FLOOR_EXIT,CABLE_COVER_B,COVER_CLEAR_B,COVER_ENTRY,COVER_BORE,COVER_BORE2
python ua_parts.py ring hub upper lower carrier
```

Exports (STEP and STL, arm coordinates): `hardware/designs/left-arm/prints/upper-arm-clamshell/`.

## Parts

| Part | What it is | Solid | Sliced (P2 process, 6 walls, PETG) |
| --- | --- | --- | --- |
| `ua_upper` | Half above the seam (Z > 6). Four seam bosses with Ruthex M3 inserts, Ø6.4 pin sockets. Lug bosses with inserts at the two hub lugs and the ring lug | 39.9 cm³ | 55.1 g, 50% infill, 100% modifiers on the columns, the hub band (Y 286–303) and the ring capture |
| `ua_lower` | Half below the seam. Seam columns with Ø6 × 1.2 pins and counterbored M3 clearance holes, lug columns, ring column | 37.7 cm³ | 55.5 g, same zones |
| `ua_ring` | RS02 seat ring, X 150.8–166.2, r < 43.6. The plug-housing pocket runs through to the seat face over 161–191°, so the db71b23b lip is gone. The +Y lug is split by the seam and clamped by one long seam screw | 28.3 cm³ | 34.4 g, 100% |
| `ua_hub` | Hub disc. A 6 mm plate at the yaw RS02 face. Six plate windows between the M4 bosses, with the old tube floor opened. Spigot r 28–35.8 into the halves, two seam lugs | 52.6 cm³ | 66.4 g, 50%, 100% on the plate, bosses and spigot |
| `ua_carrier` | Carrier (R4_SPRING_SEAT body), now with the elbow back-loop tie-down saddle | 30.8 cm³ | 60.7 g, 100% (support under the saddle) |

**Weight against the one-piece arm.** Like for like, the slicer gives:

- four arm parts: **211.4 g**;
- the one-piece db71b23b arm: **229.0 g** (same slicer, same process, 50% with its clamp modifier). That arm weighed **202 g** on the scale.

That is 7.7% lighter, about 186 g expected on the scale. It is not the big drop the concept suggested. The concept's 162 g assumed the one-piece printed/solid ratio. These parts are small and thin-walled, so 6 walls plus shells make them almost solid: the hub slices at 66 g for 52.6 cm³ (1.26 g/cm³) whatever its infill.

The carrier went up 24 g with the saddle and its support (36.4 g before). Real savings now need geometry rather than infill. Candidates:
- hub plate 6 → 4 mm;
- 4 walls on the halves outside the clamp zones;
- a lighter saddle.

## Seam

- Z split at Z 6. Four M3 × 22 at the tube columns: (141.3, 262), (138.9, 278), (199.7, 262), (202.1, 278). The columns sit about 2 mm into the 3.3 mm wall.
- Two M3 through the hub lugs at (139, 292) and (202, 292).
- One about M3 × 40 through the ring lug at (158.5, 259.9).
- Pins Ø6 × 1.2 into Ø6.4 × 1.4 sockets at the four tube columns for alignment. Inserts are only in the upper half; screws go in from below.

## Elbow back loop (Joey's choice, 2026-11-08)

- Saddle on the carrier at 60°, r 48, loop plane X 132. `loop_anchor()` builds it:
  - 8.4 mm U-groove, 8 mm flat, tilted 15° outward;
  - R25 floor run-outs over 45°;
  - 4.5 × 2 mm cable-tie tunnel under the floor;
  - chamfered and bevelled ends;
  - a web back to the carrier rim.
- The matching forearm saddle (cheek rim, 140°) comes with the forearm clamshell.
- Sizing (`loop_sweep.py`, `loop_sweep_190.json`, run with `LOOP_ELASTICA=1 LOOP_NEW=1 LOOP_L=190 LOOP_PA=60 LOOP_PF=140 LOOP_ALPHA=15 LOOP_RMIN=36 LOOP_RBEND=3`):
  - 190 mm free, bend radius at least 27.9 mm through 0–124°;
  - bow at 0° about 70 mm past r 48;
  - clearance: forearm 8.5 mm, RS02 26, carrier screws 16, RS00 40, fork screws 65.
- The wiring side is in `hardware/electrical/wiring/left-arm-trunk/README.md`, S4.

## Not in this draft

- Trunk channel on the −X inner wall.
- Port routing to the elbow RS02.
- Fuse and splice cradles.
- Shoulder strain relief.

The fuse holders can sit in the tube with hook-and-loop until these are added.
