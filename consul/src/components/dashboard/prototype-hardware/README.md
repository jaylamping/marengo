# PROTOTYPE — Hardware page UI

**Throwaway.** Not production. Answers: _What should the Consul Hardware page feel like?_

Three variants on `/prototype/hardware?variant=A|B|C`, switchable via the floating bar or ← →.

| Key | Name        | Structure                                                                     |
| --- | ----------- | ----------------------------------------------------------------------------- |
| A   | Stage       | Full-bleed orbiting robot; chrome floats; click joint → settings sheet         |
| B   | Limb rail   | Limb cards pick the focus set; robot dims to match; list is the picker         |
| C   | Ortho board | Locked orthographic elevation (no orbit); header is the drop target            |

Run: `cd consul && npm run dev:prototype-hardware`

## 3D layer

Vanilla **Three.js** (no `@react-three/fiber` / `drei`), following
[`sickn33/antigravity-awesome-skills` `threejs-skills`](https://www.skills.sh/sickn33/antigravity-awesome-skills/threejs-skills):
scene/camera/renderer, `MeshStandardMaterial` + lights, `OrbitControls`, raycast
picking, soft shadows, ACES tone mapping, `setAnimationLoop`, dispose on teardown.

| File                    | Job                                                                            |
| ----------------------- | ------------------------------------------------------------------------------ |
| `humanoid-rig.ts`       | Armor plates + joint balls in body-space metres                                 |
| `humanoid-scene.ts`     | WebGL scene, picking, DOM label overlay, dirty-flagged render loop              |
| `humanoid-viewport.tsx` | React mount; drives the scene imperatively so state never rebuilds WebGL        |

The figure is a throwaway Bender-from-Futurama homage (corrugated limbs, dome
head, cream visor/grill) — not real URDF meshes and not production art. Coloured
status LEDs sit on actuated joints: green = on CAN, amber = completeness gap,
grey = description only.
