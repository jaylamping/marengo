# How to assemble this packet (on Joey-PC, where the Git LFS files are real)

From hardware/designs/torso-frame/review/packet3/:
- data/cradle/: ../../cradle/{README.md,CRADLE_SPEC.md,*.step,*.stl}
- data/carrier/: ../../carrier/{README.md,*.step,*.stl}
- data/frame/: ../../generator/parts.json, ../../README.md, ../../BUILD.md
- scripts/: ../../cradle/{build_cradle,check_overlaps,check_hook_fit,mass_estimate,render_cradle}.py, ../../carrier/{build_carrier,draw_carrier}.py
- views/: ../../cradle/CRADLE_ASSEMBLED.png, ../../carrier/CARRIER.png
Then: find data scripts views PACKET.md -type f -print0 | xargs -0 sha256sum > SHA256SUMS.txt
Run per hardware/docs/review-recipe.md. Commit the review outputs; do not commit data/ copies (they duplicate tracked files).
