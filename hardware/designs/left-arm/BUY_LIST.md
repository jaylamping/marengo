# Left arm buy list, elbow and upper arm rev C

2026-11-06. Quantities are for one left arm, with no spares. Buy about 20 percent over on screws and washers. Sizes come from the rev C CAD after the gate fixes.

The first validation pass has no custom metal inserts and no steel stop. Simple bought hardware stays.

Supplier names are starting points. No listing below has been checked for stock, price or exact fit.

## Custom cut

These need a DXF and a laser or waterjet shop. The hole positions are specific to this arm, so no catalog part will fit. **The DXFs are not made yet.**

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Elbow ring | 1 | Ø84 OD, Ø66 ID, 2 mm steel, 6 × M3 clearance holes on r 36.5 at 63.9, 103.9, 143.9, 223.9, 263.9 and 303.9 degrees | Between carrier and hoop, X 149.7 to 151.7 |
| Hub ring | 2 | Ø32 OD, Ø16 ID, 2 mm steel, 6 × M4 clearance holes on the Ø24 circle | One on the flat at Y 298, one in the fork ring seat |

Search terms: "laser cut steel parts", "custom waterjet steel". SendCutSend and OSH Cut both take DXF uploads. Use plain or zinc-plated mild steel, and ask for the burrs to be removed. Check that 2 mm stock is offered.

A plain Ø32 × Ø16 × 2 washer can stand in for the hub ring if the holes are drilled by hand on a fixture. The elbow ring has no such stand-in.

## Standard parts, cut to length

No catalog length matches. Buy the next size up and cut, or order a cut-to-length collar.

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Hoop sleeve | 6 | Ø5 OD, 3.2 ID, 13.5 long, steel | In the hoop, X 151.7 to 165.2 |
| Carrier sleeve | 6 | Ø5 OD, 3.2 ID, 2.5 long, steel | In the carrier, X 147.2 to 149.7, on the elbow ring |
| Hub sleeve | 6 | Ø6 OD, 4.3 ID, 21.86 long, steel | In the top hub |

Search terms: "unthreaded spacer 5 mm OD M3", "unthreaded spacer 6 mm OD M4" at McMaster-Carr. At Misumi look for collars or spacers with a configurable length, which avoids cutting. All sleeve ends must be square, because they carry the screw clamp load instead of the PETG.

## Standard parts

| Part | Qty | Spec | Where it goes |
| --- | --- | --- | --- |
| Bearing | 1 | 6808-2RS, 40 × 52 × 7 mm | Carrier seat on the fork stub |
| Hoop screw | 6 | ISO 7380 button head, M3 × 25, steel | Carrier into the RS02 elbow housing |
| Hoop washer | 6 | DIN 125 M3, Ø7 × 0.5, steel | Under the hoop screw heads |
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

- Elbow ring and hub ring DXFs.
- Cover screw and RS00 screw lengths, and whether the cover uses heat-set inserts.
- Elbow cable and fuse hardware, which are in `hardware/electrical/wiring/left-arm-trunk/README.md`.
