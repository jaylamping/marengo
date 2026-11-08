"""Back loop sweep: loop centreline (8 mm loom) vs upper arm, carrier, elbow RS02, carrier screws, forearm, RS00 through
elbow 0-124 deg, for candidate anchor X planes. usage: python loop_sweep.py [x ...]"""
import sys, json
import os
xs = [float(a) for a in sys.argv[1:] if not a.endswith(('.png', '.STL'))] or [141.0, 146.0, 152.0]
KW = dict(L=float(os.environ.get('LOOP_L', 148)), pa=float(os.environ.get('LOOP_PA', 70)), pf=float(os.environ.get('LOOP_PF', 110)))
sys.argv = ['x']
import numpy as np, trimesh
import elbow_diagram as E
from scipy.optimize import brentq
CY, CZ = 209.860668, 5.985875
B = {p['key']: p['mesh'] for p in E.build()}
FB = trimesh.load('stl/fa/forearm_b.STL', force='mesh')
RS00 = B['rs00']


def rot(m, th):
    T = trimesh.transformations.rotation_matrix(-np.radians(th), [1, 0, 0], [0, CY, CZ])
    q = m.copy(); q.apply_transform(T); return q


def P(r, phi, x):
    return np.array([x, CY + r * np.cos(phi), CZ - r * np.sin(phi)])


def back_loop(th, L=148.0, x=141.0, r0=48.0, pa=70.0, pf=110.0):
    """Anchors r0 at pa (upper arm) and pf + th (forearm); sine bulge sized to free length L, floor r 44."""
    pA, pF = np.radians(pa), np.radians(pf + th)
    phis = np.linspace(pA, pF, 60)
    base = lambda h: [P(r0 + h * np.sin(np.pi * (p - pA) / (pF - pA)), p, x) for p in phis]
    length = lambda h: np.sum(np.linalg.norm(np.diff(np.array(base(h)), axis=0), axis=1))
    h = brentq(lambda h: length(h) - L, 44 - r0, 200) if length(44 - r0) < L else 44 - r0
    return np.array(base(h)), h + r0 - 44, P(r0, pA, x), P(r0, pF, x)



def arc_loop(th, L=148.0, x=141.0, r0=48.0, pa=70.0, pf=110.0):
    """Free loop as a circular arc of length L through the anchors, bulging away from the axis (exits free to tilt)."""
    a, b = P(r0, np.radians(pa), x), P(r0, np.radians(pf + th), x)
    c = np.linalg.norm(b - a)
    t = brentq(lambda t: L * np.sin(t) / t - c, 1e-4, np.pi - 1e-4); rho = L / (2 * t)
    u = (b - a) / c; mid = (a + b) / 2; n = mid - np.array([x, CY, CZ]); n -= (n @ u) * u; n /= np.linalg.norm(n)
    ctr = mid - n * rho * np.cos(t)
    pts = np.array([ctr + rho * (np.sin(k) * u + np.cos(k) * n) for k in np.linspace(-t, t, 60)])
    return pts, rho, rho * (1 - np.cos(t)), np.degrees(np.pi - t) if t > np.pi / 2 else np.degrees(t)


def frame(phi_deg, sense, alpha=45.0, r0=48.0):
    """Clamp centre (Y, Z) and groove direction toward the free loop (same construction as ua_parts.anchor_frame)."""
    f, a = np.radians(phi_deg), np.radians(alpha)
    C = np.array([CY + r0 * np.cos(f), CZ - r0 * np.sin(f)])
    t = sense * np.array([-np.sin(f), -np.cos(f)]); n = np.array([np.cos(f), -np.sin(f)])
    return C, np.cos(a) * t + np.sin(a) * n


