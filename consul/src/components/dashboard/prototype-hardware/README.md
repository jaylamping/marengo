# PROTOTYPE — Hardware page UI

**Throwaway.** Not production. Answers: _What should the Consul Hardware page feel like?_

Three variants on `/prototype/hardware?variant=A|B|C`, switchable via the floating bar or ← →.

| Key | Name        | Structure                                                                     |
| --- | ----------- | ----------------------------------------------------------------------------- |
| A   | Table · 3D  | **Preferred.** Joint data table by default; header toggle reveals Bender 3D   |
| B   | Limb rail   | Limb cards pick the focus set; robot dims to match; list is the picker         |
| C   | Ortho board | Locked orthographic elevation (no orbit); header is the drop target            |

Run: `cd consul && npm run dev:prototype-hardware`

## Preferred direction (A)

- Default chrome is a joint **data table** (status, limb, CAN, gaps, sources).
- Click a row → same unified settings sheet (source-tagged fields, Accept incoming).
- Header **Table / 3D** toggle swaps in the Bender-style Three.js picker without changing the sheet contract.
- 3D stays optional until it has enough polish to be the primary surface.

## 3D layer

Vanilla **Three.js** (no `@react-three/fiber` / `drei`), following
[`sickn33/antigravity-awesome-skills` `threejs-skills`](https://www.skills.sh/sickn33/antigravity-awesome-skills/threejs-skills).

| File                     | Job                                                                           |
| ------------------------ | ----------------------------------------------------------------------------- |
| `humanoid-rig.ts`        | Bender anchors + palette                                                       |
| `humanoid-scene.ts`      | WebGL scene, picking, DOM label overlay, dirty-flagged render loop             |
| `humanoid-viewport.tsx`  | React mount; drives the scene imperatively so state never rebuilds WebGL       |

The figure is a throwaway Bender-from-Futurama homage — not real URDF meshes and not
production art. Coloured status LEDs: green = on CAN, amber = completeness gap,
grey = description only.
