# Power Distribution Board

KiCad project for the Marengo power distribution board. The project lives at [`../schematics/marengo_pdu/`](../schematics/marengo_pdu/) and is currently an **empty stub** (blank schematic and PCB). Nothing is designed yet.

**Architecture plan (rev 0, 2026-11-06): [`PLAN.md`](PLAN.md)**, block diagram [`POWER_BOARD_BLOCKS.png`](POWER_BOARD_BLOCKS.png). Planning only; nothing approved or ordered.

Current inputs: custom 13S2P 21700 pack in the torso lower box ([`../power/custom-21700-pack.md`](../power/custom-21700-pack.md), Joey 2026-11-06), BMS on this board, board envelope 100 × 150 × 24 in front of the pack ([torso frame README](../../designs/torso-frame/README.md) section 6). The older handoff inputs (48 V pack in the pelvis, Milwaukee M18 aux pack, PDB in the chest zone) are superseded.

## Component groups

_(Document rail assignments, fuse/protection strategy, and revision notes here.)_
