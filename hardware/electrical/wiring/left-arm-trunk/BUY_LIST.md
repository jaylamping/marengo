# Left arm trunk (option B): buy list

Rev 0, 2026-11-06. Quantities are per arm, with the spares in brackets. Links were checked by search on 2026-11-06. Prices and stock change, so confirm the exact part number on the page before ordering.

**Read this first: fuse voltage.** Ordinary ATM/mini-blade fuses are rated **32 V DC**. That is fine on the 24 V bench supply but **not on the 54.6 V pack**: a 32 V fuse may not stop arcing when it blows at 48 V. On the robot, use the Littelfuse **997 series MINI 58 V** fuses below. They fit standard mini-blade holders. Most inline holders are also labelled 32 V, so check the holder's rating too.

| # | Part | Qty per arm | Where | Notes |
|---|------|-------------|-------|-------|
| 1 | AMASS XT30(2+2)-F.G.B female (RS02 / RS00 drops) | 3 (+2) | [LCSC C19268028](https://www.lcsc.com/product-detail/C19268028.html) | Genuine AMASS. LCSC ships to the US; group it with the other LCSC parts |
| 2 | AMASS XT30(2+2)-M (trunk end that plugs into the power board port) | 1 (+1) | LCSC, same family as #1 (search "XT30(2+2)-M") | Only needed once the power board exists; on the bench, the trunk starts at the PSU/E-stop |
| 3 | AMASS XT30UW-F right-angle female (RS03 power drops) | 2 (+2) | [GetFPV AMASS](https://www.getfpv.com/amass?cat=75) or [Zbotic XT30UW-F](https://zbotic.in/product/amass-xt30uw-female/) | Low profile for the 11 to 12 mm gap at the pitch flats |
| 4 | AMASS XT30U-F / XT30U-M pairs (joint breaks, if kept) | 4 pairs (+2) | [LCSC XT30U-F C99102](https://lcsc.com/product-detail/plug_Changzhou-Amass-Elec-XT30U-F_C99102.html) + matching M, or [GetFPV AMASS](https://www.getfpv.com/amass?cat=75) | Female on the upstream (live) side |
| 5 | JST GH 1.25 mm 2-pin housings (GHR-02V-S) + pre-crimped leads | RS03 CAN: 2 (+2); joint breaks: 4 pairs (+2) | [Digi-Key JST GH series](https://www.digikey.com/en/product-highlight/j/jst/gh-series-connector) (GHR-02V-S housings, AGHGH28K305 pre-crimped leads) or a pre-crimped kit like [this one](https://www.unmannedtechshop.co.uk/products/jst-gh-1-25mm-custom-cable-master-kit-pre-crimped-silicone-wires-2-10-pin) | Pre-crimped leads avoid needing a GH crimper. Solder the 28 AWG lead to the 26 AWG CAN pair. The RS03 CAN plug is GH1.25 (vendor manual) |
| 6 | Mini-blade **58 V** fuses, Littelfuse 997 series | 7.5 A ×2, 5 A ×2, 3 A ×1 (+1 each) | [0997005.WXN (5 A) at Digi-Key](https://www.digikey.ph/en/products/detail/littelfuse-inc/0997005-WXN/701057); 3 A = [0997003.WXN](https://www.littelfuse.com/products/fuses-overcurrent-protection/fuses/automotive-fuses/blade-fuses-shunt/mini/997/0997003-wxn); 7.5 A = 0997007.5WXN (verify) | The 997 series is time-delay, which is fine here. Low-profile alternative: [LP MINI 891 series 58 V](https://littelfuse.com/products/fuses/automotive-passenger-car/blade-fuses/891-lp-mini.aspx) |
| 7 | Inline mini-blade (ATM) fuse holder, 16 AWG leads | 5 (+2) | [Parts Express 070-676](https://www.parts-express.com/In-line-Mini-Blade-ATM-16-AWG-Fuse-Holder-150mm-x-2-070-676) or [Crimp Supply](https://crimpsupply.com/mini-atm-inline-fuse-holder-with-cap-16-ga-wire-leads.html) | Check the voltage rating (see top). Shorten the leads to fit the 200 mm drop |
| 8 | 16 AWG silicone wire, red + black | 3.5 m each | [BNTECHGO 16 AWG red/black](https://www.desertcart.sg/products/407432477-bntechgo-16-gauge-silicone-wire-spool-red-and-black-each) (also on Amazon) | You may already have this |
| 9 | Elbow section S4 (decided 2026-11-08, Joey): **round high-flex wire in loom, free service loop, no guard.** 18 AWG high-strand silicone, red ×2 + black ×2 (the power pair doubled for the future hand) + the 26 AWG CAN pair. Plus 1 extra mini-blade holder and a 58 V 5 A fuse at the M14 splice; hand drop: 1 more XT30U-F, 1 GH 2-pin, 1 holder + 58 V 3 A fuse, 120 Ω terminator cap | 18 AWG: 0.5 m each of red and black (cut 2 × 250 mm per colour) | [BNTECHGO 18 AWG silicone red/black](https://www.amazon.com/s?k=BNTECHGO+18+AWG+silicone+red+black) or any 18 AWG silicone with fine strands | Loom for the loop: see the sleeving table, S4 row |
| 10 | 26 AWG CAN twisted pair | 3.5 m | [Spec55 twisted CAN 26 AWG](https://xtramotorsport.com/?p=11352), or twist your own from 26 AWG silicone (yellow + green) with a drill, ~1 turn per 25 mm | Silicone twists tighter and flexes better at the joints |
| 11 | Solder-seal heat-shrink splices + assorted heat-shrink | 1 kit | [Crimp Supply 16-14 solder heat-shrink splices](https://crimpsupply.com/16-14-ga-solder-heat-shrink-butt-splice-terminals.html) | For the drop T-splices: solder, then shrink each conductor and add an overall shrink |
| 12 | 120 Ω ¼ W resistor | 1 (+2) | any (Digi-Key, Amazon assortment) | Across CAN_H and CAN_L at the M15 splice |

Rough total per arm, excluding wire you already have: about $40 to $70, mostly connectors and 58 V fuses.

## Sleeving (Joey 2026-11-06: sleeve all wiring)

Techflex Flexo PET, sized from the [Techflex PET size table](https://www.jmesales.com/content/docs/Techflex/pet-specs.pdf):

| Where | Size | Fits |
|-------|------|------|
| Trunk (4 conductors, ~7 mm) | PTN0.25 (1/4") | 3.2 to 11.1 mm |
| Drops, CAN pair on its own | PTN0.13 (1/8") | 2.4 to 6.4 mm |
| **S4 elbow loop**, 6 conductors, about 7 mm bundle | PTN0.38 (3/8"), run loose so it stays flexible | 4.8 to 15.9 mm. Buy 0.5 m |
| Torso battery / power-board runs | PTN0.75 (3/4", Joey's [Digi-Key PTN0.75BK250](https://www.digikey.com/en/products/detail/techflex/PTN0.75BK250/2502453)) | 12.7 to 31.8 mm |

Do **not** sleeve the ribbon clock-spring in the lower-arm yaw (inside the forearm): braid stiffens the bend and abrades the ribbon. **Exception, the elbow (S4):** Joey chose a free round-wire loop in loom there. Use the oversize PTN0.38 so the braid stays loose on the 7 mm bundle and doesn't stiffen the bend, and stop the braid inside each anchor clamp.

**Terminating the sleeve:** cut hot (hot knife or soldering iron) or flame-seal a cold cut; cut ~10% long; slide it on before soldering connectors (bunch it to open it, inchworm it along, tape the bundle end); cap every end with 15 to 20 mm of **adhesive-lined 3:1 heat-shrink** lapping half on the sleeve, half on the wire or boot. At a drop splice, end the trunk sleeve on both sides and let the splice heat-shrink lap over both ends. Add adhesive-lined 3:1 heat-shrink (assorted 3 to 12 mm) to the order.
