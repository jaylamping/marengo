# Head concepts: 20-face line-up

2026-11-06. Joey asked for a line-up of about 20 robot faces that suit the body and his taste, before we pick a direction.
All 20 are original designs, rendered on the approved body baseline (rev B shell, 215/195 arm), with the same head
size each time (about 132 wide, 164 deep and 204 tall, mm). They are parametric mock-ups for choosing a style, not parts.

| Sheet | Shows |
| --- | --- |
| [HEAD_LINEUP_01-10.png](HEAD_LINEUP_01-10.png) | 1 Sentinel, 2 Crested arrow, 3 Cyclops lens, 4 Twin optics, 5 Scanner bar, 6 Brow guard, 7 Chevron, 8 Listener, 9 Fins, 10 Screen face |
| [HEAD_LINEUP_11-20.png](HEAD_LINEUP_11-20.png) | 11 Grille knight, 12 Hunter, 13 Sensor stack, 14 Owl, 15 Companion, 16 Crown, 17 Talking jaw, 18 Prism, 19 Hood, 20 Stereo brow |

Each tile is a three-quarter view with the front view inset.

The functional idea behind each one:

- Visor heads (1, 2, 7, 9, 16, 17, 18, 20): cameras and eye displays behind tinted acrylic, so the hardware is hidden and the eyes are animated.
- Visible sensor heads (3, 4, 13, 14, 20): the cameras themselves are the face.
- Expression hardware: 8 (ear mic arrays with status rings), 9 (fins on small servos), 10 (full screen), 17 (jaw light that moves with speech), 5 (a scanning light bar).

## Regenerate

The renders come from the body-shell generator: `hardware/designs/body-shell/concept/generator/heads.js` builds a head
with `?head=1..20` (and `?hs=` for head scale). Shots and the sheet script:

```
cd hardware/designs/body-shell/concept/generator   # needs three.js in node_modules and a static server on :8765
DPR=2 node shoot.js '[["i1","head=1&ty=190&d=1500&az=32&el=8"],["f1","head=1&view=front&ty=170&span=600"], ...]'
python3 compose_heads.py
```
