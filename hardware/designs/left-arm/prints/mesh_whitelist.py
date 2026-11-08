"""Known design corners and hidden thin spots that tools/mesh_check.py may pass, per part.

Coordinates are CAD coordinates of the source STL (arm frame, mm) unless the part is generated in the
build script (seat_ring_only, layer_coupons, dummy_output), where they are plate coordinates.
Every entry needs a reason. Add entries only after looking at the spot in a render and in the CAD.
"""
WHITELIST = {
    'upper_arm': {
        'revolve': {
            'center': (170.49, 5.99), 'y_range': (254.9, 319.0),
            'boxes': [
                {'ang': (49.0, 125.0), 'y': (254.9, 257.0), 'label': 'collar + web end', 'reason': 'collar block and hoop web running 2 mm onto the tube, with the r1.5 root fillet; structural, several mm wide'},
                {'ang': (235.0, 311.0), 'y': (254.9, 257.0), 'label': 'web + collar end', 'reason': 'hoop web and collar block running onto the tube, other side; structural'},
                {'ang': (237.0, 303.0), 'y': (262.0, 310.0), 'label': 'cover recess', 'reason': 'cover recess and cable window, closed by the printed cover'},
                {'ang': (165.0, 195.0), 'y': (298.0, 319.5), 'label': 'bed flat', 'reason': 'flat at X 131.99 that the arm prints on'},
            ],
        },
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
    'carrier': {
        'thin': [
            {'box': ((140.0, 229.9, -10.7), (143.7, 231.4, -9.1)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 182.9, 3.6), (143.7, 184.3, 5.2)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 183.3, 10.3), (143.7, 184.8, 11.7)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 233.3, -4.8), (143.7, 234.9, -3.6)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 223.5, -16.6), (143.7, 225.1, -15.3)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 217.8, -19.7), (143.7, 219.0, -18.1)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 182.7, 3.0), (143.7, 184.4, 5.2)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 217.6, -19.9), (143.7, 219.0, -18.1)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 223.5, -16.7), (143.7, 225.3, -15.4)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 230.3, -10.9), (143.7, 231.5, -9.1)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 183.2, 10.3), (143.7, 184.7, 11.9)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
            {'box': ((140.0, 233.3, -4.8), (143.7, 235.1, -3.6)), 'reason': 'C2_RET retainer pocket circle crossing the 6808 seat bore: a cusp that tapers to zero over about 1 mm, inside the seat, not a free blade'},
        ],
    },
    'p2': {
        'thin': [
            {'box': ((-111.5, 17.5, -73.5), (-108.9, 19.0, -55.0)), 'reason': 'keyhole: the circle meets the slot edge at a shallow angle, a vertical cusp tapering to zero inside the keyhole; P2 geometry belongs to the P2 thread'},
            {'box': ((-92.0, 33.9, -66.5), (-89.9, 35.8, -63.5)), 'reason': 'hole cusp, 1.3 mm2, 2 mm tall, inside the hole; P2 geometry belongs to the P2 thread'},
            {'box': ((-68.7, 38.6, -66.5), (-67.3, 40.3, -63.5)), 'reason': 'hole cusp, 0.5 mm2, inside the hole; P2 geometry belongs to the P2 thread'},
        ],
    },
    'fork_sector': {
        'corners': [
            {'pt': (131.75, 150.0, 4.49), 'reason': 'end of the 0.05 mm step where TRANSITION meets YOKE on the outboard face, below layer height, not a blade (fold check passes)'},
            {'pt': (131.75, 150.0, -20.01), 'reason': 'other end of the same 0.05 mm step'},
        ],
        'thin': [
            {'box': ((149.6, 145.5, -26.5), (152.0, 147.0, -22.5)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((145.9, 149.0, -26.5), (156.5, 150.5, -25.1)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((198.0, 149.5, -25.9), (204.9, 150.5, -19.5)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((175.8, 149.0, -26.5), (184.4, 150.5, -25.2)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((179.5, 145.5, -26.5), (181.7, 147.1, -24.0)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((132.1, 149.5, -26.3), (135.5, 150.5, -24.3)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((131.2, 149.5, -25.6), (133.5, 150.5, -19.5)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
            {'box': ((135.5, 149.4, -26.5), (138.9, 150.5, -25.1)), 'reason': 'skin where the TRANSITION loft grazes the YOKE flat at Z -26 or the GUSSET_IN end, 0.03 to 0.08 mm, sits on the surface, not proud; the slicer drops it'},
        ],
    },
    'seat_ring_only': {'bodies': 2},
    'layer_coupons': {'bodies': None},
    'dummy_output': {'bodies': 2},
}
# upper-arm clamshell (hardware/designs/left-arm/upper-arm/clamshell/cad), cut from the db71b23b master: the entries
# below are the master's own reviewed spots that land in each part, plus one ring end corner.
WHITELIST['ua_ring'] = {
    'corners': WHITELIST['upper_arm']['corners'][:2] + [
        {'pt': (166.2, 233.65, 39.38), 'reason': 'ring +X end face meets the hoop outer surface at 167 deg (a gentle crease, not a spike), on the RS02 side under the halves skirt'},
    ],
    'thin': WHITELIST['upper_arm']['thin'][2:4] + [
        {'box': ((162.0, 174.0, 17.4), (165.4, 175.3, 18.2)), 'reason': 'same port/flange relief corner, its far end, under the RS02 flange, not visible'},
        {'box': ((151.5, 248.3, 27.0), (166.4, 248.9, 27.7)), 'reason': 'parting-line feather: the master hoop OD runs into the hoop-to-tube fillet tangentially, so any ring/halves cut radius grazes it; 0.16 mm, under the halves, prints as nothing'},
        {'box': ((151.5, 248.3, -15.8), (166.4, 248.9, -15.1)), 'reason': 'same parting-line feather, back side'},
        {'box': ((151.5, 250.9, -10.3), (166.4, 251.5, -9.3)), 'reason': 'widened port-slot wall meets the hoop OD fillet, 0.58 mm, inside the slot under the halves'},
        {'box': ((165.3, 232.6, -27.6), (166.4, 234.4, -26.4)), 'reason': 'the master RS02-side skirt thin spot (0.35 mm in the master too) where it reaches the ring end face'},
    ],
}
WHITELIST['ua_upper'] = {'corners': [WHITELIST['upper_arm']['corners'][2]]}
WHITELIST['ua_lower'] = {'thin': [{'box': ((151.2, 252.2, -7.5), (152.0, 253.1, -1.5)), 'reason': WHITELIST['upper_arm']['thin'][4]['reason']},
                                   {'box': ((143.6, 259.8, -17.6), (145.0, 264.2, -15.0)), 'reason': 'oblique counterbore exit: the Ø6.5 head pocket leaves the curved underside skin at a shallow angle on the outboard side, a wedge of about 2 mm2, not a free blade'},
                                   {'box': ((143.6, 276.2, -20.2), (145.0, 279.0, -18.6)), 'reason': 'same counterbore exit, Y 278 post'},
                                   {'box': ((196.0, 259.8, -17.6), (197.4, 264.2, -15.0)), 'reason': 'same counterbore exit, +X side'},
                                   {'box': ((196.0, 276.2, -20.2), (197.4, 279.0, -18.6)), 'reason': 'same counterbore exit, +X side, Y 278 post'},
                                   {'box': ((145.5, 267.8, -20.6), (148.6, 270.6, -19.0)), 'reason': 'acute lip where the Ø7.2 RS00-branch exit (along X) leaves the back wall, which leans 51 deg: a wedge under 1 mm2 at the bottom of the hole mouth, not a free blade'}], 'corners': [dict(c, reason=c['reason'].replace('under the cover', 'now inside the closed shell, not visible')) for c in WHITELIST['upper_arm']['corners'][3:5]]}

WHITELIST['elbow_carrier'] = WHITELIST['elbow_carrier_strength'] = WHITELIST['carrier']  # same mesh as the arm carrier
