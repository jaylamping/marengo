# Head bake-off brief: marengo's head, after the Iron Giant

You are one of several independent designers asked to model the head for **marengo**, a humanoid robot Joey is building
for himself (personal, non-commercial). Joey's direction: the head should look **as close as possible to the Iron Giant**
(Brad Bird, 1999) in the reference images in `refs/`. An earlier designer has made 10 passes and Joey says they are
"close" but keep missing. Your job is to produce your own best version. Don't copy the earlier script's choices unless you
agree with them.

## What to deliver

Put everything in your own folder `entries/<your-name>/`:

1. `build_head.py`: a **build123d** (Python, OpenCascade) script. Running `python3 build_head.py` writes `out/*.stl`, one
   STL per part, plus `out/parts.json`, a list of `{"file": "<part>.stl", "mat": "<material>"}`. Optionally also write
   `out/head.step`.
2. `NOTES.md`: up to one page. List the proportions you measured from the references (as ratios), the choices you made,
   and anything you'd change with more time.
3. Optional: your own preview renders (`preview_*.png`). The organiser re-renders every entry in one identical scene
   (3/4, front, side and from-below views on the robot's body), so judging is fair.

## Coordinate frame, size and materials (fixed)

- Robot frame in mm: **X forward, Y left, Z up**. z = 0 is the bottom of the neck collar, where the head assembly sits on
  the torso. The head is centred on X = Y = 0.
- Overall head (cranium plus jaw, excluding the collar): about **125–135 mm wide (Y)** and **170–190 mm tall (Z)**. It
  must sit on a collar of radius ≤ 75 mm, with the bottom of the jaw no lower than z ≈ 20.
- Eyes are a **round 1.28" display (~32 mm active diameter)** behind each eye opening, so the glowing disc must be
  ≥ 30 mm across. A stereo camera can hide anywhere in the face.
- Materials. Use only these names in `parts.json`:
  - `white`: head shell, ears, collar
  - `grey1`: very light grey, **the jaw/mouth** (Joey asked for this)
  - `grey2`: darker grey, **the mohawk/crest** (Joey asked for this)
  - `dark`: eye rings, neck, gaps and seams
  - `eye`: glowing eye disc
- No other colour schemes. **Never two-tone** (e.g. a white head with a gunmetal jaw). Joey hates that.

## Joey's feedback so far (verbatim where quoted)

- "get as close to those last two iron giant images … as possible" (ref1 side poster, ref2 front close-up). Later, on the
  film still (ref3): "make it match that exactly".
- Not uncanny-valley, not a blob/egg head.
- On the jaw (pass 3/4): "notice how thick ours currently is vs his". The jaw was a fat block, but his is a plate.
- On pass 4, pinned at the top front of the jaw in side view: "this can still probably be lowered 2 inches". Pinned at the
  chin: "lets make sure he actually has a chin". The earlier designer misread this and made the jaw bigger (pass 5, "you
  somehow made his jaw even larger..."). Joey meant a SHORTER jaw band with a real chin.
- Mohawk: "mohawk look good though" (pass 4 crest: a blade that starts as a point just above the jaw and rises to a spike
  at the crown), "his mohawk is also a darker shade of gray".
- ref5 (Funko): "for the love of god do not turn him into this funkopop … but does this help you get a better idea of the
  proportions?" So use it for proportions only, no chibi.
- Pass 7 (squarer head, flatter crown, eyes near mid-height, short jaw band): "yes now just stretch the top of his head
  about an inch and i think we are close". That became pass 8.
- Pass 8 (jaw band with a flat front and angled corner facets), pinned on the jaw front: "his jaw is rounded, no flat".
- Pass 9 (jaw follows the head's rounded-square plan): "we are regressing now". Then, pinned on the jaw in side view: "how
  about just pull his mouth area out a bit so it's not so smushed and we'll be close". That became pass 10.
- Joey then asked for other designers (you) to try.

Pin screenshots are in `current/joey_pin_*.png`. Earlier passes are `current/HEAD_GIANT_PASS*.png`, and their script is
`current/build_head3_reference.py`, which you may read.

## What the references show (organiser's reading, so check it yourself)

- Cranium: tall, rounded-square in plan, straight sides, and a domed crown that is fairly full rather than a bullet.
- Eyes: two big round glowing eyes, each in a thick dark ring set into a recess. They sit high-middle on the face, fairly
  wide apart.
- Crest: a raised blade on the centre line. It starts as a sharp point just above the jaw, between and below the eyes, and
  grows wider and taller over the forehead into a small spike at the top.
- Jaw: a separate part that wraps the lower face. It stands proud of the face at its top edge (a ledge), and is
  rounded/scooped rather than boxy. It pivots on a round boss at each side toward the back (ref1). Its lower front forms a
  chin.
- Ears: small stepped rectangular blocks on the sides at about eye height.
- Collar: the head sits down into a wide, stepped round collar (ref2, ref4), with the neck hidden.

## Rules

- Work only inside `entries/<your-name>/`. Do not edit anything else in this folder.
- The script must run headless with build123d 0.13 and Python 3.11+, and finish in under 5 minutes.
