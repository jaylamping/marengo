# Torso frame rev A: build sheet

2026-11-06. Torso coordinates: X sideways (+X = robot left), Y up, Z forward (+Z = front). "Inner face" = the face that looks
into the box. Numbers from `generator/parts.json` (main). Picture: `FRAME_EXPLODED_DETAIL.png`, `frame_viewer.html`.

## 0. Tools and parts on the bench

- Saw with a square stop (mitre saw or a good box), file/deburr, square.
- **M5 × 0.8 tap** + handle, cutting oil. The 2020 centre bore is already the 4.2 mm tap drill.
- 3 and 4 mm hex keys, a torque-limited driver if you have one (M5 into aluminium T-nuts: snug, about 4 N·m).
- Fasteners (README 2a): M5 × 10 button heads, M5 × 18 button heads (hardpoints, later), 8 M5 × 35 socket heads (end-taps),
  8 DIN 125 M5 washers (pitch brackets), slide-in and drop-in M5 T-nuts, 16 × 2028 corner brackets, 4 × 2020 brackets (20 mm legs),
  4 gussets (3 mm, from `gusset_plate_left.dxf`, two mirrored).

**T-nut rule:** a slide-in T-nut has to be in its slot before both slot ends are closed by another member. If you miss one, a
**drop-in** (twist-in) T-nut goes in any time. When in doubt, use drop-ins for everything in step 5 onward.

## 1. Strip the built frame

Take the built frame apart down to loose parts. Keep: the 4 bottom rails (2 × 110 front/rear, 2 × 100 sides), the 2 × 100 top
side rails, the 2 × 100 shelf side rails (they become the **step side rails**), the shelf plate, the pitch brackets, the brackets and
screws. The 4 × 340 posts get cut in step 2.

## 2. Cut

| From | Cut into | Becomes |
| --- | --- | --- |
| 4 × 340 posts (robot frame) | 198.6 + 121.4 each (one cut at 198.6, kerf goes to the short piece; aim 121.0 to 121.4) | 4 lower posts (198.6), 4 upper posts (121.4) |
| Spare frame post 1 and 2 | 226.0 + rest | 2 shared step rails (front, rear) |
| Spare frame post 3 and 4 | 186.0 + rest | 2 top rails (front, rear) |
| The two 114-ish offcuts from posts 1 and 2 | 100.0 | 2 shelf rails |

Square and deburr every end. Lengths that pair (the two 226s, the two 186s, the four lower posts, the four 100s at each level) must
match each other within 0.2 mm, more than they must hit the nominal number.

## 3. Tap (8 ends)

Tap M5 into **both ends** of: the 2 new **shelf rails** (100) and the 2 **step side rails** (the old shelf side rails, 100).
Go straight, at least 12 mm deep, back the tap out often. These 8 tapped ends are the end-tap joints.

## 4. Lower box (Y 155 to 353.6)

1. Bottom ring: 2 × 110 (front/rear) between 2 × 100 sides, 4 bottom corner joints with **2028 brackets** on the inner faces
   (rev A puts aluminium at the hip attach; the printed bottom gussets come out).
2. Stand the 4 lower posts (198.6) at the corners, each on 2 brackets (one on its inner X face, one on its inner Z face).
3. Load before the next level closes the post tops, per post: 3 T-nuts in the **outer Z face** slot (front posts: front face, rear posts:
   rear face) for the gusset, at about Y 297, 317, 337; 1 in the **outer X face** at about Y 255 for the shell hardpoint H5.
4. Bottom rails: 2 T-nuts in each bottom rail's outer face for H4 (front and rear, X ±40) and H6 (sides, Z ±35). Drop-ins are fine.

## 5. Step level (Y 353.6 to 373.6)

1. Lay the 2 shared rails (226) across the front and rear post tops, centred (they overhang 38 mm each side).
   One bracket per lower post top, on the post's inner X face, into the rail underside (4).
2. Put the 2 shelf rails (100, tapped) between the shared rails at X ±55..75 (directly above the lower posts) and the 2 step side rails
   (100, tapped) at X ±93..113 (the rail ends). Hold them with an M5 × 30 through the shared rail into each tapped end for now, or go
   straight to step 6 with the gussets.
3. Load in each shared rail before the upper posts go on: 2 T-nuts in the top face (upper post brackets), 1 in the outer Z face
   at X ±84 (gusset), 2 in the outer Z face at X ±35 (H3). The shared rail's front/rear face nuts can be drop-ins.

## 6. Upper posts and gussets

1. Stand the 4 upper posts (121.4) on the shared rail ends (X ±93..113), each on one bracket on its inner X face.
2. Per upper post, load 3 T-nuts in the outer Z face for the gusset (about Y 385, 400, 415) and 1 in the inner X face at about
   Y 424 for the pitch bracket.
3. **Gussets** (4, outer Z faces, the R15 corner at X ±113 / Y 335): per plate, 7 × M5 × 10 into the T-nuts (3 lower post, 1 shared
   rail at X ±84, 3 upper post) and **2 × M5 × 35 socket heads** through the plate and the shared rail into the tapped ends of the shelf
   rail (X ±65) and the step side rail (X ±103). Snug all 9, then tighten the two end-taps first.

## 7. Top ring (Y 475 to 495)

1. Load the 2 top rails (186): 2 T-nuts in the outer Z face for H1/H2 (front X ±60, rear X ±85).
2. Top side rails (100): 1 T-nut in the inner X face at Z 0 for the pitch bracket.
3. Fit the top ring on the upper posts: 4 corner joints with the **2020 brackets (20 mm legs)** on the inner X faces. The Z-face bracket
   at those corners is gone on purpose (it sat inside the pitch bracket).

## 8. Shoulders, shelf, rest

- Pitch brackets onto the step side rail (Y 364), top side rail (Y 485) and upper posts (Y 425, Z ±60): M5 × 10 + DIN 125 washer.
- Shelf plate (your reprint with the cable slots) between the shelf rails at Y 354.6: optional for now (Joey 20:41Z), the shelf rails go in anyway because they are part of the step joint.
- Shell hardpoints H1 to H6 later with M5 × 18 (measure the screw depth into a T-nut first).
- Pack cradle and board carrier when printed.

## Check when done

- Diagonals of each box face equal within 1 mm; upper box 226 wide, lower 150 wide; height 340.
- Pitch axis at Y 424.9 (269.9 above the frame bottom) on both sides.

T-nut count: 96 (40 brackets, 28 gussets, 20 hardpoints, 8 pitch brackets) plus 8 end-tap screws with no nut.
