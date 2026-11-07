# Left arm buy list, elbow and upper arm rev C

2026-11-07. Sizes come from the rev C CAD after the printed joint change.

The first validation pass prints everything except fasteners and bearings.

Supplier names are starting points. No listing below has been checked for stock, price or exact fit.

## Order

`BUY_ORDER.md` is the order: fasteners and bearings only. No steel rings or sleeves for this validation pass, because everything else prints. The ring DXFs in `dxf/` are a future option.

| Part | Spec |
| --- | --- |
| Bearing | 6808-2RS, 40 × 52 × 7 mm |
| Hoop screw and washer | Button head M3 × 25 cut to 24.0, DIN 9021 M3. 7.2 mm into the 8 mm RS02 thread |
| Top hub screw and washers | DIN 912 M4 × 30, two DIN 125 M4. 6.6 mm into the 7 mm RS02 thread |
| Fork output screw and washers, later | ISO 7380 M4 × 14, with two DIN 125 M4 in the fork seat. Check the thread depth before buying |
| RS00 screw, later | M3, length to confirm, forearm tube to the RS00 rim on a 50 mm circle |

**Head types matter.** DIN 912 heads on the hoop or retainer screws collide with the fork. Use button heads there, as listed.

**6808 note.** The Ø49.5 floor bore was sized from EZO's Ø49 to 50 abutment for a 6808 2RS. Check the drawing of the bearing you actually buy.

## Tools

- 2 mm hex bit for the M3 button heads, with a body no larger than Ø6 to pass the fork access notch.
- Low-profile ratchet with a 3 mm hex bit for the top hub M4s. This is proven on the dummy first, because access is tight.
- Torque screwdriver covering 0.3 to 0.6 N·m. Values are in `prints/upper-arm/ASSEMBLY_ORDER.md`.

## Still open

- RS00 screw length.
- Elbow cable and fuse hardware, which are in `hardware/electrical/wiring/left-arm-trunk/README.md`.