def elastica(th, L=148.0, x=141.0, pa=70.0, pf=110.0, flat=4.0, N=48, r_min=None, alpha=None, free_deg=20.0, R_bend=25.0):
    """Minimum bending-energy planar loom of free length L between the clamp flats. The heading at each clamp exit may
    swing +-free_deg (the R25 floor run-out on one side, lift-off past the tie on the other); bend radius >= R_bend
    everywhere; centreline kept 4 mm outside r_min. Returns 3D points (incl. clamp centres) and the min bend radius."""
    from scipy.optimize import minimize
    alpha = float(os.environ.get('LOOP_ALPHA', 45)) if alpha is None else alpha
    r_min = float(os.environ.get('LOOP_RMIN', 44)) if r_min is None else r_min
    R_bend = float(os.environ.get('LOOP_RBEND', R_bend))
    CA, uA = frame(pa, +1, alpha); CF, uF = frame(pf + th, -1, alpha)
    A, F = CA + flat * uA, CF + flat * uF
    h = L / N
    a0, a1 = np.arctan2(uA[1], uA[0]), np.arctan2(-uF[1], -uF[0])
    def path(q):
        return A + np.vstack([[0, 0], np.cumsum(h * np.c_[np.cos(q), np.sin(q)], axis=0)])
    def energy(q):
        return np.sum(np.diff(q) ** 2) / h
    cons = [{'type': 'eq', 'fun': lambda q: path(q)[-1] - F},
            {'type': 'ineq', 'fun': lambda q: np.hypot(path(q)[:, 0] - CY, path(q)[:, 1] - CZ) - (r_min + 4.0)},
            {'type': 'ineq', 'fun': lambda q: (h / R_bend) ** 2 - np.diff(q) ** 2}]
    best = None
    for guess in ('arc', 'wrap'):
        g = (arc_loop(th, L=L, x=x, pa=pa, pf=pf)[0] if guess == 'arc' else back_loop(th, L=L, x=x, pa=pa, pf=pf)[0])[:, 1:]
        d = np.diff(g, axis=0); hd = np.unwrap(np.arctan2(d[:, 1], d[:, 0]))
        q0 = np.interp(np.linspace(0, 1, N), np.linspace(0, 1, len(hd)), hd)
        q0 = q0 + np.round((a0 - q0[0]) / (2 * np.pi)) * 2 * np.pi
        a1u = a1 + np.round((q0[-1] - a1) / (2 * np.pi)) * 2 * np.pi
        fd = np.radians(free_deg)
        def bnd(a, C, u):      # the floor allows no inward swing past the flat (its R25 run-out is the bend limit); lift-off outward is free
            n_ = np.array([C[0] - CY, C[1] - CZ]); n_ /= np.linalg.norm(n_)
            w_ = n_ - (n_ @ u) * u; sg = np.sign(u[0] * w_[1] - u[1] * w_[0])
            return (a, a + 2 * fd) if sg > 0 else (a - 2 * fd, a)
        bnds = [bnd(a0, CA, uA)] + [(None, None)] * (N - 2) + [bnd(a1u, CF, uF)]
        r = minimize(energy, q0, constraints=cons, bounds=bnds, method='SLSQP', options=dict(maxiter=600, ftol=1e-10))
        ok = r.success and np.abs(path(r.x)[-1] - F).max() < 0.05
        if ok and (best is None or r.fun < best.fun):
            best = r
    if best is None:
        return None, None
    rb = float(h / max(np.abs(np.diff(best.x)).max(), 1e-9))
    full = np.vstack([CA, path(best.x), CF])
    return np.c_[np.full(len(full), x), full], rb


def loop(th, x):
    if os.environ.get('LOOP_ELASTICA'):
        pts, rb = elastica(th, x=x, **KW)
        if pts is not None:
            r = np.hypot(pts[:, 1] - CY, pts[:, 2] - CZ)
            return pts, dict(model='elastica', bend_r=round(rb, 1), bow=round(float(r.max() - 48), 1))
        print('  elastica failed at', th, flush=True)
    pts, rho, bow, ex = arc_loop(th, x=x, **KW)
    r = np.hypot(pts[:, 1] - CY, pts[:, 2] - CZ)
    if r.min() >= 44.0:
        return pts, dict(model='arc', bend_r=round(rho, 1), bow=round(bow, 1), exit_deg=round(ex, 1))
    pts, bulge, a, f = back_loop(th, x=x, **KW)
    return pts, dict(model='wrap', bend_r=44.0, bow=round(float(bulge), 1))


def main():
    R_LOOM = 4.0
    parts = {'upper arm': B['arm'], 'carrier': B['carrier'], 'RS02': B['rs02e']}
    if os.environ.get('LOOP_NEW'):
        parts['upper arm'] = trimesh.util.concatenate([trimesh.load(f'stl/ua/ua_{k}.STL', force='mesh') for k in ('upper', 'lower', 'ring', 'hub')])
        parts['carrier'] = trimesh.load('stl/ua/ua_carrier.STL', force='mesh')
    parts.update({'carrier screws': B['hs'], 'spring washers': B['hc'], 'washers': B['hw'], '6808': B['brg']})
    print('static parts', list(parts), flush=True)
    pq = {k: trimesh.proximity.ProximityQuery(m) for k, m in parts.items()}
    out = {}
    for x in xs:
        rows = []
        for th in list(range(0, 121, 8)) + [124]:
            pts, info = loop(th, x)
            mid = pts[3:-3]                       # clamps hold the ends
            if info.get('model') == 'elastica':   # skip the clamp flats and the first 12 mm of run-out
                k0 = 1 + int(12 / (KW['L'] / 48))
                mid = pts[k0:-k0]
            row = {'th': th, **info}
            for k, q in pq.items():
                sd = q.signed_distance(mid)
                row[k] = round(float(-sd.max()) - R_LOOM, 1)   # >0 = clear
                if k == 'carrier' and row[k] < 0:
                    pi = int(sd.argmax()); row['carrier_at'] = [round(v, 1) for v in mid[pi]] + [round(float(np.degrees(np.arctan2(-(mid[pi][2] - CZ), mid[pi][1] - CY))), 1)]
            for k, m in (('forearm', rot(FB, th)), ('RS00', rot(RS00, th)), ('fork screws', rot(B['fb'], th))):
                row[k] = round(float(-trimesh.proximity.ProximityQuery(m).signed_distance(mid).max()) - R_LOOM, 1)
            rows.append(row)
        out[x] = rows
        print('X', x)
        for r in rows:
            print('  ', r)
        print('  ', KW, 'min clearance', {k: min(r[k] for r in rows) for k in rows[0] if k not in ('th', 'model', 'bow', 'exit_deg', 'carrier_at')}, flush=True)
    json.dump(out, open('loop_sweep.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
