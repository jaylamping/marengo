# Torso frame gate review 2: rev A + 13S2P pack + boards

2026-11-06. Packet `review/packet2/` (main 525283a, 32 hashed files, `SHA256SUMS.txt` sha256
`c9837500d4e3cfc76d268a6506e881f339773aa1a6a07f02c2b46ec0f11439c7`). Hashes checked before (10:52:25Z) and after (11:10:42Z): unchanged.
Reviews in `packet2/reviews/`, run log in `packet2/reviews/STATUS.txt`. Commands: `hardware/docs/review-recipe.md`.

## Reviewers and times

| Reviewer | Elapsed | Verdict |
| --- | --- | --- |
| Muse Wonder | 253 s | NO-GO on cutting the posts and ordering the gussets as-is |
| Codex Sol | 665 s | NO-GO on cutting and the gussets |
| Codex Astra | 696 s | NO-GO on cutting and the gussets |
| Opus 5.5, high (API) | 960 s | Cutting the posts is low-regret; do not build the tray, carrier or harness/cooling as specified; order gussets after the end-anchor concept |
| Antigravity Argon, High | 963 s | NO-GO: no cut, no gusset order, no tray/carrier print |
| Fable 5.1, high (API) | 1097 s | Not ready to cut or order |

Codex attempt 1 failed in 0 s for both Codex reviewers (`-i` swallowed the prompt); attempt 2 passed the prompt on stdin. All six ran in
parallel on the MacBook, about 18 min wall clock.

## Fixed in this revision (generator, sheets, README)

| # | Finding (who) | Fix |
| --- | --- | --- |
| 1 | Bracket screw heads were drawn inside the bracket leg and the T-nuts 4 mm shallow, on the wrong side of the member face (Sol, Astra, Fable, Opus) | `bolt_bracket`: head on the leg's open face, nut 1.5 to 7.5 mm behind the member face. Holes moved 2.5 mm out along each leg so the two heads no longer meet in the inside corner |
| 2 | 28 T-nuts counted but not drawn: 20 hardpoint and 8 pitch-bracket nuts (Fable, Opus) | Drawn (`hft_*`, `pbt*`) |
| 3 | M5 × 20 through 12 mm PETG and M5 × 12 through the 4 mm ears bottom in the slot before they clamp (Opus) | M5 × 18 hardpoints, M5 × 10 pitch screws. **Measure the slot depth of the stock first** |
| 4 | Spine rail lip (\|X\| ≤ 60) hits the rear gussets and their screw heads (Opus M8) | Lip \|X\| ≤ 52 |
| 5 | Board screws floated 10 mm above the carrier; no standoffs (Fable, Astra) | 6 mm M3 F-F standoffs drawn, PCBs from Z 47.5, M3 × 6 both sides |
| 6 | Carrier, Nomex sheet and tray rails overlap (Sol, Muse) | Rails end at Z 37.5, sheet inside the rails (\|X\| ≤ 47.5), carrier \|X\| ≤ 55 screwed to the rail front ends |
| 7 | Front-corner harness (Z 62 to 70) ran through the front rails and 8 corner brackets; the "Z 50 to 60 gap" between the shelf and the shared front rail does not exist (all six) | Trunk moved into the side bands (\|X\| 50 to 55, Z 42 to 50, Y 160 to 353.6). The step passage is now drawn as an **open, proposed** shelf corner notch |
| 8 | Shoulder harness (X ±42 to 78) ran through the Jetson (Opus M9) | Stops at \|X\| 52 and crosses behind the Jetson at Z −58 |
| 9 | "8.5 mm to the front rail plane" is wrong: the front rail is at Z 50 to 70 (Sol, Astra) | Reworded: 8.5 mm behind the frame front face; front rails only at Y 155 to 175 and 353.6 to 373.6 |
| 10 | README stale: "battery not in the torso", "frame reopens if the pack moves to the torso", frame mass 2.1 kg, third board "lower box front", M5 × 16/20 (Fable, Opus, Argon) | Rewritten; frame structure now **2.8 kg** (generator sum), everything in the torso with the pack **8.8 kg** |
| 11 | T-nut order short: one 50-pack of drop-ins vs about 68 needed (Fable) | Shopping list: two 50-packs of drop-ins |

My own errors that the reviewers caught: the front rail Z band (I used 60 to 80), the step-gap claim, "rails on the lower post inner
faces" (no geometry attached them), "interference-free", the packet's claim that the low pack mass does not load the step joints
(Sol: unverified, about 367 N of local inertia; folded into L), and the stale README text above.

Interference check after the fixes (`parts.json`, bounding boxes, T-nuts and shanks inside their members excluded): the remaining
hits are all in the open list below (step-level brackets vs shelf, pitch bracket envelope vs upper corner brackets, nut-map overlaps,
bracket heads at the board corners, spine pads vs H2 and gusset heads), plus placeholders (waist RS03 vs pelvis ghost).

## Accepted, open (needs a decision or another thread)

