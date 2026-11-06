# RobStride actuator manuals (vendor reference)

English CAN 2.0 user manuals for the RobStride actuators Marengo uses, copied verbatim for offline reference.

**Source:** <https://github.com/RobStride/Product_Information>, folder `Product Literature/CAN2.0/`, commit `b05095a98d5a586092d4011c3dbb6b7fb719a63b` (2026-10-30). Fetched 2026-11-06. The vendor updates these in place; check the source for newer revisions before relying on a number for a design decision.

| File | Model | Revision in filename | SHA256 |
|------|-------|----------------------|--------|
| `RS00_User_Manual_260713.pdf` | RS00 | 260713 | `47d0ae5c909df2d989b0485c870d36900b9e17c72e608efeb3e13b6e98953832` |
| `RS02_User_Manual_260713.pdf` | RS02 | 260713 | `87baa04792996978d1d4edc4176cd583e695e8bc09a65f97fc982b2e0a7f71b7` |
| `RS03_User_Manual_261013.pdf` | RS03 | 261013 | `53fca37a717b0717d059e7227b53d7d6d2ab9a32fea790ce0c1896527347a1ff` |
| `RS04_User_Manual_260713.pdf` | RS04 | 260713 | `6782bf105cf4cc945142e65356bd7a21a9fc007b008ec3616307956030d72b31` |

Not copied (fetch from the source repo when needed): vendor STEP models (`RSxx-20261030.stp`; RS04 is 30 MB, and CAD lives outside this repo, see `cad/README.md`), the CAN FD manuals (Marengo runs CAN 2.0 at 1 Mbit/s), Chinese-language originals, the RS-series spec overview (Chinese only), OTA protocol, power board and discharge module docs.

## Connector and wiring facts (from section 2 of each manual)

| Model | Drive connector (board end) | Mating plug (line end) | Pins |
|-------|-----------------------------|------------------------|------|
| RS00, RS02 | AMASS XT30PB(2+2)-M.G.B | AMASS XT30(2+2)-F.G.B | VBAT+, GND on the two power blades; CAN-L and CAN-H on the two signal pins (diagram: CAN-L above CAN-H, beside GND) |
| RS03, RS04 | AMASS XT30APW-M (power) + GH1.25-2PWT (CAN) | AMASS XT30UW-F + GH1.25-T 2-pin housing | XT30: VBAT+, GND. GH1.25: CAN-H and CAN-L (diagram order, left to right: CAN-H, GH1.25, CAN-L, GND, XT30, VBAT+) |

- Rated 48 VDC, maximum 60 VDC, operating range 24 to 60 VDC (RS02, RS03).
- CAN 2.0, 1 Mbit/s.
- Rated / maximum phase current: RS02 7 / 23 Apk, RS03 12 / 43 Apk. Standby: RS02 ≤ 18 mA, RS03 ≤ 40 mA.
- The manuals mention a 120 Ω termination resistor only on the vendor USB-CAN module (DIP switch 2). They do not say the drives terminate the bus. Measure CAN-H to CAN-L on a drive before assuming either way.
- Always confirm pin positions against the drive in hand: the manuals give the pinout as a photo, and the physical port is what counts.
