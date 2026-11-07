# Pack cradle, rev A (spec for the SolidWorks model)

Joey 2026-11-06 16:32Z: printed cradle, side thumbscrews, no rear bar, room to add support later. Coordinates: torso frame
(X lateral, +X robot left; Y up from the pelvis bottom; Z forward), mm. Frame data: `../generator/parts.json`, `../README.md`.

## What it holds

13S2P pack box **|X| ≤ 47.5, Y 183 to 345, Z −45 to +34** (95 × 162 × 79, 2.2 kg). It slides out toward −Z through the frame rear
opening and the D2 door. Battery thread: one M4 heat-set insert in each pack side face at **Y 252.6, Z −5** (allowed band Z −20 to +10);
the thumbscrew may reach at most **6 mm** past the side face (cells behind), so it must have a shoulder or a length stop.

## Frame members it bolts to (aluminium, do not modify)

- Bottom side rails: |X| 55 to 75, Y 155 to 175, Z −50 to +50 (top face Y 175, T-slot centred at |X| 65).
- Shelf rails (shared step level): |X| 55 to 75, Y 353.62 to 373.62, Z −50 to +50 (bottom face Y 353.62, slot at |X| 65).
- Lower posts at |X| 55 to 75, Z ±50 to ±70 (not used; corner bracket heads near the post feet and tops).

## Part: cradle side, two off (left = mirror of right)

One printed piece per side, PETG, a C-shape seen from the front:

1. **Web**: plate |X| 47.5 to 51 (3.5 thick), Y 175 to 353.62, Z −50 to +37.5.
2. **Bottom foot**: |X| 51 to 75, Y 175 to 179, Z −44 to +37.5 (the rear bottom corner brackets sit on the post Z face at Z −50 to −46), lying on the bottom side rail top face. 2 × M5 slots (5.5 × 10, long
   along Z) at |X| 65, Z −30 and +20, into drop-in T-nuts.
3. **Top foot**: |X| 51 to 75, Y 349.62 to 353.62, Z −44 to +37.5, under the shelf rail. 2 × M5 slots at |X| 65, Z −30 and +20.
4. **Bottom ledge** (the pack rests on it): |X| 40 to 47.5, Y 179 to 183, Z −45 to +34.
5. **Front lip**: |X| 42 to 47.5, Y 183 to 345, Z 34 to 37.5. Stops the pack moving forward.
6. **Thumbscrew boss**: Ø16 boss on the web outer face at Y 252.6, Z −5, through hole for the thumbscrew; the boss length plus the
   web sets the stop so the tip ends ≤ 6 mm past |X| 47.5.
7. **Carrier pads**: two M3 heat-set insert bosses in the web front edge (Z 37.5 face) at Y 200 and 330, for the board carrier.
8. Lightening: oval windows in the web are welcome (V1 side exhaust, about 35 % open), keeping 6 mm around the boss, feet and pads.

Clearances to respect: the brake resistor plate (|X| ≤ 35, Y 154 to 184, Z 33.4 to 50) is inboard of the ledge, so no conflict;
the cable side band |X| 50 to 55, Z 42 to 50 stays free (the cradle ends at Z 37.5); corner bracket heads near Y 175 to 203 and
325 to 353 at the posts are outside |X| 55 except on the post Z faces, so keep the feet at Z ≤ 37.5 and ≥ −50.

## Part: top strap, one off

PETG strap 20 wide × 3 thick across the pack top at Y 345 to 348, Z −15 to +5, X −51 to +51, ends hooking over the two webs.
The pack power and sense harness passes above it (Y 348 to 353.6).

## Fasteners

8 × M5 × 10 button head + drop-in T-nut (feet), 2 × M4 thumbscrew with shoulder (length per item 6), 4 × M3 heat-set inserts
(carrier pads).

## Deliverables

SLDPRT for each part in `J:\code\marengo\cad\torso\`, STEP + STL in `hardware/designs/torso-frame/cradle/` (Git LFS), one
assembled render with the frame and pack. **No 3MF yet**: printed parts go through the gate review first (project rule).
