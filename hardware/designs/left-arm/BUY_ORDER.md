# Left arm: one order for the upper arm and the elbow rig test

2026-11-07. One list covering both jobs:

- the **upper arm** (`prints/upper-arm/`);
- the **elbow rig test** (`prints/test-pass-1/`, run before the forearm goes on).

The rig breaks parts, so it gets its own screws, washers and sleeves rather than borrowing the arm's. Quantities are exact. Add about 20% on screws, washers and nuts; small packs usually cover that anyway. No listing has been checked for stock or price.

Detail and reasons are in `BUY_LIST.md`, `prints/upper-arm/PARTS_LIST.md` and `prints/test-pass-1/PRINT_ORDER.md`. If they disagree with this list, this list is newer.

## 1. Laser-cut steel (SendCutSend, OSH Cut or similar)

Upload the DXFs from `dxf/`. Units are mm, the material is 2 mm mild steel or 304 stainless, and both parts should be deburred.

| Part | DXF | Qty | Arm | Rig | Fork (later) |
| --- | --- | --- | --- | --- | --- |
| Elbow ring Ø84 × Ø66 | `elbow_ring_84x66x2.dxf` (no notch) | **2** | 1 | 1 | |
| Hub ring Ø32 × Ø16 | `hub_ring_32x16x2.dxf` | **3** | 1 | 1 | 1 |

## 2. Bearings

| Part | Qty | Notes |
| --- | --- | --- |
| 6808-2RS, 40 × 52 × 7 | **2** | One for the arm and one for the rig. Buy a decent brand, and check its drawing for an abutment of about Ø49 to 50 |

## 3. Sleeves and spacers (McMaster, Misumi configurable collars)

| Part | Qty | Arm | Rig | Notes |
| --- | --- | --- | --- | --- |
| Ø5 OD × 3.2 ID steel sleeve, cut to **13.45** | **6** | 6 | | Square ends, 0.05 proud of the hoop |
| Ø5 OD × 3.2 ID steel sleeve, cut to **13.5** | **6** | | 6 | For the rig housing pockets. Buy one longer length and cut all 12 |
| Ø6 OD × 4.3 ID steel sleeve, cut to **21.86** | **6** | 6 | | Top hub |

## 4. Screws, nuts and washers (steel)

| Part | Qty | Arm | Rig | Use |
| --- | --- | --- | --- | --- |
| M3 × 25 button head, class 8.8+, cut to 24.0 | **6** | 6 | | Carrier into the elbow RS02 |
| M3 × 22 button head, class 8.8+ | **6** | | 6 | Carrier into the rig housing nuts |
| M3 × 6 button head, thread-forming for plastic | **6** | 3 | 3 | 6808 retainers |
| M3 × 6 button head | **4** | 4 | | Cable window cover |
| M3 hex nut | **6** | | 6 | Rig housing pockets |
| DIN 433 M3 washer, Ø6 × 0.5 | **24** | 12 | 12 | Two per carrier screw |
| DIN 125 M3 washer, Ø7 × 0.5 | **12** | 6 | 6 | One per carrier screw, under the head |
| DIN 9021 M3 washer, Ø9 × 0.8 | **6** | 3 | 3 | Under the retainers |
| M3 heat-set insert, 4 mm long, Ø4 hole | **4** | 4 | | Cover bosses (the two at Y 268 are being deepened to suit) |
| M4 × 30 socket cap, DIN 912, class 12.9 | **6** | 6 | | Top hub into the yaw RS02 |
| M4 × 16 button head, ISO 7380 | **6** | | 6 | Fork sector to the dummy output |
| M4 × 14 button head, ISO 7380 | **6** | | | Real fork to the elbow RS02, for later, with the forearm |
| M4 hex nut | **6** | | 6 | Dummy output slots |
| Ø4 × 8 steel dowel pin | **3** | | 3 | Dummy output to fork sector |
| M8 × 55 hex bolt, class 10.9, **fully threaded (ISO 4017)** | **1** | | 1 | Rig pivot and torque input |
| M8 nut | **2** | | 2 | Locked pair in the stub bore |
| M8 washer | **1** | | 1 | Housing face |
| M5 × 40 bolt and nut | **2** | | 2 | Lever in its socket |
| M6 bolts and spacers to suit your board | **4** | | 4 | Rig housing legs, Ø6.5 holes |

## 5. Electrical (arm)

| Part | Qty | Notes |
| --- | --- | --- |
| Inline mini-blade fuse holder, 16 AWG leads | **2** | The pocket takes up to 12.5 mm wide; a 35 mm length is fine |
| Littelfuse 997 series 58 V mini-blade fuse, 5 A | **2** | Not the 32 V automotive kind. The robot bus is 54.6 V |
| Thin self-adhesive hook-and-loop tape | 1 strip | Holds the fuse holders |

The 14-way ribbon and the elbow cable guard are a separate small order, once the guard is final.

## 6. Adhesives and consumables

| Part | Qty | Notes |
| --- | --- | --- |
| Medium thread-locker (Loctite 243 class) | 1 | Hoop and hub screws, M8 nuts |
| **Slow-cure epoxy** (30 min or longer) | 1 | Bonds the hoop sleeves. Not Loctite 638, which cures poorly against PETG and can crack it |

## 7. Tools

- 2 mm hex bit with a slim body, Ø6 or smaller, for the M3 button heads.
- 3 mm hex bit on a low-profile ratchet, for the top-hub M4s.
- Small torque screwdriver covering 1.0 and 2.5 N·m (hoop and hub screws).
- 13 mm deep socket and a torque wrench covering about 5 to 25 N·m (M8 jam nuts at 15, rig test 5 up to 20).
- Ø4 and Ø6.2 drill bits, for clearing the dowel and hub holes.
- 2020 extrusion, 600 mm, as the rig lever.
- Feeler gauges and calipers.
