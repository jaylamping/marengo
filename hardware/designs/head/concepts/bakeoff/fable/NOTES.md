# Entry "fable": marengo head after the Iron Giant

`python3 build_head.py` -> `out/*.stl`, `out/parts.json`, `out/head.step` (build123d 0.13, ~1 min).
Cranium 125 W x 124 D, jaw 136 W; chin z = 22, crown z = 196, spike to z = 214; collar r = 75.
Head (cranium + jaw) 136 W x 174 H. Materials: white head/ears/collar, grey1 jaw + hinge bosses,
grey2 crest, dark bezel/core/neck, eye glow (30.4 mm disc, pocket r = 20.5).

## Proportions measured from the references (W = cranium width at eye level)

| feature | ref2 / ref6 (front) | this entry |
|---|---|---|
| head height (crown to chin) / W | 1.34 (ref2, foreshortened) to 1.40 | 1.39 (174 / 125) |
| eye centre below crown / H | 0.47 | 0.47 (z 114) |
| jaw ledge below crown / H | 0.76 | 0.76 (z 64) |
| jaw band height (ledge to chin) / W | 0.31 to 0.32 | 0.34 (42 mm) |
| straight walls end above eye centre / W | 0.19 to 0.25 | 0.21 (z 140); dome 56 = 0.45 W |
| dome silhouette | pointed arch, narrower than an ellipse (r = (1 - f^m)^(1/m), m ~ 1.6 fits ref6 at 1/3, 0.6, 0.85 height) | m = 1.65, plan relaxes from n = 4.2 to 2.3 at the apex |
| eye ring dia / W, glow dia / W | 0.27, 0.20 | 0.33, 0.24 (forced by the 32 mm display) |
| eye spacing / W | 0.45 | 0.47 (+-29.5) |
| ring edge to side wall / W | 0.10 | 0.10 |
| crest: tip above ledge / W, width at eyes, at forehead, spike height | 0.07, 0.09, 0.12 to 0.13, 0.12 | 0.07 (9 mm), 11 mm, 17 mm, 18 mm |
| ledge proud of the face / W | 0.03 to 0.035 (ref2, ref4) | 0.044 (5.5 mm incl. 1.5 mm dark gap) |
| jaw band at the back vs front (ref1, ref4) | ~0.6 | 0.57 (24 / 42 mm) |
| hinge boss dia / W (ref1) | ~0.19 | 0.18 (r 11) |
| ears: tall / proud / above eye centre | 0.165 W / 0.04 W / +0.02 W | 27 mm / 9 mm / +3 mm (stepped tab) |
| collar: top step ~W, lower step ~1.15 W (ref2) | | r 68 and r 75, 7 + 9 mm |

## Choices

- Cranium: rounded-square plan (superellipse n = 4.2 front, 3.0 back) with dead-straight walls from inside
  the jaw to z = 140, then a *pointed-arch* dome, not an ellipse: ref2/ref6 show the sides converging almost
  straight into a narrow rounded apex under the spike. The plan rounds toward a circle up the dome so the 3/4
  view stays a full dome rather than a cone (tested both; uniform scaling looked like a helmet).
- Jaw: a 4 mm plate, 1.5 mm off the face, open at the top so the flat top rim is the ledge with a dark gap
  behind it. The front rakes forward 11 mm from the ledge to the chin (Joey: "pull the mouth out"), the lower
  edge is rounded (r 11 front, r 16 sides, scoop), the bottom is a shallow arc across (8 mm sag) and rises in a
  straight line from under the chin (z 22) to z 40 at the back, so the band is deep at the front and short at
  the hinge, as in ref1/ref4. Hinge boss (disc + ring + pin) centred in the short rear band. No flats, no facets.
- Crest: lofted triangular ridge (sharp apex, base widths taken at the head surface) from a point 9 mm above
  the ledge, between and below the eyes, growing to 17 x 11 mm over the forehead, fading out 22 deg behind the
  crown; a separate tapering fin makes the spike at the crown. Darker grey.
- Eyes: 20.5 mm pockets 6 mm deep with a 5.3 mm dark bezel ring and dark pocket lining; the 1.28" display disc
  sits 1 mm behind the face. Eye ring is ~20% larger than the reference ratio because of the 30 mm minimum.
- Ears: stepped blocks (base 17 x 27, tab 12 x 18) at eye height, mid-depth. Collar: two steps, r 75 / r 68.

## Known flaws / with more time

- Slight shading seam where the straight walls meet the arch (geometry is tangent, 0.04 mm; it is the arch's
  curvature jump showing); a true G2 blend would hide it.
- From below, the dark neck shows through two slots between the rising jaw floor and the collar; a deeper
  collar top step or a skirt on the core would close it.
- Eye glow is 24% of W against 20% in the film; the ring could be thinned to 4 mm if Joey prefers a smaller eye.
- Side view: the crest hump over the back of the crown is a little heavy; could fade sooner (crest_end_deg).
- Crest STL is large (fine loft tessellation).
