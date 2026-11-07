"""Power board carrier, rev B (one 100 x 150 board). build123d, torso coordinates (X lateral, Y up, Z forward), mm.

Plate |X| <= 55, Y 190 to 340, Z 37.5 to 41.5 (4 mm PETG). Rear face sits on the cradle carrier pads (|X| 48 to 55.5,
Y 195-205 and 325-335, front face Z 37.5) and is screwed to their M3 heat-set inserts at (+-51.75, 200) and (+-51.75, 330)
with M3 x 8 countersunk screws from the front (flush, clear of the side cable band |X| 50-55, Z 42-50).
Board standoffs (rev B floorplan, LAYOUT_NOTES): 6 x M3 F-F 6 mm hex standoffs at (+-46, 194), (+-46, 265), (+-46, 336),
fixed by M3 x 6 button heads from the rear, heads in a 2 mm counterbore so the rear face stays flat (3 mm air gap to the
pack sheet). Two lightening windows between the standoff columns. Run: python3 build_carrier.py
"""
from build123d import *
import os

T, Z0 = 4.0, 37.5
X0, X1, Y0, Y1 = -55.0, 55.0, 190.0, 340.0
CARRIER_SCREWS = [(sx * 51.75, y) for sx in (1, -1) for y in (200.0, 330.0)]
STANDOFFS = [(sx * 46.0, y) for sx in (1, -1) for y in (194.0, 265.0, 336.0)]
WINDOWS = [(-34, 34, 206, 256), (-34, 34, 274, 324)]  # x0, x1, y0, y1, R6 corners

# Sketch in the XY plane, extrude along +Z (= torso Z, toward the front).
with BuildPart() as p:
    with BuildSketch(Plane.XY.offset(Z0)):
        with Locations(((X0 + X1) / 2, (Y0 + Y1) / 2)):
            RectangleRounded(X1 - X0, Y1 - Y0, 4)
        for x0, x1, y0, y1 in WINDOWS:
            with Locations(((x0 + x1) / 2, (y0 + y1) / 2)):
                RectangleRounded(x1 - x0, y1 - y0, 6, mode=Mode.SUBTRACT)
    extrude(amount=T)
    front = Plane.XY.offset(Z0 + T)
    rear = Plane.XY.offset(Z0).reverse()  # normal -Z, so holes cut toward +Z
    # carrier screws: countersunk from the front (M3 90 deg, head 6.0)
    with Locations(*[front * Pos(x, y) for x, y in CARRIER_SCREWS]):
        CounterSinkHole(radius=1.7, counter_sink_radius=3.1, depth=T)
    # standoff screws: counterbored from the rear (button head 5.7 x 1.65)
    with Locations(*[rear * Pos(x, -y) for x, y in STANDOFFS]):
        CounterBoreHole(radius=1.7, counter_bore_radius=3.25, counter_bore_depth=2.0, depth=T)

part = p.part
here = os.path.dirname(os.path.abspath(__file__))
export_step(part, os.path.join(here, 'marengo_torso_board_carrier.step'))
export_stl(part, os.path.join(here, 'marengo_torso_board_carrier.stl'))
bb = part.bounding_box()
print('bbox', [round(v, 2) for v in (bb.min.X, bb.min.Y, bb.min.Z, bb.max.X, bb.max.Y, bb.max.Z)])
print('volume mm3', round(part.volume, 1), 'mass g @1.27', round(part.volume * 1.27e-3, 1))
