# Left arm: one order for the upper arm and the elbow rig test

2026-11-07. **Fasteners, bearings and consumables.** Everything else prints. This covers:

- the **upper arm** (`prints/upper-arm/`);
- the **elbow rig test** (`prints/test-pass-1/`, run before the forearm goes on).

Quantities are what's needed, with no spares, except where a row says so. Small packs usually cover the rest. No listing has been checked for stock or price, apart from the 6808.

The rig and the arm share parts where a part comes out of the rig undamaged, so nothing is bought twice. After the rig, inspect anything you reuse: a bent screw, a stripped thread or a rough bearing gets replaced.

## Upper arm clamshell, draft 1 (not released, buy when it passes review)

It replaces the one-piece upper arm's cable-window cover (drop the 4 × M3 × 6 cover screws and 4 cover inserts above). The carrier and its screws are unchanged.

| Part | Qty | Use |
| --- | --- | --- |
| M3 × 30 socket or button head | **4** | Seam posts, from below |
| M3 × 40 socket or button head | **2** | Hub-lug posts, from below |
| M3 × 12 socket or button head | **2** | Ring pins, one per half |
| Ruthex M3 × 5.7 heat-set insert (Ø4.0 hole) | **8** | 4 seam posts, 2 hub-lug posts, 2 in the ring |
| Cable tie, 3.6 mm | **3** | Loop clip, trunk entry, RS00 branch exit |

## 1. Bearings

| Part | Qty | Notes |
| --- | --- | --- |
| 6808-2RS, 40 × 52 × 7 | **2** | Ordered: HiPicco 2-pack, arriving Monday. One for the left arm and rig, one for the future right arm. If the rig roughens the first, the second becomes the left arm's |

The moving-parts audit adds no other bearing, bushing or thrust washer. See `MOVING_INTERFACES.md`. **Optional, don't buy yet:** a PTFE thrust washer, 40 ID, 0.5 thick, only if the rig shows the fork cheek closing its 0.8 mm gap in normal motion.

## 2. Screws, nuts, washers and pins

| Part | Qty | Arm | Rig | Use |
| --- | --- | --- | --- | --- |
| M3 × 25 button head, ISO 7380, class 8.8+ | **15** | 6 | 6 | Carrier and hoop into the elbow RS02, cut to 24.0 for the arm. Runs uncut in the rig nuts. Own set each, plus about 20% |
| DIN 9021 M3 washer, Ø9 × 0.8 | **22** | 9 | 9 | 6 under the hoop screws and 3 under the retainers, on each. Own set each, plus about 20% |
| DIN 6796 M3 conical spring washer | **15** | 6 | 6 | On each DIN 9021 under the hoop screws, cone toward the head. Own set each, plus about 20% |
| M3 × 6 button head, thread-forming for plastic | **3** | 3 | shared | 6808 retainers |
| M3 × 6 button head | **4** | 4 | | Cable window cover |
| M3 heat-set insert, 4 mm long, Ø4 hole | **4** | 4 | | Cover bosses |
| M3 hex nut | **6** | | 6 | Rig housing pockets |
| M4 × 30 socket cap, DIN 912, class 8.8+ | **6** | 6 | | Top hub into the yaw RS02 |
| DIN 125 M4 washer, Ø9 × 0.8 | **36** | 18 | 12 | Three under each top hub screw, two under each fork sector screw in the rig. Own set each, plus about 20% |
| M4 × 16 button head, ISO 7380 | **6** | | 6 | Fork sector to the dummy output |
| M4 hex nut | **6** | | 6 | Dummy output slots |
| Ø4 × 8 steel dowel pin | **3** | | 3 | Dummy output to fork sector |
| M8 × 55 hex bolt, class 10.9, **fully threaded (ISO 4017)** | **1** | | 1 | Rig pivot and torque input |
| M8 nut | **2** | | 2 | Locked pair in the stub bore |
| M8 washer | **1** | | 1 | Housing face |
| M5 × 40 bolt and nut | **2** | | 2 | Lever in its socket |
| M6 bolts to suit your board | **4** | | 4 | Rig housing legs, Ø6.5 holes |

"Shared" means the rig borrows the arm's parts. The clamp screws and washers are bought as two sets, so the arm and the rig can be built at the same time.

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
| Medium thread-locker (Loctite 243 class) | 1 | **Later, final assembly after testing. Not needed now.** Everything is fitted dry while it is being tested. Tip threads only when it is used: keep it off bare PETG, because it can crack it |
| Paint marker | 1 | Witness marks on every clamp screw for the retorques |

## 5. Tools

- 2 mm hex bit with a slim body, Ø6 or smaller, for the M3 button heads.
- 3 mm hex bit on a low-profile ratchet, for the top-hub M4s.
- Small torque screwdriver covering 0.3 to 0.6 N·m. The clamp torques are low on purpose.
- 13 mm deep socket and a torque wrench covering about 5 to 25 N·m (M8 jam nuts at 15, rig test 5 up to 20).
- Ø4 drill bit for clearing the dowel holes.
- 2.5 mm ball-end hex key (long arm), for the 6 forearm fork-rim M3 × 14 screws: a straight key can't reach them past the sleeve.
- 2020 extrusion, 600 mm, as the rig lever.
- Feeler gauges and calipers.

## 6. Forearm, support bearings (option A) and test weights

Added 2026-11-07 for forearm B with the 6808 support pair (`forearm/README.md`). **Quantities are for both arms** (left now, right later), except the test-weight hardware, which is one shared set. Joey is buying the 6808 10-pack, the inserts and the M8 hardware.

| Part | Per arm | Both arms | Use |
| --- | --- | --- | --- |
| 6808-2RS, 40 × 52 × 7 | 2 | **4** | Forearm support pair. The 10-pack also covers the elbow bearing of the right arm, fit checks and spares |
| Heat-set insert M3 × 5.7 (Ruthex / CNC Kitchen), Ø4.0 printed hole, 7 deep | 21 | **42** | Fork rim 6, sleeve bottom 3 (wrist cap), tube end 6 (wrist flange), wrist flange 6 (hand). Plus 12 for the four puck lids (shared). Buy a 100-pack for coupons and spares |
| M3 × 6 socket head, ISO 4762 | 12 | **24** | RS00 housing to the RS00 ring (6 × M3 on Ø50), and coupling plate to the RS00 output (6 × M3 on Ø27) |
| M3 × 14 socket head, ISO 4762 | 6 | **12** | Sleeve flange and RS00 ring into the fork rim inserts |
| M3 × 10 button head, ISO 7380 | 6 | **12** | Wrist flange into the tube end inserts |
| M3 × 6 button head, ISO 7380 | 3 | **6** | Wrist cap into the sleeve inserts |
| M5 × 20 button head, ISO 7380 | 1 | **2** | 6808 inner-race clamp through the fork stub |
| M5 nyloc nut, DIN 985 | 1 | **2** | Captured in the clamp boss |
| M5 washer, DIN 9021 (Ø15) | 1 | **2** | Under the clamp bolt head in the cheek cap |
| M8 × 100 hex bolt, fully threaded, ISO 4017 | | **1** | Test-weight stud, shared |
| M8 nut | | **2** | Locks the puck stack, plus one spare |
| M8 washer | | **1** | Under the nut |
| M3 × 10 socket head, ISO 4762 | | **6** | Test-weight adapter into the wrist flange inserts, shared |
| M3 × 8 socket head, ISO 4762 | | **12** | Puck lids, 3 per puck, for four pucks, into Ruthex inserts (taken from the insert 100-pack) |

All dry during testing, no thread-locker. Torques are set in the forearm assembly notes.
