# Left arm: one order for the upper arm and the elbow rig test

2026-11-07. **Fasteners and bearings only.** Everything else prints. This covers:

- the **upper arm** (`prints/upper-arm/`);
- the **elbow rig test** (`prints/test-pass-1/`, run before the forearm goes on).

Quantities are what's needed, with no spares. Small packs usually cover the rest. No listing has been checked for stock or price, apart from the 6808.

The rig and the arm share parts where a part comes out of the rig undamaged, so nothing is bought twice. After the rig, inspect anything you reuse: a bent screw, a stripped thread or a rough bearing gets replaced.

## 1. Bearings

| Part | Qty | Notes |
| --- | --- | --- |
| 6808-2RS, 40 × 52 × 7 | **2** | Ordered: HiPicco 2-pack, arriving Monday. One for the left arm and rig, one for the future right arm. If the rig roughens the first, the second becomes the left arm's |

The moving-parts audit adds no other bearing, bushing or thrust washer. See `MOVING_INTERFACES.md`.

## 2. Screws, nuts, washers and pins

| Part | Qty | Arm | Rig | Use |
| --- | --- | --- | --- | --- |
| M3 × 25 button head, ISO 7380, class 8.8+ | **6** | 6 | shared | Carrier and hoop into the elbow RS02, cut to 24.0 for the arm. Runs uncut in the rig nuts |
| DIN 9021 M3 washer, Ø9 × 0.8 | **9** | 9 | shared | 6 under the hoop screws, 3 under the retainers |
| M3 × 6 button head, thread-forming for plastic | **3** | 3 | shared | 6808 retainers |
| M3 × 6 button head | **4** | 4 | | Cable window cover |
| M3 heat-set insert, 4 mm long, Ø4 hole | **4** | 4 | | Cover bosses |
| M3 hex nut | **6** | | 6 | Rig housing pockets |
| M4 × 30 socket cap, DIN 912, class 8.8+ | **6** | 6 | | Top hub into the yaw RS02 |
| DIN 125 M4 washer, Ø9 × 0.8 | **12** | 12 | shared | Two under each top hub screw, and two under each fork sector screw in the rig |
| M4 × 16 button head, ISO 7380 | **6** | | 6 | Fork sector to the dummy output |
| M4 hex nut | **6** | | 6 | Dummy output slots |
| Ø4 × 8 steel dowel pin | **3** | | 3 | Dummy output to fork sector |
| M8 × 55 hex bolt, class 10.9, **fully threaded (ISO 4017)** | **1** | | 1 | Rig pivot and torque input |
| M8 nut | **2** | | 2 | Locked pair in the stub bore |
| M8 washer | **1** | | 1 | Housing face |
| M5 × 40 bolt and nut | **2** | | 2 | Lever in its socket |
| M6 bolts to suit your board | **4** | | 4 | Rig housing legs, Ø6.5 holes |

"Shared" means the rig borrows the arm's parts. If you build the arm while the rig is still running, you need a second set of those.

M4 × 14 button heads for the real fork come later, with the forearm.

## 3. Electrical (arm)

| Part | Qty | Notes |
| --- | --- | --- |
| Inline mini-blade fuse holder, 16 AWG leads | **2** | The pocket takes up to 12.5 mm wide; a 35 mm length is fine |
| Littelfuse 997 series 58 V mini-blade fuse, 5 A | **2** | Not the 32 V automotive kind. The robot bus is 54.6 V |
| Thin self-adhesive hook-and-loop tape | 1 strip | Holds the fuse holders |

The 14-way ribbon and the elbow cable guard are a separate small order, once the guard is final.

## 4. Consumables

| Part | Qty | Notes |
| --- | --- | --- |
| Medium thread-locker (Loctite 243 class) | 1 | Hoop and hub screws, M8 nuts. Use it sparingly: keep it off bare PETG, because it can crack it |

## 5. Tools

- 2 mm hex bit with a slim body, Ø6 or smaller, for the M3 button heads.
- 3 mm hex bit on a low-profile ratchet, for the top-hub M4s.
- Small torque screwdriver covering 0.3 to 0.6 N·m. The clamp torques are low on purpose.
- 13 mm deep socket and a torque wrench covering about 5 to 25 N·m (M8 jam nuts at 15, rig test 5 up to 20).
- Ø4 drill bit for clearing the dowel holes.
- 2020 extrusion, 600 mm, as the rig lever.
- Feeler gauges and calipers.
