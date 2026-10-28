# Power Distribution Board

KiCad project for the Marengo power distribution board. The project lives at [`../schematics/marengo_pdu/`](../schematics/marengo_pdu/) and is currently an **empty stub** (blank schematic and PCB). Nothing is designed yet.

Design inputs so far (draft, from [`hardware/docs/torso-design-handoff.md`](../../docs/torso-design-handoff.md)): a 48 V main pack in the pelvis feeds the motor bus; a Milwaukee M18 aux pack (18 V) in the upper back is an adjunct, not the motor bus; the PDB lives in the chest zone of the torso cage with the Pi. The physical NC E-stop must break motor power.

## Component groups

_(Document rail assignments, fuse/protection strategy, and revision notes here.)_
