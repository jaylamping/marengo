# PROTOTYPE — Hardware page UI

**Throwaway.** Not production. Answers: _What should the Consul Hardware page feel like?_

Three variants on `/prototype/hardware?variant=A|B|C`, switchable via the floating bar or ← →.

| Key | Name        | Structure                                                                     |
| --- | ----------- | ----------------------------------------------------------------------------- |
| A   | Stage       | Full-bleed orbiting humanoid; chrome floats; click joint → settings sheet      |
| B   | Limb rail   | Limb cards pick the focus set; humanoid dims to match; list is the picker      |
| C   | Ortho board | Locked orthographic elevation (no orbit); header is the drop target            |

Run: `cd consul && npm run dev:prototype-hardware`

## 3D layer

Vanilla **Three.js** — no `@react-three/fiber`, no `drei`.

| File                   | Job                                                                            |
| ---------------------- | ------------------------------------------------------------------------------ |
| `humanoid-rig.ts`      | Skeleton anchors + bones in body-space metres                                   |
| `humanoid-scene.ts`    | Renderer, camera, lights, raycast picking, DOM label overlay, dirty-flag loop   |
| `humanoid-viewport.tsx`| React mount point; drives the scene imperatively so state never rebuilds WebGL  |

Picking uses oversized invisible hit spheres so a 28 mm joint marker is still easy to
click. Hover raycasts are throttled to 40 ms, and the frame loop only renders when
something actually changed.

Colour follows `DESIGN.md`: green LED = on CAN, amber = completeness gap, grey =
description only, amber ring = selection.