| # | Item (who) | Proposed |
| --- | --- | --- |
| A | **Pack retention.** Nothing holds the pack in a fall; the D2 door (magnets) is not a restraint; the tray rails have no load path to the frame (all six) | Printed cradle: flanges onto the post inner faces at Y 203 to 325 (clear of the corner brackets), bottom lip, front lip, top hold-down, and a removable rear bar or two thumbscrews. **Joey decision**; battery thread df11714: two M4 inserts on the pack rear face at X ±35, Y 205; keep clear of X 5 to 45, Y 300 to 345 |
| B | **No cable passage from the lower box to the upper bay** (all six) | 10 × 10 notch in each front corner of the printed shelf (\|X\| 45 to 55, Z 40 to 50). **Joey decision** (the shelf cutout for connectors was withdrawn; this is a cable notch) |
| C | **Step-level joints.** The 8 shelf and step-side brackets (br24 to br31) sit in the shelf plate; their heads too (Sol, Astra, Fable) | End-tap the shelf and step-side rails (M5 into the extrusion core) instead of brackets, or notch the shelf. **Joey decision** together with B |
| D | Rear lower-box corner bracket heads protrude into the pack's slide path (Astra) | Rotate those brackets to the Z faces, or use end-tap anchors (same choice as C) |
| E | **Lower box has no exhaust**: shelf and shared rails close it (Sol, Fable, Opus, Argon) | Shell c73d348: sideways out the V1 side grilles (upper half louvred exhaust). Frame path: the Z 37.5 to 50 window each side (tray rails below, front posts above), shared with the cable band |
| F | Pack self-heating about 133 W at 40 A (Sol, Argon) | **Answered** (battery df11714): 20 W steady, 36 W worst sustained; 40 A is a burst (under 1 K) |
| G | Brake resistor: real HS50 class part is bigger than 25 × 50 × 8 (Astra, Opus) | **Fixed**: Arcol HS25 10 Ω (board thread) flat on a 2 mm plate on the front bottom rail inner face, 1 mm clear of the pack; the HS50 overlapped it by 1.7 mm |
| H | **Answered** (board a50269c3: corner keep-outs for parts over 5 mm, arm terminals inboard of \|X\| 44). Corner bracket heads (X ±46.5 to 51) reach 3.5 mm into the board corners at Y 175 to 203 and 325 to 353 once seated correctly (Astra; confirmed by the fix in 1) | Board thread: corner keep-outs or 93 mm boards |
| I | 32 V MIDI fuse in the pack doc on a 54.6 V pack (Sol, Argon) | **Fixed** (battery df11714): ≥ 58 V; Class T 60 A preferred |
| J | RS03 housing at GND makes the 1 MΩ ∥ 10 nF frame bond moot (Fable, Opus) | Measure housing-to-GND on a real RS03; board thread |
| K | Pitch bracket vs the upper corner brackets; M5 vs M6 in the ears (Sol, Opus) | **Arm thread 11:47Z (bracket STL vs parts.json):** the four top Z-face brackets overlapped the as-built pitch bracket by about 4440 mm³ each: **deleted** (28 brackets, 120 M5). Ear slots 6.2 to 6.4: M5 + DIN 125 washers. Still open: br3/9/15/21, br4/10, br25/27 and the pitch screw heads touch at 0 mm, so check them on the real parts |
| L | End anchors for the step joint (friction only along the slots) (Sol, Astra, Opus) | Design before ordering gussets (from gate 1) |
| M | Nut map and build order: 16 bracket-nut pairs overlap at shared corners, plus gusset/hardpoint/pitch nuts in the same slots (Opus, Argon) | Slot-by-slot map before the build |
| N | Upper spine pads overlap the H2 brackets and the rear gusset heads by up to 5 mm in Z (script check) | Shell thread: pad standoffs |
| O | Hip/waist decision before cutting the posts (Sol, Astra, Argon) vs "cutting is low-regret" (Opus) | **Joey decision** |
| P | Boot the ring lugs; side-band trunk is 5 × 8 mm per side, tight (Astra, Muse) | Board thread: bundle size |

## Not adopted

| Finding | Why |
| --- | --- |
| Decide the leg-stage battery before cutting, because a 110 mm inner lower box locks out a future 13S3P torso pack (Argon) | Joey locked 13S2P at 06:13Z. More runtime goes to the pelvis cell or backpack options, which stay open for the hip stage; the lower box is not the place for it. Joey can overrule under O |

## Gate status

**NO-GO stands** on ordering the gussets and building the tray, carrier and harness until A, B/C, E and L are settled. Post cutting
waits for Joey's call on O.

## Joey's decisions (2026-11-06 16:32 to 16:43Z)

| Item | Decision |
| --- | --- |
| A, pack retention | Printed cradle on the lower post inner faces, bottom and front lips, top strap, **two side thumbscrews** through the tray rails into M4 heat-set bosses in the pack case sides (battery thread: Y 252.6, Z −20 to +10; tip at most 6 mm past the side face, shouldered screw). No rear bar; more support can be added later |
| B, cable passage | **Oval cable slots in the printed shelf**, PC-case style, as on the built shelf. Modelled as three 20 × 12 slots along the front edge; Joey packs the upper bay himself (Pi I/O stays at the rear, behind D1) |
| C, step-level joints | **End-tapped**: M5 × 35 through the gusset and the shared rail into the tapped shelf rail / step side rail end, gusset holes X ±65 and ±103 at Y 363.62 (DXF regenerated). These fixed screws also anchor the gusset along the slot (item L) |
| O, post cuts | **Cut now** (198.62 + 121.38); the hips attach under the bottom ring later |
