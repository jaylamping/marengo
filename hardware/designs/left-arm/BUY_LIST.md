# Left arm buy list, elbow and upper arm rev C

2026-11-06. Quantities are for one left arm, with no spares. Buy about 20 percent over on screws and washers. Sizes come from the rev C CAD after the gate fixes.

The first validation pass has no custom metal inserts and no steel stop. Simple bought hardware stays.

Supplier names are starting points. No listing below has been checked for stock, price or exact fit.

## Custom cut

These go to a laser or waterjet shop, because the hole positions are specific to this arm and no catalog part will fit. The files are in `dxf/`: `elbow_ring_84x66x2.dxf` and `hub_ring_32x16x2.dxf`. They are 1:1 in millimetres, circles, lines and arcs, so set the units to mm on upload and order 1 and 2 off. The elbow ring pattern is mirror symmetric, so it can go in either way up.

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Elbow ring | 1 | Ø84 OD, Ø66 ID, 2 mm steel, 6 × Ø3.4 holes on r 36.5 at 63.9, 103.9, 143.9, 223.9, 263.9 and 303.9 degrees, ribbon notch at 180 degrees, 19 wide to r 38. If already cut, file the notch by hand | Between carrier and hoop, X 149.7 to 151.7 |
| Hub ring | 2 | Ø32 OD, Ø16 ID, 2 mm steel, 6 × Ø4.5 holes on the Ø24 circle, 60 degrees apart | One on the flat at Y 298, one in the fork ring seat |

Search terms: "laser cut steel parts", "custom waterjet steel". SendCutSend and OSH Cut both take DXF uploads.

**Material:** 2 mm A36 or 1018 mild steel, or 304 stainless if you want no rust. Inch stock near 2 mm is fine, for example 14 gauge, about 1.9 mm. I couldn't confirm SendCutSend's current stock list, so choose the nearest offered at upload. Ask for the burrs to be removed, or deburr them by hand, because the rings clamp against printed faces.

**Cut rules:** SendCutSend's study guide gives 50 percent of thickness as the minimum hole and bridge width, so 1 mm here. All holes pass. The thinnest webs are 1.75 mm on the hub ring and 1.8 mm on the elbow ring. Those pass that minimum, but they are under SendCutSend's preferred bridge of 1 to 1.5 times thickness, and under the general advice of one hole diameter from an edge. Expect the quote check to accept them, and look at the hub ring webs for distortion when they arrive.

A plain Ø32 × Ø16 × 2 washer can stand in for the hub ring if the holes are drilled by hand on a fixture. The elbow ring has no such stand-in.

## Standard parts, cut to length

No catalog length matches. Buy the next size up and cut, or order a cut-to-length collar.

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Hoop sleeve | 6 | Ø5 OD, 3.2 ID, 13.5 long, steel | In the hoop, X 151.7 to 165.2 |
| Hub sleeve | 6 | Ø6 OD, 4.3 ID, 21.86 long, steel | In the top hub |

Search terms: "unthreaded spacer 5 mm OD M3", "unthreaded spacer 6 mm OD M4" at McMaster-Carr. At Misumi look for collars or spacers with a configurable length, which avoids cutting. All sleeve ends must be square, because they carry the screw clamp load instead of the PETG.

## Standard parts

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Bearing | 1 | 6808-2RS, 40 × 52 × 7 mm | Carrier seat on the fork stub |
| Hoop screw | 6 | Button head, M3 × 22, steel. Gives 5.0 mm in the RS02 thread. ISO 7380 doesn't usually list a 22, so if none is sold use M3 × 25 cut to 24, which gives 7.0 mm against the 8 mm depth | Carrier into the RS02 elbow housing |
| Hoop washer | 6 | DIN 125 M3, Ø7 × 0.5, steel | Under the hoop screw heads |
| Carrier spacer washer | 12 | DIN 433 M3, Ø6 × 0.5, steel | Two per screw in the Ø6.4 carrier hole, standing in for a 1 mm sleeve on the elbow ring |
| Retainer screw | 3 | Button head, M3 × 6, thread-forming for plastic | Holds the 6808 outer race, Ø2.7 pilot |
| Retainer washer | 3 | DIN 9021 M3, Ø9 × 0.8, steel | Under the retainer heads, clamps the race |
| Top hub screw | 6 | DIN 912 socket cap, M4 × 30, steel | Top hub into the RS02 yaw output |
| Fork output screw | 6 | ISO 7380 button head, M4 × 14, steel | Fork into the RS02 elbow output |
| Cover screw | 4 | M3, length to confirm | Cable window cover |
| RS00 screw | 6 | M3, length to confirm | Forearm tube to the RS00 rim, on a 50 mm circle |

Search terms: "6808-2RS", "ISO 7380 M3x25", "DIN 9021 M3". For the retainers, "thread forming screw for plastic M3" or "PT screw" will find close matches. Check that the head fits the Ø9.4 pocket. McMaster and Misumi are good for screws and washers. Amazon sells both, but check the bearing brand.

**Head types matter.** DIN 912 heads on the hoop or retainer screws collide with the fork. Use button heads there, as listed.

**6808 note.** The Ø49.5 floor bore was sized from EZO's Ø49 to 50 abutment for a 6808 2RS. Check the drawing of the bearing you actually buy.

## Tools

- 2 mm hex bit for the M3 button heads, with a body no larger than Ø6 to pass the fork access notch.
- Low-profile ratchet with a 3 mm hex bit for the top hub M4s. This is proven on the dummy first, because access is tight.
- Torque values are not set. They come from the dummy output test.

## Still open

- Cover screw and RS00 screw lengths, and whether the cover uses heat-set inserts.
- Elbow cable and fuse hardware, which are in `hardware/electrical/wiring/left-arm-trunk/README.md`.
