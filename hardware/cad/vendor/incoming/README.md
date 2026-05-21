# Incoming vendor STEP

Drop vendor downloads here before staging to `hardware/cad/vendor/`.

| File (you provide) | Registry `asset_id` |
|--------------------|---------------------|
| `grabcad_2020_aluminium_extrusions.SLDPRT` | `extrusion_2020_profile` (GrabCAD SW download → review → save canonical copy under `vendor/`) |
| `robstride_rs03_vendor.stp` | `actuator_rs03` (already staged) |

Then: MCP `vendor_stage_local_asset` → import in SolidWorks → save `.SLDPRT` next to `.step` in `vendor/`.
