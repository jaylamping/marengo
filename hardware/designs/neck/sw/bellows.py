"""TPU bellows sizing: fold profile, stretch at the range corners, bending strain, clearance. No SolidWorks.

Fold model: per half fold, crest half-arc, straight flank, root half-arc on the wall centreline. Flanks stay straight
and arc length stays constant, so an arc tightens or opens as the fold angle changes. Bending strain at the arc is
t/2 * (1/R_now - 1/R_moulded). That overstates it where a real flank flexes too, and ignores hoop strain.
Run: python bellows.py [sweep]
"""
import itertools
import sys
from math import atan2, cos, degrees, hypot, pi, radians, sin, sqrt

import numpy as np

import neck_geom as g

FOOT_H, HEAD_H = 20.0, 72.0   # fold ends: top of the tube spigot, and a skirt hanging from the head plate, 8 below the U-joint
OD_R = 41.0


def fold(p, rr, rc, R):
    """Neutral half angle psi and flank length f for pitch p, centreline radii rr, rc, arc radius R."""
    d = rc - rr
    lo, hi = 0.05, pi / 2
    for _ in range(80):
        psi = (lo + hi) / 2
        # flank length from the axial balance, then test the radial one
        f = (p / 2 - 2 * R * sin(psi)) / cos(psi)
        if 2 * R * (1 - cos(psi)) + f * sin(psi) > d:
            hi = psi
        else:
            lo = psi
    f = (p / 2 - 2 * R * sin(psi)) / cos(psi)
    return psi, f


def shape(psi, a, f):
    """Half pitch and depth at half angle psi with arc length a per apex (2 half arcs) and flank f."""
    r = a / (2 * psi)
    return 2 * r * sin(psi) + f * cos(psi), 2 * r * (1 - cos(psi)) + f * sin(psi), r


def strain(p_now, psi0, a, f, R, t):
    """Bending strain and depth at local pitch p_now. Returns (strain, depth, arc radius, psi)."""
    lo, hi = 0.05, 2.2   # past 90 deg the fold bulges into a keyhole
    for _ in range(80):
        m = (lo + hi) / 2
        if shape(m, a, f)[0] * 2 > p_now:
            lo = m
        else:
            hi = m
    h, d, r = shape(m, a, f)
    return (t / 2) * abs(1 / r - 1 / R), d, r, m


def lengths(r, poses=None):
    """Chord length per circumferential angle, foot ring to head ring, over the range. Returns min, max."""
    lo, hi = 1e9, 0
    for p, q in (poses or g.corners()):
        Rh = g.head_rot(p, q)
        for th in np.linspace(0, 2 * pi, 180, endpoint=False):
            a = np.array([r * cos(th), r * sin(th), FOOT_H])
            b = g.UJ + Rh @ (np.array([a[0], a[1], HEAD_H]) - g.UJ)
            L = np.linalg.norm(b - a)
            lo, hi = min(lo, L), max(hi, L)
    return lo, hi


def evaluate(n, rr, R, t):
    """Worst strain and contact over the corners for n folds, root centreline rr, crest OD_R."""
    rc = OD_R - t / 2
    p = (HEAD_H - FOOT_H) / n
    if 4 * R >= p * 1.05:
        return None
    psi0, f = fold(p, rr, rc, R)
    if f <= 0.2:
        return None
    a = 2 * psi0 * R
    Lmin, Lmax = lengths(rc)
    s = []
    for L in (Lmin, Lmax):
        pn = L / n
        e, d, r, m = strain(pn, psi0, a, f, R, t)
        s.append((pn, e, d, r, degrees(m)))
    return dict(n=n, rr=rr, R=R, t=t, p=round(p, 2), psi0=round(degrees(psi0), 1), f=round(f, 2), Lmin=round(Lmin, 1), Lmax=round(Lmax, 1),
                comp=[round(v, 3) for v in s[0]], ext=[round(v, 3) for v in s[1]],
                arc_inner_comp=round(s[0][3] - t / 2, 2), overhang_deg=round(degrees(psi0), 1))


if __name__ == "__main__":
    for n, rr, R, t in itertools.product((3, 4, 5), (32.5, 33.0, 34.0), (1.2, 1.5, 2.0, 2.5, 3.0), (0.8, 1.0, 1.2)):
        v = evaluate(n, rr, R, t)
        if v:
            print(n, rr, "R", R, "t", t, "pitch", v["p"], "psi", v["psi0"], "flank", v["f"], "L", float(v["Lmin"]), float(v["Lmax"]),
                  "comp(p,e,depth,arc,psi)", [float(x) for x in v["comp"]], "ext", [float(x) for x in v["ext"]])
