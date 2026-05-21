# Vendor sourcing

Multi-vendor options and lead-time notes for critical BOM lines.

| Part ID | Primary | Alternate | Notes |
|---------|---------|-----------|-------|
| `vendor_2020_black_extrusion` | [Amazon **B08Y8KCJW4**](https://www.amazon.com/dp/B08Y8KCJW4) (IXGNIJ 2020 T-slot black) | CAD: [GrabCAD 2020 Aluminium Extrusions](https://grabcad.com/library/2020-aluminium-extrusions-1) | Canonical SW: `vendor_2020_black_extrusion.SLDPRT`. [`torso-2020-extrusion.md`](../docs/torso-2020-extrusion.md). |
| `frame_2020_*` | Cut from `vendor_2020_black_extrusion` | — | 12 named instances in `marengo_torso_frame_asm_revA`; verticals **340 mm**. |
