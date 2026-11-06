# Left upper arm, round tube v1

2026-11-05. Baseline version, not reviewed and not released for printing or load. The CAD stays on Joey-PC.

The arm carries the yaw RS02 output at the top and holds the elbow RS02 housing at the bottom. Results and the load test are in [BASELINE.md](BASELINE.md).

- Section: round, on the arm axis, 78.5 mm OD at the top tapering to 69 mm at the elbow, with a 3 mm wall. The top matches the RS02 flange.
- Cables: 14 mm bore through the top plate, the tube as the channel, a rear window, and a slot to the upper RS02 port.
- Cover: curved to match the tube and held by 4 M3 screws into inserts.
- Joints: M4 and M3 clearance holes sized for compression limiters. Inserts are specified, not modelled.
- Elbow support: bolted carrier with a stub for a 6808 bearing (40 × 52 × 7 mm), hollow for cables.
- Stops: elbow lugs on the carrier for 0° and 135°. They need the forearm fork to act against.
- RS02 fit: relief for the lower connector and two flange bosses. CAD interference with the RS02 is zero.

Mass is about 177 g as solid PETG for the arm, carrier and cover.

Fold clearance at 135° is 6.7 mm to the RS00, or 3.7 mm once the forearm has a 3 mm shell, so no flat is needed on the tube.

Open items:

- Fails the fixed load case at the neck, top plate and window. See BASELINE.md.
- The shoulder roll stop is not here: it belongs on P1 and P2, at the roll joint. See `../envelope/README.md` for the limit.
- No bearing retainer or forearm fork yet.
- The lower RS02 port is enclosed, so only the upper port is usable.
