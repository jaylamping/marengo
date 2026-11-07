"""Known design corners and hidden thin spots that tools/mesh_check.py may pass, per part.

Coordinates are CAD coordinates of the source STL (arm frame, mm) unless the part is generated in the
build script (seat_ring_only, layer_coupons, dummy_output), where they are plate coordinates.
Every entry needs a reason. Add entries only after looking at the spot in a render and in the CAD.
"""
WHITELIST = {
    'upper_arm': {
        'corners': [
            {'pt': (151.7, 230.09, 43.48), 'reason': 'hoop outer circle meets the straight web edge on the carrier face, near-tangent corner'},
            {'pt': (151.7, 230.09, -31.5), 'reason': 'same corner, other side'},
            {'pt': (185.48, 280.52, 36.49), 'reason': '0.02 mm step between FUSE2_WALLS and FUSE_FILL on the inner fuse-bay wall, inside the tube, below slicer resolution'},
            {'pt': (186.24, 264.07, -18.1), 'reason': 'window frame corner meeting the inner taper at a grazing angle, under the cover'},
            {'pt': (188.04, 271.08, -18.1), 'reason': 'window frame corner meeting the inner taper at a grazing angle, under the cover'},
            {'pt': (153.05, 307.17, -21.99), 'reason': 'cover recess end corner against the outer taper, under the cover'},
            {'pt': (187.93, 307.17, -21.99), 'reason': 'cover recess end corner against the outer taper, under the cover'},
            {'pt': (157.93, 307.17, -24.49), 'reason': 'cover recess end corner against the inner taper, under the cover'},
            {'pt': (183.05, 307.17, -24.49), 'reason': 'cover recess end corner against the inner taper, under the cover'},
        ],
        'thin': [
            {'box': ((159.0, 270.0, -30.0), (161.0, 299.0, -26.0)), 'reason': 'cover recess side edge where the flat recess meets the taper at a grazing angle, under the cover'},
            {'box': ((180.5, 270.0, -30.0), (182.0, 299.0, -26.0)), 'reason': 'cover recess side edge, other side, under the cover'},
            {'box': ((162.0, 172.5, -1.5), (164.0, 176.0, -0.5)), 'reason': 'corner where the merged port and flange reliefs meet, under the RS02 flange, not visible'},
            {'box': ((162.0, 172.5, 17.0), (164.0, 176.0, 18.0)), 'reason': 'same, upper relief'},
            {'box': ((151.6, 252.3, -7.5), (152.0, 252.7, -1.5)), 'reason': '0.2 mm ROOT_EASE back face inside the port slot, one layer, not a blade'},
        ],
    },
    'seat_ring_only': {'bodies': 2},
    'layer_coupons': {'bodies': None},
    'dummy_output': {'bodies': 2},
}
