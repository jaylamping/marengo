# Entry "fable", round 2: marengo head after the Iron Giant

`python3 build_head.py` -> `out/*.stl`, `out/parts.json`, `out/head.step` (build123d 0.13, ~20 s).
Cranium 125 W x 130 D, jaw band 136 W (145 over the ear pads); chin z = 22, crown z = 196, spike z = 214; collar r = 75.
Head (cranium + jaw) 136 W x 174 H. Materials: white head/collar, grey1 jaw + pivot bosses, grey2 crest,
dark bezel/core/ribbed neck, eye glow (30.4 mm disc in a 41 mm socket).

## Proportions measured from the references (W = cranium width at eye level)

| feature | reference | this entry |
|---|---|---|
| head height (crown to chin) / W | 1.34 (ref2) to 1.40 (ref6, ref9) | 1.39 |
| eye centre below crown / H; ledge below crown / H | 0.47; 0.76 (ref2, ref9) | 0.47; 0.76 |
| jaw band height (ledge to chin) / W | 0.31 to 0.32 | 0.34 |
| straight walls end above eye centre / W | 0.19 to 0.25 (ref2) | 0.21 |
| dome, front view | pointed arch, m ~ 1.6 (ref2, ref6, ref9) | m = 1.65; plan n 4.2 -> 2.3 at the apex |
| dome, side view | flattish forehead, long rounder back curve (ref10, ref8) | m = 2.6 front / 1.8 back |
| eye ring dia / W, glow dia / W, spacing / W | 0.27, 0.20, 0.45 | 0.33, 0.24 (32 mm display), 0.47 |
| eye rim: bright bevel outside the dark ring | ref4, ref11 | 2.5 mm chamfer cone |
| crest: tip above ledge, width at forehead, spike | 0.07 W, 0.13 W, 0.12 W (ref2) | 9 mm, 17 mm, 18 mm |
| crest over / behind the crown | thin low ridge fading at ~140 deg (ref8, ref11) | h x w fade from 62 % of the way to the crown |
| ledge proud of the face / W | 0.03 to 0.035 (ref2, ref4) | 0.044 (5.5 mm incl. 1.5 mm gap) |
| band rear edge: vertical, then a diagonal cut to the chin | ref10, ref1, ref4 | rear edge x = -50, (-50, 50) -> (-6, 22) |
| strap: plate from the band to the pivot, front edge meets the ledge in a knee | ref10, ref1 | knee at x = -26, boss at (-33, 119), r 13 |
| pivot boss: disc + raised ring + recessed centre, at eye height, 55 % back | ref10, ref8, ref1 | r 9 / 6 / 3, z 119 (eye 114) |
| ear = the strap's rounded end, proud of the head | ref9, ref2, ref3 | 5 mm pad, 26 mm tall |
| head shell continues down behind the band into the collar | ref4, ref10, ref11 | skirt to z = 15 |
| dark ribbed neck between band and collar | ref9, ref7 | r 38, three ribs |

## Subtle features found in the round-2 references and changed

1. ref10 / ref1: the jaw's side plate is a back-leaning strap whose front edge meets the ledge in a concave knee,
   not a vertical tab. Rebuilt as band + strap with the knee; the pivot sits at the strap's rounded top.
2. ref10 / ref8 / ref1: the pivot boss is concentric (disc, raised ring, recessed centre) and sits at eye height,
   about 55 % of the head depth back. Moved from z 51 to z 119 (round 1 had it in the jaw band).
3. ref9 / ref2 / ref3: the "ears" are the strap ends standing proud at eye level. The separate stepped ear blocks
   are gone; the strap end is a 5 mm pad carrying the boss.
4. ref10 / ref4: the band's rear edge is vertical with a diagonal lower-rear cut down to the chin (round 1 was a
   full hoop with a rising floor).
5. ref4 / ref10 / ref11: the cranium shell continues down behind the band into the collar (skirt), so the head
   does not float on the collar.
6. ref10 / ref8: side profile is not fore-aft symmetric: flattish forehead, longer rounder curve at the back.
7. ref8 / ref11: the crest is a thin low ridge over and behind the crown; height and width now shrink together
   from 62 % of the way up the forehead (fixes the "heavy hood" flaw of round 1).
8. ref4 / ref11: a bright bevelled rim outside the dark eye ring.
9. ref9 / ref7: dark ribbed neck between the band and the collar; the band is a hoop around the dark cavity, so
   the dark seen from below is the neck, not a void (round-1 flaw).

## How the face serves "Inquisitive, Brave, Endearing"

- Inquisitive: eyes are the biggest feature, round, set high-middle (0.47 H) and wide (0.47 W), with a dark ring
  and bright bevel so they read as open and looking, not slits; nothing else on the face competes with them.
- Brave: solid, tall rounded-square cranium, straight sides, a crest that starts as a point between the eyes and
  rises to a spike, and a jaw with a forward chin (11 mm rake) and a ledge, so the profile has resolve.
- Endearing: no brow, no sharp corners anywhere the eye lands (arch dome, scoop-rounded jaw, soft ledge), eyes
  a little larger than the film (display minimum) which tilts the read toward gentle rather than menacing;
  proportions kept at the film's, not chibi (ref5 ignored except as a proportion check).

## Remaining flaws

- The strap plate is only 5.5 mm proud of the head, so from the side it reads as a shadow line, not the crisp
  outlined plate of the cel (ref10); a 1 mm dark groove around it would help.
- From below, two dark wedges between the band's diagonal cut and the collar show the neck/core; consistent with
  ref10 / ref9 but darker than the posters.
- Eye glow is 24 % of W against 20 % in the film (32 mm display rule).
- Overall jaw width over the ear pads is 145 mm (band 136, cranium 125), above the 125 to 135 band at that point.
- A faint shading seam where the straight walls meet the arch (geometry is tangent, 0.04 mm).
